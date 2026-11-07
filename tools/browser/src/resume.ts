// Resuming the targets the engine creates paused (window.open popups) when the helper is attached over CDP.
/** A window the page opens (window.open) is a new target the host creates paused, waiting for a debugger; Playwright's attach to it waits on the target in turn, so
 *  neither goes on and the popup stays blank (its opener and the person's tab are fine). A second session of ours resumes every new target at once: harmless for one
 *  that is not waiting. */
export function resumePaused(endpoint: string): WebSocket {
  const ws = new WebSocket(endpoint); let n = 0;
  const send = (method: string, params: object = {}, sessionId?: string) => ws.send(JSON.stringify({ id: ++n, method, params, ...(sessionId ? { sessionId } : {}) }));
  ws.onopen = () => send("Target.setDiscoverTargets", { discover: true });
  const seen = new Set<string>();
  const resume = (targetId: string) => send("Target.attachToTarget", { targetId, flatten: true });
  ws.onmessage = (e) => {
    let m: any; try { m = JSON.parse(String(e.data)); } catch { return; }
    const ti = m.params?.targetInfo;
    if ((m.method === "Target.targetCreated" || m.method === "Target.targetInfoChanged") && ti && (ti.type === "page" || ti.type === "other")) {
      const key = `${ti.targetId}|${ti.type}|${ti.url}`;
      if (!seen.has(key)) { seen.add(key); resume(ti.targetId); for (const ms of [300, 1500]) setTimeout(() => { try { resume(ti.targetId); } catch {} }, ms); }
    }
    if (m.id && m.result?.sessionId) { send("Runtime.runIfWaitingForDebugger", {}, m.result.sessionId); setTimeout(() => { try { send("Target.detachFromTarget", { sessionId: m.result.sessionId }); } catch {} }, 1200); }
  };
  ws.onerror = () => {};
  return ws;
}

