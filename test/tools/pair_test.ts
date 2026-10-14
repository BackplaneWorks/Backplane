// The web page saves a pairing link's token before it clears the URL
// fragment. A first open of http://host:3787/#token=<hex> has to keep the
// token; a later visit with no fragment still has it.
//   bun test/tools/pair_test.ts
import { readFileSync } from "fs";
import { TOKEN_KEY, takeToken, tokenIn } from "../../src/web/pair.js";

let failed = false;
const check = (name: string, ok: boolean, got?: unknown) => {
  console.log(ok ? `ok ${name}` : `FAIL ${name}: ${JSON.stringify(got)}`);
  if (!ok) failed = true;
};

function mem() {
  const m = new Map<string, string>();
  return {
    setItem(k: string, v: string) { m.set(k, v); },
    getItem(k: string) { return m.has(k) ? m.get(k)! : null; },
  };
}

{
  const s = mem();
  const order: string[] = [];
  const storage = {
    setItem(k: string, v: string) { order.push("set"); s.setItem(k, v); },
    getItem(k: string) { order.push("get"); return s.getItem(k); },
  };
  const token = takeToken("#token=ab12", storage, () => order.push("clear"));
  const setAt = order.indexOf("set");
  const clearAt = order.indexOf("clear");
  check("a pairing fragment is saved before it is cleared", setAt !== -1 && clearAt !== -1 && setAt < clearAt && token === "ab12" && s.getItem(TOKEN_KEY) === "ab12", order);
}
{
  const s = mem();
  s.setItem(TOKEN_KEY, "kept");
  let cleared = false;
  const token = takeToken("", s, () => { cleared = true; });
  check("a return visit keeps the saved token and leaves the fragment", token === "kept" && !cleared);
}
{
  const s = mem();
  s.setItem(TOKEN_KEY, "kept");
  let cleared = false;
  const token = takeToken("#bot=kit", s, () => { cleared = true; });
  check("a bot fragment is cleared and leaves the saved token", cleared && token === "kept" && s.getItem(TOKEN_KEY) === "kept");
}
{
  check("tokenIn reads the hex", tokenIn("#token=ab12") === "ab12");
  check("tokenIn ignores a non-hex token", tokenIn("#token=ZZ") === "");
  check("tokenIn ignores an empty fragment", tokenIn("") === "");
}

{
  const src = readFileSync(new URL("../../src/web/host.js", import.meta.url), "utf8");
  const save = src.indexOf("takeToken(");
  const clear = src.indexOf("history.replaceState");
  check("host.js saves the pairing token before it clears the fragment", save !== -1 && save < clear, { save, clear });
  check("host.js does not read the token from the fragment after clearing it", !/history\.replaceState[\s\S]*location\.hash\.match\(\/token=/.test(src));
}

process.exit(failed ? 1 : 0);
