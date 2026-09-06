import { createKiCadState } from "@t3tools/client-runtime/state/kicad";
import { connectionAtomRuntime } from "../connection/runtime";

export const kicadState = createKiCadState(connectionAtomRuntime);
