import { beforeEach, expect, it, vi } from "vite-plus/test";
import { EnvironmentId } from "@t3tools/contracts";
import type { SavedRemoteConnection } from "../../lib/connection";

const mocks = vi.hoisted(() => ({
  permission: vi.fn(),
  token: vi.fn(),
  fetch: vi.fn(),
}));
vi.mock("expo-constants", () => ({
  default: {
    expoConfig: { ios: { bundleIdentifier: "com.i2cjak.k3eda" }, extra: { appVariant: "k3eda" } },
  },
}));
vi.mock("expo-notifications", () => ({
  getPermissionsAsync: mocks.permission,
  getDevicePushTokenAsync: mocks.token,
  IosAuthorizationStatus: { AUTHORIZED: 2, PROVISIONAL: 3, EPHEMERAL: 4 },
}));
vi.mock("../../persistence/imperative", () => ({
  loadOrCreateAgentAwarenessDeviceId: async () => "phone",
}));
const connection: SavedRemoteConnection = {
  environmentId: EnvironmentId.make("one"),
  environmentLabel: "One",
  httpBaseUrl: "https://one.test",
  wsBaseUrl: "wss://one.test",
  bearerToken: "bearer",
  pairingUrl: "https://one.test",
  displayUrl: "https://one.test",
};
beforeEach(() => {
  vi.resetModules();
  vi.clearAllMocks();
  mocks.permission.mockResolvedValue({ granted: true });
  mocks.token.mockResolvedValue({ data: "a".repeat(64) });
  mocks.fetch.mockImplementation(async () => new Response(JSON.stringify({ configured: true })));
  vi.stubGlobal("fetch", mocks.fetch);
});
it("shares the native token request across overlapping environment registrations", async () => {
  const { registerDirectPush } = await import("./directRegistration");
  await Promise.all([
    registerDirectPush({ connection, liveActivitiesEnabled: true }),
    registerDirectPush({
      connection: { ...connection, environmentId: EnvironmentId.make("two") },
      liveActivitiesEnabled: true,
    }),
  ]);
  expect(mocks.token).toHaveBeenCalledTimes(1);
  expect(mocks.fetch).toHaveBeenCalledTimes(2);
  expect(JSON.parse(mocks.fetch.mock.calls[0]![1].body).pushToken).toBe("a".repeat(64));
});
it("still registers a Live Activity when the notification token request fails", async () => {
  mocks.token.mockRejectedValue(new Error("Native APNs unavailable"));
  const { registerDirectPush, getDirectPushStatuses, getDirectPushErrors } =
    await import("./directRegistration");
  await registerDirectPush({
    connection,
    liveActivitiesEnabled: true,
    activityToken: "b".repeat(64),
  });
  expect(JSON.parse(mocks.fetch.mock.calls[0]![1].body).activityToken).toBe("b".repeat(64));
  expect(getDirectPushStatuses().get("one")).toBe("failed");
  expect(getDirectPushErrors().get("one")).toContain("Native APNs unavailable");
});
it("accepts provisional iOS permission instead of disabling delivery", async () => {
  mocks.permission.mockResolvedValue({ granted: false, ios: { status: 3 } });
  const { registerDirectPush } = await import("./directRegistration");
  await registerDirectPush({ connection, liveActivitiesEnabled: true });
  expect(JSON.parse(mocks.fetch.mock.calls[0]![1].body).notificationsEnabled).toBe(true);
});
