import { useAtomSet, useAtomValue } from "@effect/atom-react";
import { useEffect, useState, useSyncExternalStore } from "react";
import { Alert, AppState, Linking, Platform } from "react-native";
import * as Notifications from "expo-notifications";
import { AsyncResult } from "effect/unstable/reactivity";
import { mobilePreferencesAtom, updateMobilePreferencesAtom } from "../../state/preferences";
import { SettingsSection } from "../settings/components/SettingsSection";
import { SettingsSwitchRow } from "../settings/components/SettingsSwitchRow";
import {
  getDirectPushStatuses,
  getDirectPushErrors,
  refreshDirectPush,
  subscribeDirectPushStatus,
} from "./directRegistration";
import { useSavedRemoteConnections } from "../../state/use-remote-environment-registry";
import { AppText as Text } from "../../components/AppText";
import { notificationPermissionGranted } from "./notificationPermissionGranted";
import { supportsAgentAwarenessPush } from "./capabilities";

export function LocalNotificationSettings() {
  const errors = useSyncExternalStore(subscribeDirectPushStatus, getDirectPushErrors);
  const statuses = useSyncExternalStore(subscribeDirectPushStatus, getDirectPushStatuses);
  const { savedConnectionsById } = useSavedRemoteConnections();
  const connections = Object.values(savedConnectionsById);
  const preferences = useAtomValue(mobilePreferencesAtom);
  const save = useAtomSet(updateMobilePreferencesAtom);
  const [granted, setGranted] = useState(false);
  const [busy, setBusy] = useState(true);
  useEffect(() => {
    if (Platform.OS !== "ios") return;
    const refresh = () => {
      void Notifications.getPermissionsAsync()
        .then((result) => setGranted(notificationPermissionGranted(result)))
        .catch(() => setGranted(false))
        .finally(() => setBusy(false));
    };
    refresh();
    const subscription = AppState.addEventListener("change", (state) => {
      if (state === "active") refresh();
    });
    return () => subscription.remove();
  }, []);
  if (Platform.OS !== "ios") return null;
  return (
    <SettingsSection title="Notifications">
      <SettingsSwitchRow
        icon="bell"
        label="Notifications"
        value={granted}
        disabled={busy}
        subtitle="Completion, approval, and input alerts while connected."
        onValueChange={(enabled) => {
          if (!enabled) {
            void Linking.openSettings();
            return;
          }
          setBusy(true);
          void Notifications.requestPermissionsAsync({
            ios: { allowAlert: true, allowBadge: true, allowSound: true },
          })
            .then((result) => {
              setGranted(notificationPermissionGranted(result));
              refreshDirectPush();
              if (!notificationPermissionGranted(result))
                Alert.alert(
                  "Notifications are disabled",
                  "Enable notifications for Backplane in iOS Settings.",
                  [
                    { text: "Cancel", style: "cancel" },
                    {
                      text: "Open Settings",
                      onPress: () => {
                        void Linking.openSettings();
                      },
                    },
                  ],
                );
            })
            .catch(() =>
              Alert.alert("Could not enable notifications", "Try again in iOS Settings."),
            )
            .finally(() => setBusy(false));
        }}
      />
      <SettingsSwitchRow
        icon="waveform"
        label="Live Activities"
        value={
          supportsAgentAwarenessPush() &&
          AsyncResult.isSuccess(preferences) &&
          preferences.value.liveActivitiesEnabled !== false
        }
        disabled={!supportsAgentAwarenessPush() || !AsyncResult.isSuccess(preferences)}
        subtitle="Show running tasks on the Lock Screen and Dynamic Island. Background updates require server push delivery."
        onValueChange={(liveActivitiesEnabled) => save({ liveActivitiesEnabled })}
      />
      {connections.map((connection) => (
        <Text key={connection.environmentId} className="px-4 pb-3 text-sm text-foreground-muted">
          {connection.environmentLabel}:{" "}
          {errors.get(`${connection.environmentId}:activity`) ||
            errors.get(connection.environmentId) ||
            null}{" "}
          {statuses.get(connection.environmentId) === "registered"
            ? "Background push registered."
            : statuses.get(connection.environmentId) === "failed"
              ? "Push registration failed. Reconnect to retry."
              : statuses.get(connection.environmentId) === "unconfigured"
                ? "Background push needs Apple push setup on this server."
                : "Checking background push setup…"}
        </Text>
      ))}
    </SettingsSection>
  );
}
