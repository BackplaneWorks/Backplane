import { describe, expect, it } from "vite-plus/test";
import { EnvironmentId, ThreadId } from "@t3tools/contracts";
import type { AgentAwarenessState } from "@t3tools/shared/agentAwareness";
import { advanceLocalAwareness, localActivityProps } from "./localAwareness";

const state = (phase: AgentAwarenessState["phase"], environment = "one"): AgentAwarenessState => ({
  environmentId: EnvironmentId.make(environment),
  threadId: ThreadId.make("thread"),
  projectTitle: "Project",
  threadTitle: "Task",
  modelTitle: "model",
  phase,
  headline: phase,
  updatedAt: "2026-09-07T12:00:00Z",
  deepLink: "/threads/one/thread",
});
describe("direct connection awareness", () => {
  it("does not replay completed history when opening the app", () => {
    expect(advanceLocalAwareness(new Map(), [state("completed")]).notifications).toEqual([]);
  });
  it("alerts once for each approval, input, completion and failure transition", () => {
    let previous = advanceLocalAwareness(new Map(), [state("running")]).next;
    for (const phase of [
      "waiting_for_approval",
      "waiting_for_input",
      "completed",
      "failed",
    ] as const) {
      const result = advanceLocalAwareness(previous, [state(phase)]);
      expect(result.notifications).toEqual([state(phase)]);
      expect(advanceLocalAwareness(result.next, [state(phase)]).notifications).toEqual([]);
      previous = result.next;
    }
  });
  it("keeps environments separate and retains the baseline across disconnects", () => {
    const first = advanceLocalAwareness(new Map(), [state("running")]);
    const disconnected = advanceLocalAwareness(first.next, []);
    const result = advanceLocalAwareness(disconnected.next, [
      state("completed"),
      state("completed", "two"),
    ]);
    expect(result.notifications).toEqual([state("completed")]);
  });
  it("ends the card after all work completes and includes tasks awaiting input", () => {
    expect(localActivityProps([state("completed")]).activeCount).toBe(0);
    const props = localActivityProps([
      state("waiting_for_input"),
      state("running", "two"),
      state("completed", "three"),
    ]);
    expect(props.activeCount).toBe(2);
    expect(props.activities.map((row) => row.phase)).toEqual(["waiting_for_input", "running"]);
  });
});
