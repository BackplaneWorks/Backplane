// Pure helpers for assistant message file links (`files[]`, gated on `fileLinks`)
// and `/s/<token>/<name>` share links. Mirrors docs/file-links.md.
// Downloads always use plain `<a href="..." download>` (never fetch-and-blob)
// so the browser handles large files and Range/If-Range resume natively.

export interface EntryFile {
  path: string;
  name?: string;
  size?: number;
  open?: "view" | "download" | string;
}

export interface TextLinkSpan {
  start: number;
  end: number;
  kind: "file" | "share";
  file?: EntryFile;
  sharePath?: string;
  label: string;
}

const SHARE_RE = /(?<![A-Za-z0-9_./-])(\/s\/[A-Za-z0-9_-]{16,}\/[^/\s?#`"'<>)\]]+)/g;

export function basenameOf(p: string): string {
  const clean = p.replace(/\\/g, "/").replace(/\/+$/, "");
  const idx = clean.lastIndexOf("/");
  return idx >= 0 ? clean.slice(idx + 1) : clean;
}

/**
 * Build a same-origin `/file?path=...` URL for an assistant-referenced file.
 * For far threads (`<link>~<id>`), appends `thread=<id>` per docs/file-links.md.
 */
export function fileDownloadUrl(filePath: string, threadId?: string): string {
  if (!filePath || /[\0\n\r\t]/.test(filePath)) return "";
  if (threadId && threadId.includes("~")) {
    if (/[\0\n\r\t]/.test(threadId)) return "";
    return `/file?thread=${encodeURIComponent(threadId)}&path=${encodeURIComponent(filePath)}`;
  }
  return `/file?path=${encodeURIComponent(filePath)}`;
}

/** Validate a same-origin `/s/<token>/<name>` share path (no traversal or control chars). */
export function isSafeSharePath(raw: string): boolean {
  if (!raw || /[\0\n\r\t?#\\]/.test(raw)) return false;
  const m = /^\/s\/([A-Za-z0-9_-]{16,})\/([^/]+)$/.exec(raw);
  if (!m) return false;
  const name = m[2];
  return name !== "." && name !== ".." && !name.includes("..");
}

export function formatFileSize(bytes?: number): string {
  if (typeof bytes !== "number" || !Number.isFinite(bytes) || bytes < 0) return "";
  if (bytes < 1024) return `${Math.round(bytes)} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

/** Map a viewable file path to the corresponding web viewer rail kind, or "files". */
export function viewerKindForFile(filePath: string): string {
  const lower = filePath.toLowerCase();
  if (lower.endsWith(".kicad_pcb")) return "board";
  if (lower.endsWith(".kicad_sch")) return "schematic";
  if (lower.endsWith(".step") || lower.endsWith(".stp")) return "mech";
  if (/\.(gbr|gbl|gtl|gbo|gto|gbs|gts|gm1|drl|xln)$/.test(lower)) return "gerbers";
  return "files";
}

function isBoundaryChar(ch: string | undefined): boolean {
  if (!ch) return true;
  return /[\s([{"'`,;:<>]/.test(ch);
}

/**
 * Find non-overlapping occurrences of hub-authorized `files[]` paths and
 * `/s/<token>/<name>` share links inside a plain text string.
 */
export function findChatLinkSpans(text: string, files: EntryFile[]): TextLinkSpan[] {
  if (!text) return [];
  const candidates: TextLinkSpan[] = [];

  // 1. Match hub-authorized file paths (longest path first to prefer specific paths).
  const sorted = [...files]
    .filter((f) => f && typeof f.path === "string" && f.path.length > 0)
    .sort((a, b) => b.path.length - a.path.length);

  for (const file of sorted) {
    let from = 0;
    while (from <= text.length - file.path.length) {
      const idx = text.indexOf(file.path, from);
      if (idx < 0) break;
      const end = idx + file.path.length;
      const before = idx > 0 ? text[idx - 1] : undefined;
      const after = end < text.length ? text[end] : undefined;
      if (isBoundaryChar(before) && (isBoundaryChar(after) || after === ")" || after === "]" || after === ".")) {
        candidates.push({
          start: idx,
          end,
          kind: "file",
          file,
          label: file.path,
        });
      }
      from = end;
    }
  }

  // 2. Match `/s/<token>/<name>` share paths.
  SHARE_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = SHARE_RE.exec(text)) !== null) {
    const raw = m[1].replace(/[.,;:!?]+$/, "");
    if (!isSafeSharePath(raw)) continue;
    candidates.push({
      start: m.index,
      end: m.index + raw.length,
      kind: "share",
      sharePath: raw,
      label: raw,
    });
  }

  if (candidates.length === 0) return [];

  // Sort by start ascending, then longest match first; drop overlaps.
  candidates.sort((a, b) => a.start - b.start || b.end - a.end);
  const out: TextLinkSpan[] = [];
  let cursor = 0;
  for (const c of candidates) {
    if (c.start < cursor) continue;
    out.push(c);
    cursor = c.end;
  }
  return out;
}
