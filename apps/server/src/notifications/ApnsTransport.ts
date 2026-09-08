// @effect-diagnostics nodeBuiltinImport:off globalDate:off globalTimers:off
import * as NodeCrypto from "node:crypto";
import * as NodeHttp2 from "node:http2";

export interface ApnsConfig {
  readonly teamId: string;
  readonly keyId: string;
  readonly bundleId: string;
  readonly privateKey: string;
}
export interface ApnsMessage {
  readonly token: string;
  readonly environment: "sandbox" | "production";
  readonly liveActivity: boolean;
  readonly payload: object;
}
export function createApnsTransport(config: ApnsConfig) {
  const key = NodeCrypto.createPrivateKey(config.privateKey.replace(/\\n/g, "\n"));
  if (key.asymmetricKeyType !== "ec" || key.asymmetricKeyDetails?.namedCurve !== "prime256v1") {
    throw new Error("APNs requires an Apple P-256 signing key.");
  }
  let jwt = "";
  let expires = 0;
  return async (message: ApnsMessage): Promise<boolean> => {
    const now = Math.floor(Date.now() / 1000);
    if (!jwt || now >= expires) {
      const header = Buffer.from(JSON.stringify({ alg: "ES256", kid: config.keyId })).toString(
        "base64url",
      );
      const claims = Buffer.from(JSON.stringify({ iss: config.teamId, iat: now })).toString(
        "base64url",
      );
      const content = `${header}.${claims}`;
      const signature = NodeCrypto.sign("sha256", Buffer.from(content), {
        key,
        dsaEncoding: "ieee-p1363",
      }).toString("base64url");
      jwt = `${content}.${signature}`;
      expires = now + 45 * 60;
    }
    return new Promise((resolve, reject) => {
      const client = NodeHttp2.connect(
        message.environment === "sandbox"
          ? "https://api.sandbox.push.apple.com"
          : "https://api.push.apple.com",
      );
      const finish = (error?: Error, accepted = false) => {
        clearTimeout(timer);
        client.destroy();
        if (error) reject(error);
        else resolve(accepted);
      };
      const timer = setTimeout(() => finish(new Error("Apple push delivery timed out.")), 10_000);
      client.on("error", () => finish(new Error("Could not connect to Apple push service.")));
      const request = client.request({
        ":method": "POST",
        ":path": `/3/device/${message.token}`,
        authorization: `bearer ${jwt}`,
        "apns-topic": `${config.bundleId}${message.liveActivity ? ".push-type.liveactivity" : ""}`,
        "apns-push-type": message.liveActivity ? "liveactivity" : "alert",
        "apns-priority": message.liveActivity ? "5" : "10",
        "apns-expiration": String(now + 3600),
        "content-type": "application/json",
      });
      let status = 0;
      let body = "";
      request.on("response", (headers) => {
        status = Number(headers[":status"]);
      });
      request.setEncoding("utf8");
      request.on("data", (chunk: string) => {
        if (body.length < 4096) body += chunk;
      });
      request.on("error", () => finish(new Error("Apple push request failed.")));
      request.on("end", () => {
        if (status === 200) return finish(undefined, true);
        if (
          status === 410 ||
          (status === 400 && /BadDeviceToken|DeviceTokenNotForTopic/.test(body))
        )
          return finish(undefined, false);
        finish(
          new Error(
            `Apple push delivery failed (HTTP ${status}). Check the APNs key, team, and bundle ID.`,
          ),
        );
      });
      request.end(JSON.stringify(message.payload));
    });
  };
}
