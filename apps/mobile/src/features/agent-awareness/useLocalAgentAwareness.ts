import { useAtomValue } from "@effect/atom-react";
import { projectThreadAwareness, type AgentAwarenessState } from "@t3tools/shared/agentAwareness";
import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { AppState, Platform } from "react-native";
import * as Notifications from "expo-notifications";
import type { LiveActivity } from "expo-widgets";
import { AsyncResult, Atom } from "effect/unstable/reactivity";
import { useProjects } from "../../state/entities";
import { environmentThreadShells } from "../../state/threads";
import { environmentShell } from "../../state/shell";
import { mobilePreferencesAtom } from "../../state/preferences";
import { useSavedRemoteConnections } from "../../state/use-remote-environment-registry";
import AgentActivity, { type AgentActivityProps } from "../../widgets/AgentActivity";
import { hasCloudPublicConfig } from "../cloud/publicConfig";
import { supportsAgentAwarenessPush } from "./capabilities";
import { onLocalActivityArm } from "./localActivityArm";
import { notificationPermissionGranted } from "./notificationPermissionGranted";
import { advanceLocalAwareness, localActivityProps } from "./localAwareness";
import {
  getDirectPushStatuses,
  setDirectPushError,
  onDirectPushRefresh,
  registerDirectPush,
  subscribeDirectPushStatus,
} from "./directRegistration";

const liveThreadsAtom = Atom.make((get) =>
  get(environmentThreadShells.threadShellsAtom).filter(
    (thread) =>
      !thread.archivedAt &&
      get(environmentShell.stateValueAtom(thread.environmentId)).status === "live",
  ),
);

Notifications.setNotificationHandler({
  handleNotification: async () => ({
    shouldShowBanner: true,
    shouldShowList: true,
    shouldPlaySound: true,
    shouldSetBadge: false,
  }),
});

type LocalActivity = {
  activity: LiveActivity<AgentActivityProps>;
  identity: string;
  subscription: { remove(): void };
};

