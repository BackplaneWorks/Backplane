import { describe, expect, it, vi } from "vite-plus/test";
import { EnvironmentId, ThreadId, type IosNotificationRegistration } from "@t3tools/contracts";
import type { AgentAwarenessState } from "@t3tools/shared/agentAwareness";
import {
  createDirectIosPush,
  makeDirectActivityPayload,
  type RegisteredIosDevice,
} from "./DirectIosPush.ts";

const registration: IosNotificationRegistration = {
  deviceId: "phone",
  bundleId: "com.i2cjak.k3eda",
  apsEnvironment: "production",
  pushToken: "a".repeat(64),
  activityToken: "b".repeat(64),
  notificationsEnabled: true,
  liveActivitiesEnabled: true,
};
const state = (phase: AgentAwarenessState["phase"]): AgentAwarenessState => ({
  environmentId: EnvironmentId.make("environment"),
  threadId: ThreadId.make("thread"),
  projectTitle: "Project",
  threadTitle: "Task",
  modelTitle: "model",
  phase,
  headline: phase,
  updatedAt: "2026-09-07T12:00:00Z",
  deepLink: "/threads/environment/thread",
});
const setup = (restored: RegisteredIosDevice[] = []) => {
  const send = vi.fn(async () => true);
  const persist = vi.fn(async (_devices: readonly RegisteredIosDevice[]) => {});
  const onError = vi.fn();
  const push = createDirectIosPush({
    bundleId: registration.bundleId,
    restored,
    send,
    persist,
    onError,
  });
  return { push, send, persist, onError };
};
describe("direct iOS push", () => {
  it("reports missing Apple configuration without pretending to register", async () => {
    const persist = vi.fn();
    const push = createDirectIosPush({
      bundleId: registration.bundleId,
      restored: [],
      send: null,
      persist,
      onError: vi.fn(),
    });
    expect(await push.register(registration)).toEqual({ configured: false });
    expect(persist).not.toHaveBeenCalled();
  });
  it("rejects a device for another app", async () => {
    const { push, persist } = setup();
    await expect(push.register({ ...registration, bundleId: "another.app" })).rejects.toThrow(
      "bundle ID",
    );
    expect(persist).not.toHaveBeenCalled();
  });
  it("updates a card, sends one completion alert, and ends the card", async () => {
    const { push, send, persist } = setup();
    await push.register(registration);
    await push.publish([state("running")]);
    expect(send).toHaveBeenCalledTimes(1);
    expect(send.mock.calls[0]).toEqual([
      expect.objectContaining({ environment: "production", liveActivity: true }),
    ]);
    await push.publish([state("completed")]);
    expect(send).toHaveBeenCalledTimes(3);
    expect(send.mock.calls[1]).toEqual([
      expect.objectContaining({
        liveActivity: false,
        payload: expect.objectContaining({ deepLink: state("completed").deepLink }),
      }),
    ]);
    expect(send.mock.calls[2]).toEqual([
      expect.objectContaining({
        payload: expect.objectContaining({ aps: expect.objectContaining({ event: "end" }) }),
      }),
    ]);
    await push.publish([state("completed")]);
    expect(send).toHaveBeenCalledTimes(3);
    expect(persist.mock.lastCall?.[0][0]?.activityToken).toBeUndefined();
  });
  it("restores delivery after restart without alerting about old completed tasks", async () => {
    const { push, send } = setup([{ ...registration, expiresAt: 9_999_999_999_999 }]);
    await push.publish([state("completed")]);
    expect(send).toHaveBeenCalledTimes(1);
    expect(send.mock.calls[0]).toEqual([expect.objectContaining({ liveActivity: true })]);
    await push.publish([state("running")]);
    await push.publish([state("waiting_for_approval")]);
    expect(send).toHaveBeenCalledTimes(2);
    expect(send.mock.calls[1]).toEqual([expect.objectContaining({ liveActivity: false })]);
  });
  it("removes an invalid device token and preserves a rotated token", async () => {
    const { push, send, persist } = setup();
    await push.register({ ...registration, liveActivitiesEnabled: false });
    await push.publish([state("running")]);
    send.mockResolvedValueOnce(false);
    await push.publish([state("completed")]);
    expect(persist.mock.lastCall?.[0][0]?.pushToken).toBeUndefined();
    await push.register({
      ...registration,
      pushToken: "c".repeat(64),
      liveActivitiesEnabled: false,
    });
    expect(persist.mock.lastCall?.[0][0]?.pushToken).toBe("c".repeat(64));
  });
  it("disabling both surfaces stops delivery even when old tokens were registered", async () => {
    const { push, send, persist } = setup();
    await push.register(registration);
    await push.publish([state("running")]);
    await push.register({
      ...registration,
      notificationsEnabled: false,
      liveActivitiesEnabled: false,
    });
    send.mockClear();
    await push.publish([state("failed")]);
    expect(send).not.toHaveBeenCalled();
    expect(persist.mock.lastCall?.[0][0]?.pushToken).toBeUndefined();
    expect(persist.mock.lastCall?.[0][0]?.activityToken).toBeUndefined();
  });
  it("uses the Expo widget content-state format", () => {
    const payload = makeDirectActivityPayload([state("running")], 1000);
    expect(payload.aps["content-state"].name).toBe("AgentActivity");
    expect(JSON.parse(payload.aps["content-state"].props)).toMatchObject({
      activeCount: 1,
      activities: [{ phase: "running" }],
    });
  });
});
