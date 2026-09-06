// @effect-diagnostics nodeBuiltinImport:off
import { Buffer } from "node:buffer";

/** Preserve KiCad's board-layer mesh names through GLTFLoader's node renaming. */
export function nameKiCadGlbLayers(glb: Buffer): Buffer {
  const jsonLength = glb.readUInt32LE(12);
  const document = JSON.parse(glb.subarray(20, 20 + jsonLength).toString("utf8")) as {
    meshes?: Array<{ name?: string }>;
    nodes?: Array<{ name?: string; mesh?: number }>;
  };
  for (const node of document.nodes ?? []) {
    const name = node.mesh === undefined ? undefined : document.meshes?.[node.mesh]?.name;
    if (name && /_(copper|pad|via|silkscreen|soldermask|PCB)$/i.test(name)) node.name = name;
  }
  const json = Buffer.from(JSON.stringify(document));
  const padded = Buffer.alloc(Math.ceil(json.length / 4) * 4, 0x20);
  json.copy(padded);
  const header = Buffer.from(glb.subarray(0, 20));
  const tail = glb.subarray(20 + jsonLength);
  header.writeUInt32LE(header.length + padded.length + tail.length, 8);
  header.writeUInt32LE(padded.length, 12);
  return Buffer.concat([header, padded, tail]);
}
