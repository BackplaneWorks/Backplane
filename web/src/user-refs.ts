export interface MessageRef {
  word: string;
  kind: string;
  id?: string;
  label: string;
}

export type UserSegment =
  | { kind: "text"; text: string }
  | { kind: "model"; word: string; label: string };

const OPENS = new Set(["(", "[", "\"", "'", "`"]);

function isWordBoundaryExtra(ch: string): boolean {
  return /[\p{L}\p{N}\-_\/+\[]/u.test(ch);
}

function isColonIdentChar(ch: string): boolean {
  return /[\p{L}\p{N}\-_\/]/u.test(ch);
}

function isAlnum(ch: string): boolean {
  return /[\p{L}\p{N}]/u.test(ch);
}

/**
 * Splits a user message's raw text into plain text and inline resolved model chip
 * segments matching the backend/GPUI word boundary rules (`&provider/model`).
 * Unresolved `&...` tokens stay as plain text.
 */
export function splitUserModelRefs(text: string, refs?: MessageRef[]): UserSegment[] {
  if (!text) return [];
  const models = (refs ?? []).filter((r) => r.kind === "model" && r.word && r.label);
  if (models.length === 0) return [{ kind: "text", text }];

  const out: UserSegment[] = [];
  let at = 0;
  while (at < text.length) {
    let bestStart = -1;
    let bestEnd = -1;
    let bestModel: MessageRef | null = null;

    for (const m of models) {
      let from = at;
      while (from < text.length) {
        const i = text.indexOf(m.word, from);
        if (i < 0) break;
        const end = i + m.word.length;
        const before = i === 0 || /\s/.test(text[i - 1]) || OPENS.has(text[i - 1]);
        const next = end < text.length ? text[end] : "";
        const after =
          !next ||
          (!isWordBoundaryExtra(next) &&
            !(next === "." && end + 1 < text.length && isAlnum(text[end + 1])) &&
            !(next === ":" && end + 1 < text.length && isColonIdentChar(text[end + 1])));
        if (before && after) {
          if (
            bestStart < 0 ||
            i < bestStart ||
            (i === bestStart && end > bestEnd)
          ) {
            bestStart = i;
            bestEnd = end;
            bestModel = m;
          }
          break;
        }
        from = i + 1;
      }
    }

    if (bestModel === null) break;
    if (bestStart > at) {
      out.push({ kind: "text", text: text.slice(at, bestStart) });
    }
    out.push({ kind: "model", word: bestModel.word, label: bestModel.label });
    at = bestEnd;
  }

  if (at < text.length) {
    out.push({ kind: "text", text: text.slice(at) });
  }
  return out;
}
