import type { EnvironmentId } from "@t3tools/contracts";
export interface LocalActivityArm {
  readonly environmentId: EnvironmentId;
  readonly threadTitle: string;
  readonly projectTitle: string;
}
const listeners = new Set<(input: LocalActivityArm) => void>();
export function armLocalActivity(input: LocalActivityArm) {
  for (const listener of listeners) listener(input);
}
export function onLocalActivityArm(listener: (input: LocalActivityArm) => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
