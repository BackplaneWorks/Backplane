// Run the existing Chrome acceptance tests unchanged, on one private Xvfb.
import { privateDisplay } from "./harness.ts";
const d = await privateDisplay();
try {
  for (const test of ["pages.ts", "record.ts"]) {
    const p = Bun.spawn(["bun", new URL(`../../browser/test/${test}`, import.meta.url).pathname], { env: d.env, stdout: "inherit", stderr: "inherit" });
    if (await p.exited !== 0) throw new Error(`${test} failed`);
  }
} finally { await d.cleanup(); }