/** Each direct environment owns its card so independent servers never overwrite one another. */
export function useLocalAgentAwareness() {
  const threads = useAtomValue(liveThreadsAtom);
  const projects = useProjects();
  const preferences = useAtomValue(mobilePreferencesAtom);
  const { savedConnectionsById } = useSavedRemoteConnections();
  const statuses = useSyncExternalStore(subscribeDirectPushStatus, getDirectPushStatuses);
  const previous = useRef(new Map<string, AgentAwarenessState["phase"]>());
  const activities = useRef(new Map<string, LocalActivity>());
  const initialized = useRef(false);
  const activityQueue = useRef(Promise.resolve());
  const [appState, setAppState] = useState(AppState.currentState);
  const enabled =
    AsyncResult.isSuccess(preferences) && preferences.value.liveActivitiesEnabled !== false;
  const ready = AsyncResult.isSuccess(preferences);
  const local = Platform.OS === "ios" && !hasCloudPublicConfig();
  const current = useRef({ savedConnectionsById, enabled });
  useEffect(() => {
    current.current = { savedConnectionsById, enabled };
  }, [savedConnectionsById, enabled]);

  const register = async (environmentId: string, activityToken?: string, pushToken?: string) => {
    const connection = Object.values(current.current.savedConnectionsById).find(
      (item) => item.environmentId === environmentId,
    );
    if (!connection) return;
    try {
      await registerDirectPush({
        connection,
        liveActivitiesEnabled: current.current.enabled,
        ...(activityToken ? { activityToken } : {}),
        ...(pushToken ? { pushToken } : {}),
      });
    } catch (error) {
      console.warn("Direct iOS push registration failed", error);
    }
  };
  const registerRef = useRef(register);
  useEffect(() => {
    registerRef.current = register;
  });

  useEffect(
    () =>
      onLocalActivityArm((input) => {
        if (
          !local ||
          !current.current.enabled ||
          AppState.currentState !== "active" ||
          activities.current.has(input.environmentId)
        )
          return;
        try {
          const updatedAt = new Date().toISOString();
          const props: AgentActivityProps = {
            title: "Backplane",
            subtitle: "Agent work in progress",
            activeCount: 1,
            updatedAt,
            activities: [
              {
                ...input,
                threadId: "",
                modelTitle: "",
                phase: "starting",
                status: "Starting agent",
                updatedAt,
                deepLink: "/",
              },
            ],
          };
          const activity = AgentActivity.start(props, undefined, new Date(Date.now() + 300_000));
          initialized.current = true;
          const subscription = activity.addPushTokenListener((event) => {
            void registerRef.current(input.environmentId, event.pushToken);
          });
          activities.current.set(input.environmentId, {
            activity,
            identity: JSON.stringify(props),
            subscription,
          });
          void activity
            .getPushToken()
            .then((token) => registerRef.current(input.environmentId, token ?? undefined))
            .catch((error: unknown) =>
              setDirectPushError(`${input.environmentId}:activity`, String(error)),
            );
        } catch (error) {
          setDirectPushError(`${input.environmentId}:activity`, String(error));
        }
      }),
    [local],
  );

  useEffect(() => {
    const listener = AppState.addEventListener("change", setAppState);
    return () => listener.remove();
  }, []);
  useEffect(() => {
    if (!local || !ready || !supportsAgentAwarenessPush()) return;
    const refresh = (pushToken?: string) => {
      for (const connection of Object.values(current.current.savedConnectionsById)) {
        void (async () => {
          const activityToken = await activities.current
            .get(connection.environmentId)
            ?.activity.getPushToken();
          await registerRef.current(
            connection.environmentId,
            activityToken ?? undefined,
            pushToken,
          );
        })().catch((error: unknown) => console.warn("Live Activity token refresh failed", error));
      }
    };
    const removeRefresh = onDirectPushRefresh(refresh);
    const tokenSubscription = Notifications.addPushTokenListener((token) =>
      refresh(String(token.data)),
    );
    if (appState === "active") refresh();
    return () => {
      removeRefresh();
      tokenSubscription.remove();
    };
  }, [local, ready, enabled, savedConnectionsById, appState]);

  useEffect(() => {
    if (!local) return;
    const states = threads.flatMap((thread) => {
      const project = projects.find(
        (item) => item.environmentId === thread.environmentId && item.id === thread.projectId,
      );
      const state =
        project && projectThreadAwareness({ environmentId: thread.environmentId, project, thread });
      return state ? [state] : [];
    });
    const { next, notifications } = advanceLocalAwareness(previous.current, states);
    previous.current = next;
    for (const state of notifications) {
      // Configured servers own delivery, including foreground banners.
      if (statuses.get(state.environmentId) === "registered") continue;
      void Notifications.getPermissionsAsync()
        .then((permission) => {
          if (!notificationPermissionGranted(permission)) return;
          return Notifications.scheduleNotificationAsync({
            content: {
              title: state.headline,
              body: `${state.projectTitle} · ${state.threadTitle}`,
              sound: "default",
              data: {
                environmentId: state.environmentId,
                threadId: state.threadId,
                deepLink: state.deepLink,
              },
            },
            trigger: null,
          });
        })
        .catch((error: unknown) => console.warn("Local agent notification failed", error));
    }
    if (!supportsAgentAwarenessPush() || !ready) return;
    let cancelled = false;
    activityQueue.current = activityQueue.current
      .then(async () => {
        if (cancelled) return;
        // A fresh JS runtime cannot recover the environment associated with native IDs.
        // Reconcile once from a live snapshot before recreating the current cards.
        if (!initialized.current && (threads.length > 0 || !enabled)) {
          await Promise.all(
            AgentActivity.getInstances().map((activity) => activity.end("immediate")),
          );
          initialized.current = true;
        }
        if (cancelled) return;
        const environments = new Set([
          ...activities.current.keys(),
          ...threads.map((thread) => thread.environmentId),
        ]);
        for (const environmentId of environments) {
          const existing = activities.current.get(environmentId);
          const connected = Object.values(savedConnectionsById).some(
            (item) => item.environmentId === environmentId,
          );
          const live = threads.some((thread) => thread.environmentId === environmentId);
          // Disconnection is not completion. Leave its existing card to APNs.
          if (enabled && connected && !live) continue;
          const props = localActivityProps(
            states.filter((state) => state.environmentId === environmentId),
          );
          const identity = JSON.stringify(props);
          if (!enabled || !connected || props.activeCount === 0) {
            if (existing) {
              await existing.activity.end("immediate");
              existing.subscription.remove();
              activities.current.delete(environmentId);
            }
            continue;
          }
          const stillExists =
            existing &&
            AgentActivity.getInstances().some((item) => item.getId() === existing.activity.getId());
          if (existing && stillExists) {
            if (existing.identity !== identity) {
              await existing.activity.update(props, new Date(Date.now() + 300_000));
              existing.identity = identity;
            }
          } else if (appState === "active" && AppState.currentState === "active") {
            existing?.subscription.remove();
            const activity = AgentActivity.start(props, undefined, new Date(Date.now() + 300_000));
            const subscription = activity.addPushTokenListener((event) => {
              void registerRef.current(environmentId, event.pushToken);
            });
            activities.current.set(environmentId, { activity, identity, subscription });
            void activity
              .getPushToken()
              .then((token) => registerRef.current(environmentId, token ?? undefined))
              .catch((error: unknown) => console.warn("Live Activity token read failed", error));
          }
        }
      })
      .catch((error: unknown) => {
        for (const connection of Object.values(savedConnectionsById))
          setDirectPushError(
            `${connection.environmentId}:activity`,
            `Live Activity: ${String(error)}`,
          );
        console.warn("Local Live Activity update failed", error);
      });
    return () => {
      cancelled = true;
    };
  }, [threads, projects, enabled, ready, appState, local, savedConnectionsById, statuses]);

  useEffect(
    () => () => {
      for (const { subscription } of activities.current.values()) subscription.remove();
    },
    [],
  );
}
