// Run with Bun against the production phone bundle. No hub or device required.
import { readFileSync } from "node:fs";
const path = process.argv[2];
if (!path) throw new Error("bridge bundle path required");
(0, eval)(readFileSync(path, "utf8"));
const bp = (globalThis as any).Backplane;
function check(reply: string, tag: string, empty: string) {
  const { screen, cmds } = JSON.parse(reply);
  if (screen.locale !== tag || screen.empty !== empty || cmds.some((c: any) => c.type === "send"))
    throw new Error(JSON.stringify({ expected: tag, screen, cmds }));
}
const en = "Pair with a hub to start.", ja = "ハブとペアリングしてください。";
check(bp.start("native-fixture", "{}"), "en", en);
check(bp.local("lang-os", "ja-JP"), "en", en); // preview gate remains closed
check(bp.local("lang-preview", "1"), "ja", ja); // unpaired cache must invalidate
check(bp.local("lang", "en"), "en", en);
check(bp.local("lang", "ja"), "ja", ja);
check(bp.local("threadList", "active"), "ja", ja);
check(bp.local("lang", "auto"), "ja", ja);
check(bp.local("lang-os", "en-US"), "en", en);
console.log("ok native bridge: unpaired en/ja/en, OS/override/preview and cache identity");
