import { EnvironmentId, KiCadProjectManifest, KiCadViewerSession } from "@t3tools/contracts";
import * as Effect from "effect/Effect";
import * as Option from "effect/Option";
import * as Schema from "effect/Schema";
import { HttpClient, HttpClientRequest, HttpClientResponse } from "effect/unstable/http";
import * as HttpClientError from "effect/unstable/http/HttpClientError";
import { Atom } from "effect/unstable/reactivity";
import { RemoteEnvironmentAuthorization } from "../authorization/service.ts";
import type { PreparedConnection } from "../connection/model.ts";
import { EnvironmentRegistry } from "../connection/registry.ts";
import { environmentEndpointUrl } from "../environment/endpoint.ts";
import { ManagedRelayDpopSigner } from "../relay/managedRelay.ts";
import { executeAuthenticatedEnvironmentHttpRequest } from "./environmentHttpAuth.ts";
import { createEnvironmentSessionAtoms } from "./session.ts";

export type KiCadQueryTarget = { readonly environmentId: EnvironmentId; readonly cwd: string };

const kiCadEndpointUrl = (base: string, path: string, cwd: string): string => {
  const url = new URL(environmentEndpointUrl(base, `/api/kicad/${path}`));
  url.searchParams.set("cwd", cwd);
  return url.toString();
};

const fetchKiCadJson = <A>(input: {
  prepared: PreparedConnection;
  cwd: string;
  path: string;
  schema: Schema.Codec<A, unknown>;
  method: "GET" | "POST";
}) =>
  Effect.gen(function* () {
    const signer = yield* Effect.serviceOption(ManagedRelayDpopSigner);
    const remoteAuthorization = yield* Effect.serviceOption(RemoteEnvironmentAuthorization);
    return yield* executeAuthenticatedEnvironmentHttpRequest<
      HttpClientResponse.HttpClientResponse,
      HttpClientError.HttpClientError,
      HttpClient.HttpClient
    >({
      prepared: input.prepared,
      signer,
      remoteAuthorization,
      method: input.method,
      timeoutMs: 15_000,
      url: (base) => kiCadEndpointUrl(base, input.path, input.cwd),
      isUnauthorizedResponse: (response: HttpClientResponse.HttpClientResponse) =>
        response.status === 401,
      request: ({ headers, requestUrl }) => {
        const url =
          requestUrl ?? kiCadEndpointUrl(input.prepared.httpBaseUrl, input.path, input.cwd);
        const withHeaders = (request: HttpClientRequest.HttpClientRequest) => {
          let result = request;
          if (headers.authorization)
            result = result.pipe(
              HttpClientRequest.setHeader("authorization", headers.authorization),
            );
          if (headers.dpop) result = result.pipe(HttpClientRequest.setHeader("dpop", headers.dpop));
          return result;
        };
        const request =
          input.method === "POST"
            ? withHeaders(HttpClientRequest.post(url))
            : withHeaders(HttpClientRequest.get(url));
        return Effect.flatMap(HttpClient.HttpClient, (client) => client.execute(request));
      },
    });
  });

export function createKiCadState<R, E>(
  runtime: Atom.AtomRuntime<EnvironmentRegistry | HttpClient.HttpClient | R, E>,
) {
  const session = createEnvironmentSessionAtoms(runtime);
  const preparedFor = session.preparedConnectionValueAtom;
  const query = <A>(
    label: string,
    path: string,
    schema: Schema.Codec<A, unknown>,
    method: "GET" | "POST",
  ) => {
    const family = Atom.family((key: string) => {
      const target = JSON.parse(key) as KiCadQueryTarget;
      return runtime
        .atom((get) => {
          const prepared = Option.getOrNull(get(preparedFor(target.environmentId)));
          return prepared === null
            ? Effect.never
            : fetchKiCadJson({ prepared, cwd: target.cwd, path, schema, method }).pipe(
                Effect.flatMap((response) => response.json),
                Effect.flatMap(Schema.decodeUnknownEffect(schema)),
              );
        })
        .pipe(
          Atom.swr({ staleTime: 1_000, revalidateOnMount: true }),
          Atom.setIdleTTL(5 * 60_000),
          Atom.withLabel(`${label}:${key}`),
        );
    });
    return (target: KiCadQueryTarget) => family(JSON.stringify(target));
  };
  return {
    session: query(
      "environment-data:kicad:viewer-session",
      "viewer-session",
      KiCadViewerSession,
      "POST",
    ),
    manifest: query("environment-data:kicad:manifest", "manifest", KiCadProjectManifest, "GET"),
  };
}
