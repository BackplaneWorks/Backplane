import { useAtomQueryRunner } from "../../state/use-atom-query-runner";
import { kicadState } from "../../state/kicad";
import { EnvironmentId } from "@t3tools/contracts";
import { squashAtomCommandFailure } from "@t3tools/client-runtime/state/runtime";
import * as Option from "effect/Option";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ActivityIndicator, Linking, Pressable, View } from "react-native";
import { WebView } from "react-native-webview";
import type { StaticScreenProps } from "@react-navigation/native";

import { AppText as Text } from "../../components/AppText";
import { LoadingStrip } from "../../components/LoadingStrip";
import { NativeStackScreenOptions } from "../../native/StackHeader";
import { useAppearancePreferences } from "../settings/appearance/AppearancePreferencesProvider";
import { useUniwindTheme } from "../../lib/useUniwindTheme";
import { usePreparedConnection } from "../../state/session";

type Props = StaticScreenProps<{
  readonly environmentId: string;
  readonly threadId: string;
  readonly cwd: string;
}>;

export function KiCadViewerRouteScreen({ route }: Props) {
  const { cwd } = route.params;
  const environmentId = EnvironmentId.make(route.params.environmentId);
  const connection = usePreparedConnection(environmentId);
  const mintSession = useAtomQueryRunner(kicadState.session, {
    refresh: true,
    reportFailure: false,
  });
  const [session, setSession] = useState<{ token: string; expiresAt: number } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);
  const [progress, setProgress] = useState(0);
  const webViewRef = useRef<WebView>(null);
  const theme = useUniwindTheme();
  const { themeAppearance } = useAppearancePreferences();

  const retry = useCallback(() => setRefreshKey((key) => key + 1), []);
  useEffect(() => {
    const controller = new AbortController();
    setSession(null);
    setError(null);
    if (Option.isNone(connection)) {
      setError("Reconnect to this environment to open the KiCad viewer.");
      return () => controller.abort();
    }
    void mintSession({ environmentId, cwd })
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
  }, [connection, cwd, environmentId, mintSession, refreshKey]);

  useEffect(() => {
    if (!session) return;
    const remaining = session.expiresAt - Date.now() - 60_000;
    const timer = setTimeout(retry, Math.max(1_000, remaining));
    return () => clearTimeout(timer);
  }, [retry, session]);

  const viewerUrl = useMemo(() => {
    if (!session || Option.isNone(connection)) return null;
    const url = new URL(`${connection.value.httpBaseUrl.replace(/\/$/, "")}/kicad.html`);
    url.hash = new URLSearchParams({
      api: connection.value.httpBaseUrl,
      token: session.token,
      view: "pcb",
      embedded: "1",
    }).toString();
    return url.toString();
  }, [connection, session]);
  const viewerOrigin = useMemo(() => (viewerUrl ? new URL(viewerUrl).origin : null), [viewerUrl]);
  const themeScript = useMemo(() => {
    const variables = {
      "--background": theme["--color-screen"],
      "--foreground": theme["--color-foreground"],
      "--card": theme["--color-card"],
      "--card-foreground": theme["--color-foreground"],
      "--muted": theme["--color-subtle"],
      "--muted-foreground": theme["--color-foreground-muted"],
      "--accent": theme["--color-subtle-strong"],
      "--accent-foreground": theme["--color-foreground"],
      "--border": theme["--color-border"],
    };
    return `(() => { const vars = ${JSON.stringify(variables)}; for (const [key, value] of Object.entries(vars)) document.documentElement.style.setProperty(key, value); document.documentElement.classList.toggle("dark", ${themeAppearance === "dark"}); })(); true;`;
  }, [theme, themeAppearance]);

  useEffect(() => {
    if (viewerUrl) webViewRef.current?.injectJavaScript(themeScript);
  }, [themeScript, viewerUrl]);

  return (
    <View className="flex-1 bg-sheet">
      <NativeStackScreenOptions options={{ title: "KiCad" }} />
      {progress > 0 && progress < 1 ? <LoadingStrip progress={progress} /> : null}
      {viewerUrl ? (
        <WebView
          ref={webViewRef}
          source={{ uri: viewerUrl }}
          originWhitelist={viewerOrigin ? ["http://*", "https://*", "about:blank", "blob:*"] : []}
          allowsBackForwardNavigationGestures={false}
          allowsFullscreenVideo
          setSupportMultipleWindows={false}
          startInLoadingState
          injectedJavaScriptBeforeContentLoaded={themeScript}
          onShouldStartLoadWithRequest={(request) => {
            if (
              request.url === "about:blank" ||
              request.url.startsWith("blob:") ||
              (viewerOrigin !== null && request.url.startsWith(`${viewerOrigin}/`))
            )
              return true;
            if (/^https?:\/\//i.test(request.url)) {
              // Analysis dashboards load in their own frame; full-page links open externally.
              if (request.isTopFrame === false) return true;
              void Linking.openURL(request.url).catch(() => {
                setError("Unable to open the analysis dashboard in your browser.");
              });
              return false;
            }
            setError("Navigation outside the KiCad viewer was blocked.");
            return false;
          }}
          onLoadProgress={(event) => setProgress(event.nativeEvent.progress)}
          onLoadStart={() => setProgress(0.05)}
          onLoadEnd={() => {
            setProgress(0);
            webViewRef.current?.injectJavaScript(themeScript);
          }}
          onHttpError={(event) => {
            if (event.nativeEvent.url.split("#")[0] !== viewerUrl.split("#")[0]) return;
            setProgress(0);
            setSession(null);
            setError(`The KiCad viewer returned status ${event.nativeEvent.statusCode}.`);
          }}
          onError={() => {
            setProgress(0);
            setSession(null);
            setError("The KiCad viewer could not be loaded.");
          }}
          onContentProcessDidTerminate={() => {
            setSession(null);
            setError("The KiCad viewer stopped responding.");
          }}
          renderLoading={() => (
            <View className="absolute inset-0 items-center justify-center bg-sheet">
              <ActivityIndicator />
            </View>
          )}
          style={{ flex: 1, backgroundColor: "transparent" }}
        />
      ) : (
        <View className="flex-1 items-center justify-center gap-3 px-6">
          {error ? (
            <>
              <Text className="text-center text-sm text-foreground-muted">{error}</Text>
              <Pressable className="bg-primary px-4 py-2" onPress={retry}>
                <Text className="text-sm font-t3-bold text-primary-foreground">Retry</Text>
              </Pressable>
              <Text className="text-sm text-foreground-muted">
                Use the back button to leave the viewer.
              </Text>
            </>
          ) : (
            <>
              <ActivityIndicator />
              <Text className="text-sm text-foreground-muted">Opening KiCad viewer…</Text>
            </>
          )}
        </View>
      )}
    </View>
  );
}
