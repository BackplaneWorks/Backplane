// JSON <-> protobuf for the API envelope. Messages ride as
// google.protobuf.Any inside kiapi.common.ApiRequest / ApiResponse.
import { STATUS, type Schema } from "./schema.ts";

const PREFIX = "type.googleapis.com/";

const TO = { longs: String, enums: String, bytes: String, defaults: false, json: true } as const;

export function encodeRequest(s: Schema, type: string, message: unknown, token: string, client: string): Uint8Array {
  const T = s.type(type);
  if (!T) throw new Error(`unknown command ${type}`);
  const err = T.verify(T.fromObject((message ?? {}) as object));
  if (err) throw new Error(err);
  const body = T.encode(T.fromObject((message ?? {}) as object)).finish();
  const Req = s.type("kiapi.common.ApiRequest")!;
  const msg = Req.fromObject({ header: { kicad_token: token, client_name: client }, message: { type_url: PREFIX + type, value: body } });
  return Req.encode(msg).finish();
}

export type Reply =
  | { ok: true; type: string; message: unknown; token: string }
  | { ok: false; status: string; error: string; token: string };

export function decodeResponse(s: Schema, bytes: Uint8Array): Reply {
  const Res = s.type("kiapi.common.ApiResponse")!;
  const r = Res.decode(bytes) as unknown as {
    header?: { kicad_token?: string };
    status?: { status?: number; error_message?: string };
    message?: { type_url?: string; value?: Uint8Array };
  };
  const token = r.header?.kicad_token ?? "";
  const code = r.status?.status ?? 0;
  if (code !== 1) {
    return { ok: false, status: STATUS[code] ?? `AS_${code}`, error: r.status?.error_message ?? "", token };
  }
  const url = r.message?.type_url ?? "";
  if (!url) return { ok: true, type: "", message: {}, token };
  const name = url.slice(url.lastIndexOf("/") + 1);
  const T = s.type(name);
  if (!T) return { ok: true, type: name, message: { "@type": name, bytes: Buffer.from(r.message?.value ?? []).toString("base64") }, token };
  const m = T.decode(r.message?.value ?? new Uint8Array());
  return { ok: true, type: name, message: T.toObject(m, TO), token };
}

// server side (the test stub): a request's type and message
export function decodeRequest(s: Schema, bytes: Uint8Array): { type: string; message: unknown; token: string } {
  const Req = s.type("kiapi.common.ApiRequest")!;
  const r = Req.decode(bytes) as unknown as { header?: { kicad_token?: string }; message?: { type_url?: string; value?: Uint8Array } };
  const url = r.message?.type_url ?? "";
  const name = url.slice(url.lastIndexOf("/") + 1);
  const T = s.type(name);
  return { type: name, message: T ? T.toObject(T.decode(r.message?.value ?? new Uint8Array()), TO) : {}, token: r.header?.kicad_token ?? "" };
}

export function encodeResponse(s: Schema, reply: { type?: string; message?: unknown; status?: number; error?: string; token: string }): Uint8Array {
  const Res = s.type("kiapi.common.ApiResponse")!;
  const obj: Record<string, unknown> = { header: { kicad_token: reply.token }, status: { status: reply.status ?? 1, error_message: reply.error ?? "" } };
  if (reply.type) {
    const T = s.type(reply.type)!;
    obj.message = { type_url: PREFIX + reply.type, value: T.encode(T.fromObject((reply.message ?? {}) as object)).finish() };
  }
  return Res.encode(Res.fromObject(obj)).finish();
}
