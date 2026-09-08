import * as Schema from "effect/Schema";
import { TrimmedNonEmptyString } from "./baseSchemas.ts";

export const KiCadFileKind = Schema.Literals([
  "gerber",
  "pcb",
  "schematic",
  "model",
  "project",
  "footprint",
  "symbol",
]);
export type KiCadFileKind = typeof KiCadFileKind.Type;

export const KiCadProjectFile = Schema.Struct({
  path: TrimmedNonEmptyString,
  kind: KiCadFileKind,
  mimeType: TrimmedNonEmptyString,
  size: Schema.Number,
  mtimeMs: Schema.Number,
});
export type KiCadProjectFile = typeof KiCadProjectFile.Type;

export const KiCadProjectConfig = Schema.Struct({
  analysisUrl: Schema.optionalKey(Schema.String),
  pcb: Schema.optionalKey(Schema.String),
  schematic: Schema.optionalKey(Schema.String),
  gerbers: Schema.optionalKey(Schema.Array(Schema.String)),
  symbol: Schema.optionalKey(Schema.String),
  symbolMember: Schema.optionalKey(Schema.String),
  footprint: Schema.optionalKey(Schema.String),
});
export type KiCadProjectConfig = typeof KiCadProjectConfig.Type;

export const KiCadProjectManifest = Schema.Struct({
  root: TrimmedNonEmptyString,
  revision: TrimmedNonEmptyString,
  files: Schema.Array(KiCadProjectFile),
  config: Schema.optionalKey(KiCadProjectConfig),
  warnings: Schema.Array(Schema.String),
});
export type KiCadProjectManifest = typeof KiCadProjectManifest.Type;

export const KiCadViewerSession = Schema.Struct({
  token: TrimmedNonEmptyString,
  expiresAt: Schema.Number,
});
export type KiCadViewerSession = typeof KiCadViewerSession.Type;

export const KiCadBom = Schema.Struct({
  columns: Schema.Array(Schema.String),
  rows: Schema.Array(Schema.Array(Schema.String)),
  content: Schema.String,
  preset: Schema.String,
  sourceProject: Schema.NullOr(Schema.String),
  warnings: Schema.Array(Schema.String),
});
export type KiCadBom = typeof KiCadBom.Type;

export const KiCadLibraryMember = Schema.Struct({ name: Schema.String, svg: Schema.String });
export type KiCadLibraryMember = typeof KiCadLibraryMember.Type;
