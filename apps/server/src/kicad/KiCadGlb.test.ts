// @effect-diagnostics nodeBuiltinImport:off
import { Buffer } from "node:buffer";
import { expect, it } from "vite-plus/test";
import { nameKiCadGlbLayers } from "./KiCadGlb.ts";

it("preserves binary geometry and component names while identifying board layers", () => {
  const json = Buffer.from(
    JSON.stringify({
      meshes: [{ name: "board_soldermask" }, { name: "board_copper" }, { name: "body" }],
      nodes: [
        { name: "=>0117", mesh: 0 },
        { name: "=>0118", mesh: 1 },
        { name: "U1", mesh: 2 },
      ],
    }),
  );
  const padded = Buffer.alloc(Math.ceil(json.length / 4) * 4, 0x20);
  json.copy(padded);
  const binary = Buffer.from([4, 0, 0, 0, 0x42, 0x49, 0x4e, 0, 1, 2, 3, 4]);
  const header = Buffer.alloc(20);
  header.write("glTF");
  header.writeUInt32LE(2, 4);
  header.writeUInt32LE(20 + padded.length + binary.length, 8);
  header.writeUInt32LE(padded.length, 12);
  header.write("JSON", 16);
  const result = nameKiCadGlbLayers(Buffer.concat([header, padded, binary]));
  const length = result.readUInt32LE(12);
  const document = JSON.parse(result.subarray(20, 20 + length).toString("utf8"));
  expect(document.nodes.map((node: { name: string }) => node.name)).toEqual([
    "board_soldermask",
    "board_copper",
    "U1",
  ]);
  expect(result.readUInt32LE(8)).toBe(result.length);
  expect(length % 4).toBe(0);
  expect(result.subarray(20 + length)).toEqual(binary);
});
