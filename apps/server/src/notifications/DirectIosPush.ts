// @effect-diagnostics globalDate:off
import type { IosNotificationRegistration } from "@t3tools/contracts";
import type { AgentAwarenessState } from "@t3tools/shared/agentAwareness";
import type { ApnsMessage } from "./ApnsTransport.ts";

export interface RegisteredIosDevice extends IosNotificationRegistration {
  readonly expiresAt: number;
}
const active = (state: AgentAwarenessState) =>
  ["starting", "running", "waiting_for_approval", "waiting_for_input"].includes(state.phase);
export function makeDirectActivityPayload(
  states: readonly AgentAwarenessState[],
  timestamp: number,
) {
  const running = states.filter(active).sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
  const props = {
    title: "T3CAD",
    subtitle: running.length ? "Agent work in progress" : "Agent work finished",
    activeCount: running.length,
    updatedAt: new Date(timestamp * 1000).toISOString(),
    activities: running.slice(0, 3).map((state) => ({ ...state, status: state.headline })),
  };
  return {
    aps: {
      timestamp,
      event: running.length ? "update" : "end",
      "content-state": { name: "AgentActivity", props: JSON.stringify(props) },
      ...(running.length ? { "stale-date": timestamp + 300 } : { "dismissal-date": timestamp }),
    },
  };
}

/** Serializes registration, delivery, and token removal so a rotated token cannot be lost. */
export function createDirectIosPush(input: {
  bundleId: string;
  restored: readonly RegisteredIosDevice[];
  send: ((message: ApnsMessage) => Promise<boolean>) | null;
  persist: (devices: readonly RegisteredIosDevice[]) => Promise<void>;
  onError: (error: unknown) => void;
}) {
  const devices = new Map(input.restored.map((device) => [device.deviceId, device]));
  let states: readonly AgentAwarenessState[] = [];
  let initialized = false;
  let tail = Promise.resolve();
  const serial = <T>(operation: () => Promise<T>): Promise<T> => {
    const result = tail.then(operation);
    tail = result.then(
      () => undefined,
      () => undefined,
    );
    return result;
  };
  const persist = () => input.persist([...devices.values()]);
  const sendActivity = async (device: RegisteredIosDevice) => {
    if (!input.send || !device.liveActivitiesEnabled || !device.activityToken) return;
    const payload = makeDirectActivityPayload(states, Math.floor(Date.now() / 1000));
    const accepted = await input.send({
      token: device.activityToken,
      environment: device.apsEnvironment,
      liveActivity: true,
      payload,
    });
    if (!accepted || payload.aps.event === "end") {
      const { activityToken: _, ...rest } = device;
      devices.set(device.deviceId, rest);
      await persist();
    }
  };
  return {
    configured: input.send !== null,
    hasDevices: () => devices.size > 0,
    register: (registration: IosNotificationRegistration) =>
      serial(async () => {
        if (!input.send) return { configured: false };
        if (registration.bundleId !== input.bundleId)
          throw new Error("The app bundle ID does not match this server's APNs configuration.");
        for (const [key, device] of devices)
          if (device.expiresAt <= Date.now()) devices.delete(key);
        if (!devices.has(registration.deviceId) && devices.size >= 64)
          throw new Error("Too many registered iOS devices.");
        // Omitting a token preserves it across app foregrounds; disabling a surface removes it.
        const previous = devices.get(registration.deviceId);
        if (!registration.notificationsEnabled && !registration.liveActivitiesEnabled) {
          if (previous) {
            if (previous.activityToken) {
              await input.send({
                token: previous.activityToken,
                environment: previous.apsEnvironment,
                liveActivity: true,
                payload: makeDirectActivityPayload([], Math.floor(Date.now() / 1000)),
              });
            }
            devices.delete(registration.deviceId);
            await persist();
          }
          return { configured: true };
        }
        const pushToken = registration.pushToken ?? previous?.pushToken;
        const activityToken = registration.activityToken ?? previous?.activityToken;
        const { pushToken: _push, activityToken: _activity, ...preferences } = registration;
        const device: RegisteredIosDevice = {
          ...preferences,
          ...(registration.notificationsEnabled && pushToken ? { pushToken } : {}),
          ...(registration.liveActivitiesEnabled && activityToken ? { activityToken } : {}),
          expiresAt: Date.now() + 30 * 86400_000,
        };
        devices.set(registration.deviceId, device);
        await persist();
        if (initialized) await sendActivity(device);
        return { configured: true };
      }),
    publish: (next: readonly AgentAwarenessState[]) =>
      serial(async () => {
        const previous = new Map(states.map((state) => [state.threadId, state]));
        const changed = JSON.stringify(states) !== JSON.stringify(next);
        const alerts = initialized
          ? next.filter((state) => {
              const prior = previous.get(state.threadId);
              return (
                prior &&
                prior.phase !== state.phase &&
                ["completed", "failed", "waiting_for_approval", "waiting_for_input"].includes(
                  state.phase,
                )
              );
            })
          : [];
        states = next;
        initialized = true;
        if (!input.send || !changed) return;
        for (const [id, device] of devices) {
          if (device.expiresAt <= Date.now()) {
            devices.delete(id);
            await persist();
            continue;
          }
          try {
            if (device.notificationsEnabled && device.pushToken) {
              for (const state of alerts) {
                const accepted = await input.send({
                  token: device.pushToken,
                  environment: device.apsEnvironment,
                  liveActivity: false,
                  payload: {
                    aps: {
                      alert: {
                        title: state.headline,
                        body: `${state.projectTitle} · ${state.threadTitle}`,
                      },
                      sound: "default",
                    },
                    environmentId: state.environmentId,
                    threadId: state.threadId,
                    deepLink: state.deepLink,
                  },
                });
                if (!accepted) {
                  const { pushToken: _, ...rest } = device;
                  devices.set(id, rest);
                  await persist();
                  break;
                }
              }
            }
            await sendActivity(devices.get(id)!);
          } catch (error) {
            input.onError(error);
          }
        }
      }),
  };
}
