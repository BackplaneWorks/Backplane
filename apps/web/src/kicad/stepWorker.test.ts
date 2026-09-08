import * as NodeFSP from "node:fs/promises";
import * as NodeVM from "node:vm";
import { expect, it } from "vite-plus/test";

it("loads the STEP WebAssembly from the importer's directory inside a worker", async () => {
  const base = new URL("../../public/kicad-viewer/", import.meta.url);
  const [source, importer, wasm] = await Promise.all([
    NodeFSP.readFile(new URL("step-worker.js", base), "utf8"),
    NodeFSP.readFile(new URL("occt/occt-import-js.js", base), "utf8"),
    NodeFSP.readFile(new URL("occt/occt-import-js.wasm", base)),
  ]);
  const requests: string[] = [];
  const messages: Array<{ error?: string }> = [];
  const self = {
    location: { href: "https://example.test/kicad-viewer/step-worker.js" },
    postMessage: (message: { error?: string }) => messages.push(message),
    onmessage: async (_event: { data: ArrayBuffer }) => {},
  };
  const context = NodeVM.createContext({
    self,
    URL,
    WebAssembly,
    TextDecoder,
    TextEncoder,
    Uint8Array,
    Int8Array,
    Uint16Array,
    Int16Array,
    Uint32Array,
    Int32Array,
    Float32Array,
    Float64Array,
    ArrayBuffer,
    DataView,
    setTimeout,
    clearTimeout,
    performance,
    console: { log() {}, warn() {}, error() {} },
    fetch: async (url: string) => {
      requests.push(url);
      return url === "https://example.test/kicad-viewer/occt/occt-import-js.wasm"
        ? new Response(wasm, { headers: { "Content-Type": "application/wasm" } })
        : new Response("<!doctype html>", { headers: { "Content-Type": "text/html" } });
    },
  });
  context.importScripts = () => NodeVM.runInContext(importer, context);
  NodeVM.runInContext(source, context);
  await self.onmessage({ data: new ArrayBuffer(0) });
  expect(requests).toEqual(["https://example.test/kicad-viewer/occt/occt-import-js.wasm"]);
  // Reaching the parser's empty-input error proves that the real WASM module initialized.
  expect(messages).toEqual([{ error: "No solid geometry could be read from this STEP file." }]);
});
