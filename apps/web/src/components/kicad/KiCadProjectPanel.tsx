import type { KiCadViewerSession, ScopedThreadRef } from "@t3tools/contracts";
import { squashAtomCommandFailure } from "@t3tools/client-runtime/state/runtime";
import * as Option from "effect/Option";
import { ExternalLink, LoaderCircle } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import { kicadState } from "~/state/kicad";
import { useAtomQueryRunner } from "~/state/use-atom-query-runner";
import { usePreparedConnection } from "~/state/session";
import { previewEnvironment } from "~/state/preview";
import { useAtomCommand } from "~/state/use-atom-command";
import { openUrlInPreview } from "~/browser/openFileInPreview";

import { PreviewPanelShell, type PreviewPanelMode } from "../preview/PreviewPanelShell";

export interface KiCadProjectPanelProps {
  readonly mode: PreviewPanelMode;
  readonly threadRef: ScopedThreadRef;
  readonly projectPath: string | null;
}

export function KiCadProjectPanel({ mode, threadRef, projectPath }: KiCadProjectPanelProps) {
  const [session, setSession] = useState<KiCadViewerSession | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);
  const iframeRef = useRef<HTMLIFrameElement | null>(null);
  const connection = usePreparedConnection(threadRef.environmentId);
  const mintSession = useAtomQueryRunner(kicadState.session, {
    refresh: true,
    reportFailure: false,
  });
  useEffect(() => {
    const controller = new AbortController();
    setSession(null);
    setError(null);
    if (!projectPath) {
      setError("Open a KiCad project to inspect it here.");
      return () => controller.abort();
    }
    if (Option.isNone(connection)) {
      setError("Reconnect to this environment to open the KiCad viewer.");
      return () => controller.abort();
    }
    void mintSession({ environmentId: threadRef.environmentId, cwd: projectPath })
      .then((result) => {
        if (controller.signal.aborted) return;
        if (result._tag === "Failure") throw squashAtomCommandFailure(result);
        setSession(result.value);
      })
      .catch((cause: unknown) => {
        if (!controller.signal.aborted)
          setError(cause instanceof Error ? cause.message : "Unable to open the KiCad viewer.");
      });
    return () => controller.abort();
  }, [connection, mintSession, projectPath, refreshKey, threadRef.environmentId]);

  const sendTheme = useCallback(() => {
    const frame = iframeRef.current?.contentWindow;
    if (!frame) return;
    const styles = getComputedStyle(document.documentElement);
    const variables: Record<string, string> = {};
    for (let index = 0; index < styles.length; index += 1) {
      const key = styles.item(index);
      if (key.startsWith("--")) variables[key] = styles.getPropertyValue(key).trim();
    }
    // The host document is served by this client; the API origin in the hash
    // can be remote, but it is not the iframe's postMessage target.
    const targetOrigin = window.location.origin === "null" ? "*" : window.location.origin;
    frame.postMessage(
      {
        type: "k3eda-theme",
        dark: document.documentElement.classList.contains("dark"),
        variables,
      },
      targetOrigin,
    );
  }, []);

  useEffect(() => {
    const observer = new MutationObserver(sendTheme);
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["class", "style"],
    });
    return () => observer.disconnect();
  }, [sendTheme]);

  useEffect(() => {
    if (!session || session.expiresAt <= Date.now()) {
      if (session) setRefreshKey((key) => key + 1);
      return;
    }
    const timer = window.setTimeout(
      () => setRefreshKey((key) => key + 1),
      Math.max(1_000, session.expiresAt - Date.now() - 60_000),
    );
    return () => window.clearTimeout(timer);
  }, [session]);

  const openPreview = useAtomCommand(previewEnvironment.open, { reportFailure: false });
  const viewerUrl = session
    ? (() => {
        const url = new URL("/kicad.html", window.location.href);
        url.hash = new URLSearchParams({
          api: Option.isSome(connection) ? connection.value.httpBaseUrl : window.location.origin,
          token: session.token,
          view: "pcb",
          embedded: "1",
        }).toString();
        return url.toString();
      })()
    : null;
  const openInBrowser = useCallback(() => {
    if (!viewerUrl) return;
    const target = new URL(viewerUrl);
    // The browser surface cannot load the desktop renderer's private protocol.
    if (!/^https?:$/.test(target.protocol) && Option.isSome(connection)) {
      const hosted = new URL(`${connection.value.httpBaseUrl.replace(/\/$/, "")}/kicad.html`);
      hosted.hash = target.hash;
      void openUrlInPreview({ threadRef, url: hosted.toString(), openPreview });
    } else {
      void openUrlInPreview({ threadRef, url: viewerUrl, openPreview });
    }
  }, [connection, openPreview, threadRef, viewerUrl]);
  return (
    <PreviewPanelShell mode={mode} widthStorageKey="t3code:kicad-panel-width" defaultWidth={620}>
      <div className="flex h-full min-h-0 flex-col bg-background" data-kicad-panel>
        <div className="flex min-h-[var(--workspace-topbar-height)] shrink-0 items-center gap-2 border-b border-border px-3 text-xs">
          <span className="font-medium text-foreground">KiCad</span>
          <span className="min-w-0 flex-1 truncate text-muted-foreground">
            {projectPath?.split(/[\\/]/).filter(Boolean).at(-1) ?? "Current project"}
          </span>
          <span className="rounded border border-border px-1 py-0.5 text-[10px] uppercase tracking-wide text-muted-foreground">
            read-only
          </span>
          <button
            type="button"
            disabled={!viewerUrl}
            className="inline-flex size-7 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground disabled:opacity-40"
            onClick={openInBrowser}
            aria-label="Open KiCad viewer in browser"
          >
            <ExternalLink className="size-3.5" />
          </button>
        </div>
        <div className="relative min-h-0 flex-1 bg-[#101214]">
          {viewerUrl ? (
            <iframe
              ref={iframeRef}
              onLoad={sendTheme}
              title="KiCad project viewer"
              src={viewerUrl}
              className="block h-full w-full border-0"
            />
          ) : (
            <div className="absolute inset-0 flex items-center justify-center gap-2 px-6 text-center text-xs text-muted-foreground">
              {error ? (
                <>
                  <span>{error}</span>
                  <button
                    type="button"
                    className="rounded-md bg-accent px-2 py-1 text-foreground hover:bg-accent/80"
                    onClick={() => setRefreshKey((key) => key + 1)}
                  >
                    Retry
                  </button>
                </>
              ) : (
                <>
                  <LoaderCircle className="size-4 animate-spin" />
                  Opening viewer…
                </>
              )}
            </div>
          )}
        </div>
      </div>
    </PreviewPanelShell>
  );
}
