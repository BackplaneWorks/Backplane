import Constants from "expo-constants";
import * as Notifications from "expo-notifications";
import {
  IosNotificationRegistrationResult,
  type IosNotificationRegistration,
} from "@t3tools/contracts";
import * as Schema from "effect/Schema";
import type { SavedRemoteConnection } from "../../lib/connection";
import { loadOrCreateAgentAwarenessDeviceId } from "../../persistence/imperative";
import { notificationPermissionGranted } from "./notificationPermissionGranted";
import { resolveApsEnvironment } from "./registrationPayload";

const decodeResult = Schema.decodeUnknownSync(IosNotificationRegistrationResult);
export type DirectPushStatus = "checking" | "registered" | "unconfigured" | "failed";
let statuses: ReadonlyMap<string, DirectPushStatus> = new Map();
let errors: ReadonlyMap<string, string> = new Map();
export const getDirectPushErrors = () => errors;
export function setDirectPushError(environmentId: string, error: string) {
  errors = new Map(errors).set(environmentId, error);
  for (const listener of listeners) listener();
}
let nativeTokenRequest: Promise<string> | null = null;
let cachedPushToken: string | undefined;
async function readPushToken() {
  if (cachedPushToken) return cachedPushToken;
  if (!nativeTokenRequest) {
    nativeTokenRequest = Notifications.getDevicePushTokenAsync()
      .then((token) => {
        cachedPushToken = String(token.data);
        return cachedPushToken;
      })
      .finally(() => {
        nativeTokenRequest = null;
      });
  }
  return nativeTokenRequest;
}
const pending = new Map<string, Promise<unknown>>();
const listeners = new Set<() => void>();
const refreshListeners = new Set<() => void>();
export const getDirectPushStatuses = () => statuses;
export function subscribeDirectPushStatus(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
export function setDirectPushStatus(environmentId: string, status: DirectPushStatus) {
  if (statuses.get(environmentId) === status) return;
  statuses = new Map(statuses).set(environmentId, status);
  for (const listener of listeners) listener();
}
export function onDirectPushRefresh(listener: () => void) {
  refreshListeners.add(listener);
  return () => {
    refreshListeners.delete(listener);
  };
}
export function refreshDirectPush() {
  for (const listener of refreshListeners) listener();
}

type RegistrationInput = {
  connection: SavedRemoteConnection;
  liveActivitiesEnabled: boolean;
  activityToken?: string;
  pushToken?: string;
};
export function registerDirectPush(input: RegistrationInput) {
  if (input.pushToken) cachedPushToken = input.pushToken;
  const key = input.connection.environmentId;
  const result = (pending.get(key) ?? Promise.resolve())
    .catch(() => undefined)
    .then(() => registerNow(input));
  pending.set(key, result);
  void result
    .finally(() => {
      if (pending.get(key) === result) pending.delete(key);
    })
    .catch(() => undefined);
  return result;
}
async function registerNow(input: RegistrationInput) {
  const { connection } = input;
  if (!connection.bearerToken) return false;
  const permission = await Notifications.getPermissionsAsync();
  const granted = notificationPermissionGranted(permission);
  let token = input.pushToken;
  let tokenError: string | null = null;
  if (granted && !token) {
    let timer: ReturnType<typeof setTimeout> | undefined;
    try {
      token = await Promise.race([
        readPushToken(),
        new Promise<never>((_resolve, reject) => {
          timer = setTimeout(
            () =>
              reject(
                new Error(
                  "Apple has not returned a notification token. Reopen Backplane to retry.",
                ),
              ),
            10_000,
          );
        }),
      ]);
    } catch (error) {
      tokenError = error instanceof Error ? error.message : String(error);
    } finally {
      clearTimeout(timer);
    }
  }
  const body: IosNotificationRegistration = {
    deviceId: await loadOrCreateAgentAwarenessDeviceId(),
    bundleId: Constants.expoConfig?.ios?.bundleIdentifier ?? "com.i2cjak.k3eda",
    apsEnvironment: resolveApsEnvironment(Constants.expoConfig?.extra?.appVariant),
    notificationsEnabled: granted,
    liveActivitiesEnabled: input.liveActivitiesEnabled,
    ...(token ? { pushToken: token } : {}),
    ...(input.activityToken ? { activityToken: input.activityToken } : {}),
  };
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 15_000);
  try {
    const response = await fetch(
      `${connection.httpBaseUrl.replace(/\/$/, "")}/api/notifications/ios`,
      {
        method: "POST",
        headers: {
          Authorization: `Bearer ${connection.bearerToken}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify(body),
        signal: controller.signal,
      },
    );
    if (response.status === 404) {
      setDirectPushStatus(connection.environmentId, "unconfigured");
      return false;
    }
    if (!response.ok) throw new Error(`iOS push registration failed (${response.status}).`);
    const result = decodeResult(await response.json());
    setDirectPushStatus(
      connection.environmentId,
      result.configured ? (tokenError ? "failed" : "registered") : "unconfigured",
    );
    setDirectPushError(connection.environmentId, tokenError ?? "");
    return result.configured;
  } catch (error) {
    setDirectPushStatus(connection.environmentId, "failed");
    setDirectPushError(
      connection.environmentId,
      error instanceof Error ? error.message : String(error),
    );
    throw error;
  } finally {
    clearTimeout(timeout);
  }
}
