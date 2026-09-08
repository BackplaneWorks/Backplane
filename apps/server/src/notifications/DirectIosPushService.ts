import {
  IosNotificationRegistration,
  type IosNotificationRegistrationResult,
} from "@t3tools/contracts";
import { projectThreadAwareness } from "@t3tools/shared/agentAwareness";
import { makeDrainableWorker } from "@t3tools/shared/DrainableWorker";
import * as Config from "effect/Config";
import * as Context from "effect/Context";
import * as Effect from "effect/Effect";
import * as FileSystem from "effect/FileSystem";
import * as Layer from "effect/Layer";
import * as Option from "effect/Option";
import * as Schema from "effect/Schema";
import * as Stream from "effect/Stream";
import { ServerSecretStore } from "../auth/ServerSecretStore.ts";
import { ServerEnvironment } from "../environment/ServerEnvironment.ts";
import { OrchestrationEngineService } from "../orchestration/Services/OrchestrationEngine.ts";
import { ProjectionSnapshotQuery } from "../orchestration/Services/ProjectionSnapshotQuery.ts";
import { shouldPublishAgentAwarenessEvent } from "../relay/AgentAwarenessRelay.ts";
import { forkParked } from "../serverActivation.ts";
import { createApnsTransport } from "./ApnsTransport.ts";
import { createDirectIosPush } from "./DirectIosPush.ts";

const StoredDevices = Schema.Array(
  Schema.Struct({ ...IosNotificationRegistration.fields, expiresAt: Schema.Number }),
);
const decodeStoredDevices = Schema.decodeUnknownEffect(Schema.fromJsonString(StoredDevices));
const encodeStoredDevices = Schema.encodeEffect(Schema.fromJsonString(StoredDevices));
const decodeApnsConfig = Schema.decodeUnknownEffect(
  Schema.fromJsonString(
    Schema.Struct({
      teamId: Schema.String,
      keyId: Schema.String,
      bundleId: Schema.String,
      keyPath: Schema.String,
    }),
  ),
);
const storeKey = "direct-ios-notifications";
export class DirectIosPushError extends Schema.TaggedErrorClass<DirectIosPushError>()(
  "DirectIosPushError",
  { cause: Schema.Defect() },
) {
  override get message() {
    return this.cause instanceof Error
      ? this.cause.message
      : "Direct iOS notification operation failed.";
  }
}
const failure = (cause: unknown) => new DirectIosPushError({ cause });
export class DirectIosPushService extends Context.Service<
  DirectIosPushService,
  {
    readonly register: (
      input: typeof IosNotificationRegistration.Type,
    ) => Effect.Effect<typeof IosNotificationRegistrationResult.Type, DirectIosPushError>;
  }
>()("t3/notifications/DirectIosPushService") {}

export const layer = Layer.effect(
  DirectIosPushService,
  Effect.gen(function* () {
    const secrets = yield* ServerSecretStore;
    const environment = yield* ServerEnvironment;
    const snapshots = yield* ProjectionSnapshotQuery;
    const engine = yield* OrchestrationEngineService;
    const fs = yield* FileSystem.FileSystem;
    const storedConfig = yield* secrets.get("direct-ios-apns-config");
    const defaults = Option.isSome(storedConfig)
      ? yield* decodeApnsConfig(new TextDecoder().decode(storedConfig.value))
      : { teamId: "", keyId: "", bundleId: "com.i2cjak.k3eda", keyPath: "" };
    const config = yield* Config.all({
      teamId: Config.string("T3CODE_APNS_TEAM_ID").pipe(Config.withDefault(defaults.teamId)),
      keyId: Config.string("T3CODE_APNS_KEY_ID").pipe(Config.withDefault(defaults.keyId)),
      bundleId: Config.string("T3CODE_APNS_BUNDLE_ID").pipe(Config.withDefault(defaults.bundleId)),
      keyPath: Config.string("T3CODE_APNS_PRIVATE_KEY_PATH").pipe(
        Config.withDefault(defaults.keyPath),
      ),
    });
    const privateKey = config.keyPath
      ? yield* fs
          .readFileString(config.keyPath)
          .pipe(
            Effect.catch((error) =>
              Effect.logWarning("Could not read the APNs signing key", error).pipe(Effect.as("")),
            ),
          )
      : "";
    const send =
      config.teamId && config.keyId && privateKey
        ? yield* Effect.try({
            try: () => createApnsTransport({ ...config, privateKey }),
            catch: failure,
          }).pipe(
            Effect.catch((error) =>
              Effect.logWarning("APNs signing key is invalid", error).pipe(Effect.as(null)),
            ),
          )
        : null;
    const stored = yield* secrets.get(storeKey);
    const restored = Option.isSome(stored)
      ? yield* decodeStoredDevices(new TextDecoder().decode(stored.value))
      : [];
    const push = createDirectIosPush({
      bundleId: config.bundleId,
      restored,
      send,
      persist: (devices) =>
        Effect.runPromise(
          encodeStoredDevices(devices).pipe(
            Effect.flatMap((json) => secrets.set(storeKey, new TextEncoder().encode(json))),
          ),
        ),
      onError: (error) => {
        Effect.runSync(Effect.logWarning("Direct iOS push delivery failed", error));
      },
    });
    const refresh = Effect.gen(function* () {
      if (!send || !push.hasDevices()) return;
      const environmentId = yield* environment.getEnvironmentId;
      const snapshot = yield* snapshots.getShellSnapshot();
      const projects = new Map(snapshot.projects.map((project) => [project.id, project]));
      const states = snapshot.threads.flatMap((thread) => {
        if (thread.archivedAt) return [];
        const project = projects.get(thread.projectId);
        const state = project && projectThreadAwareness({ environmentId, project, thread });
        return state ? [state] : [];
      });
      yield* Effect.tryPromise({ try: () => push.publish(states), catch: failure });
    }).pipe(
      Effect.catch((error) => Effect.logWarning("Direct iOS notification update failed", error)),
    );
    if (send) {
      const worker = yield* makeDrainableWorker(() => refresh);
      yield* forkParked(
        Effect.gen(function* () {
          yield* worker.enqueue(undefined);
          yield* Stream.runForEach(engine.streamDomainEvents, (event) =>
            shouldPublishAgentAwarenessEvent(event) ? worker.enqueue(undefined) : Effect.void,
          );
        }),
      );
    }
    return DirectIosPushService.of({
      register: (input) =>
        Effect.gen(function* () {
          const result = yield* Effect.tryPromise({
            try: () => push.register(input),
            catch: failure,
          });
          yield* refresh;
          return result;
        }),
    });
  }),
);
