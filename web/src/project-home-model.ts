// Pure Project Home model: what the hub's `project.home` (feature `projecthome`) says of a project, read defensively, and the few
// text shapes the page draws. Zero DOM imports so it can be unit tested directly with bun test.

export interface HomePart {
  name: string;
  kind: string;
  path: string;
}

export interface HomeMech {
  path: string;
  name: string;
  size: number;
}

export interface ProjectHomeData {
  project: string;
  root: string;
  intent: string;
  parts: HomePart[];
  boards: string[];
  mech: HomeMech[];
  /** The longest lead time the last estimate named, in days; 0 when there is none. */
  leadDays: number;
  /** What the last estimate warned of, each once. */
  warnings: string[];
}

export interface IntentLine {
  kind: "h" | "li" | "p";
  text: string;
}

function str(v: unknown): string {
  return typeof v === "string" ? v : "";
}

function rec(v: unknown): Record<string, unknown> {
  return v && typeof v === "object" && !Array.isArray(v) ? (v as Record<string, unknown>) : {};
}

function list(v: unknown): unknown[] {
  return Array.isArray(v) ? v : [];
}

/** The hub's answer as the page holds it: missing or mistyped fields read as empty, never as an error. */
export function parseProjectHome(raw: unknown): ProjectHomeData {
  const r = rec(raw);
  const lead = typeof r.leadDays === "number" && Number.isFinite(r.leadDays) && r.leadDays > 0 ? Math.floor(r.leadDays) : 0;
  const warnings: string[] = [];
  for (const w of list(r.warnings)) {
    const s = str(w).trim();
    if (s && !warnings.includes(s)) warnings.push(s);
  }
  return {
    project: str(r.project),
    root: str(r.root),
    intent: str(r.intent),
    parts: list(r.parts)
      .map((p) => rec(p))
      .map((p) => ({ name: str(p.name), kind: str(p.kind), path: str(p.path) }))
      .filter((p) => p.name || p.path),
    boards: list(r.boards).map(str).filter(Boolean),
    mech: list(r.mech)
      .map((m) => rec(m))
      .map((m) => ({ path: str(m.path), name: str(m.name), size: typeof m.size === "number" && m.size > 0 ? m.size : 0 }))
      .filter((m) => m.name || m.path),
    leadDays: lead,
    warnings,
  };
}

/**
 * The goal and notes as the person wrote them (`.backplane/intent.md`), a line each: a heading, a list item or text.
 * The desktop's `intent_lines` (native/gpui/src/screens/projects.rs): blank lines dropped, 60 lines at most.
 */
export function intentLines(text: string): IntentLine[] {
  const out: IntentLine[] = [];
  for (const raw of text.split("\n")) {
    const line = raw.trimEnd();
    if (!line.trim()) continue;
    if (out.length >= 60) break;
    const t = line.trimStart();
    if (t.startsWith("#")) {
      out.push({ kind: "h", text: t.replace(/^#+/, "").trim() });
    } else if (t.startsWith("- ") || t.startsWith("* ")) {
      out.push({ kind: "li", text: t.slice(2).trim() });
    } else {
      out.push({ kind: "p", text: t });
    }
  }
  return out;
}

/** Whether the intent says anything beyond its headings (the template is headings only). */
export function intentIsEmpty(text: string): boolean {
  return !intentLines(text).some((l) => l.kind !== "h" && l.text.length > 0);
}

/** A file's size as the desktop writes it: B, KB, MB. */
export function sizeText(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "";
  if (bytes < 1024) return `${Math.floor(bytes)} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(bytes < 10 * 1024 ? 1 : 0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** A project's id as the hub's requests take it: a section row's id may carry its place before a `|`. */
export function rawProjectId(id: string): string {
  const idx = id.lastIndexOf("|");
  return idx >= 0 ? id.slice(idx + 1) : id;
}
