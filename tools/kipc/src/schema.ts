// The API schema: the runtime's descriptor set when it ships one, else a
// small built-in one that knows only Ping and GetVersion.
import protobuf from "protobufjs";
import descriptor from "protobufjs/ext/descriptor/index.js";
import { readFileSync, statSync } from "node:fs";

export const NO_SCHEMA = "no API schema in this KiCad runtime";

const builtin = {
  nested: {
    google: { nested: { protobuf: { nested: { Any: { fields: { type_url: { type: "string", id: 1 }, value: { type: "bytes", id: 2 } } } } } } },
    kiapi: {
      nested: {
        common: {
          nested: {
            ApiRequestHeader: { fields: { kicad_token: { type: "string", id: 1 }, client_name: { type: "string", id: 2 } } },
            ApiRequest: { fields: { header: { type: "ApiRequestHeader", id: 1 }, message: { type: "google.protobuf.Any", id: 2 } } },
            ApiResponseHeader: { fields: { kicad_token: { type: "string", id: 1 } } },
            ApiResponseStatus: { fields: { status: { type: "int32", id: 1 }, error_message: { type: "string", id: 2 } } },
            ApiResponse: {
              fields: {
                header: { type: "ApiResponseHeader", id: 1 },
                status: { type: "ApiResponseStatus", id: 2 },
                message: { type: "google.protobuf.Any", id: 3 },
              },
            },
            types: { nested: { KiCadVersion: { fields: { major: { type: "uint32", id: 1 }, minor: { type: "uint32", id: 2 }, patch: { type: "uint32", id: 3 }, full_version: { type: "string", id: 4 } } } } },
            commands: {
              nested: {
                Ping: { fields: {} },
                GetVersion: { fields: {} },
                GetVersionResponse: { fields: { version: { type: "kiapi.common.types.KiCadVersion", id: 1 } } },
              },
            },
          },
        },
      },
    },
  },
};

export const STATUS = [
  "AS_UNKNOWN", "AS_OK", "AS_TIMEOUT", "AS_BAD_REQUEST", "AS_NOT_READY",
  "AS_UNHANDLED", "AS_TOKEN_MISMATCH", "AS_BUSY", "AS_UNIMPLEMENTED", "AS_INTERNAL_ERROR",
];

export class Schema {
  constructor(public root: protobuf.Root, public full: boolean) {}
  type(name: string): protobuf.Type | null {
    try {
      return this.root.lookupType(name.replace(/^\./, ""));
    } catch {
      return null;
    }
  }
  // only Ping and GetVersion without a descriptor set
  knows(name: string): boolean {
    return this.full || name === "kiapi.common.commands.Ping" || name === "kiapi.common.commands.GetVersion";
  }
}

const cache = new Map<string, { stamp: string; schema: Schema }>();

export function builtinSchema(): Schema {
  return new Schema(protobuf.Root.fromJSON(builtin as never), false);
}

export function fromBytes(bytes: Uint8Array): Schema {
  const set = (descriptor as never as { FileDescriptorSet: protobuf.Type }).FileDescriptorSet.decode(bytes);
  const root = (protobuf.Root as never as { fromDescriptor(d: unknown): protobuf.Root }).fromDescriptor(set);
  root.resolveAll();
  return new Schema(root, true);
}

// the schema for a descriptor-set file ("" or unreadable: the built-in one)
export function load(file: string): Schema {
  if (!file) return builtinSchema();
  let stamp: string;
  try {
    const st = statSync(file);
    stamp = `${st.size}:${st.mtimeMs}`;
  } catch {
    return builtinSchema();
  }
  const hit = cache.get(file);
  if (hit && hit.stamp === stamp) return hit.schema;
  let schema: Schema;
  try {
    schema = fromBytes(readFileSync(file));
  } catch {
    schema = builtinSchema();
  }
  cache.set(file, { stamp, schema });
  return schema;
}
