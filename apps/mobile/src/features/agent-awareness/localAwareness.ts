import type { AgentAwarenessState } from "@t3tools/shared/agentAwareness";
import type { AgentActivityProps } from "../../widgets/AgentActivity";

export function isActiveAwareness(state: AgentAwarenessState): boolean {
  return (
    state.phase === "starting" ||
    state.phase === "running" ||
    state.phase === "waiting_for_approval" ||
    state.phase === "waiting_for_input"
  );
}

/** Initial snapshots establish a baseline so opening the app never replays old alerts. */
export function advanceLocalAwareness(
  previous: ReadonlyMap<string, AgentAwarenessState["phase"]>,
  states: ReadonlyArray<AgentAwarenessState>,
) {
  const next = new Map(previous);
  const notifications: AgentAwarenessState[] = [];
  for (const state of states) {
    const key = JSON.stringify([state.environmentId, state.threadId]);
    const prior = previous.get(key);
    if (
      prior !== undefined &&
      prior !== state.phase &&
      ["completed", "failed", "waiting_for_approval", "waiting_for_input"].includes(state.phase)
    ) {
      notifications.push(state);
    }
    next.set(key, state.phase);
  }
  return { next, notifications };
}

export function localActivityProps(states: ReadonlyArray<AgentAwarenessState>): AgentActivityProps {
  const active = states
    .filter(isActiveAwareness)
    .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
  return {
    title: "Backplane",
    subtitle: active.length ? "Agent work in progress" : "Agent work finished",
    activeCount: active.length,
    updatedAt: active[0]?.updatedAt ?? states[0]?.updatedAt ?? new Date().toISOString(),
    activities: active.slice(0, 3).map((state) => ({ ...state, status: state.headline })),
  };
}
