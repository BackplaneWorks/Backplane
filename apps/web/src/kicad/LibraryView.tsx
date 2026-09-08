import { useEffect, useRef, useState } from "react";
import { Maximize, Minus, Plus } from "lucide-react";

export type LibraryKind = "footprint" | "symbol";
import type { KiCadLibraryMember as LibraryMember } from "@t3tools/contracts";
export type LibraryRead = (path: string, signal: AbortSignal) => Promise<string>;

function payload(text: string): LibraryMember[] {
  try {
    const value: unknown = JSON.parse(text);
    if (Array.isArray(value))
      return value.filter(
        (v): v is LibraryMember =>
          !!v &&
          typeof v === "object" &&
          typeof (v as LibraryMember).name === "string" &&
          typeof (v as LibraryMember).svg === "string",
      );
  } catch {
    /* raw SVG below */
  }
  return text.trimStart().startsWith("<svg") ? [{ name: "default", svg: text }] : [];
}

export function LibraryView({
  kind,
  path,
  revision,
  member,
  read,
}: {
  kind: LibraryKind;
  path: string;
  revision: string;
  member?: string;
  read: LibraryRead;
}) {
  const [members, setMembers] = useState<LibraryMember[]>([]);
  const [selected, setSelected] = useState(0);
  const selectedNameRef = useRef<string | undefined>(undefined);
  const pathRef = useRef(path);
  const [error, setError] = useState("");
  const [zoom, setZoom] = useState(1);
  const readRef = useRef(read);
  readRef.current = read;
  useEffect(() => {
    const controller = new AbortController();
    const preferred = pathRef.current === path ? selectedNameRef.current : member;
    pathRef.current = path;
    setMembers([]);
    setError("");
    readRef
      .current(path, controller.signal)
      .then((text) => {
        const next = payload(text);
        const index = preferred ? next.findIndex((item) => item.name === preferred) : -1;
        setMembers(next);
        setSelected(index >= 0 ? index : 0);
        selectedNameRef.current = next[index >= 0 ? index : 0]?.name;
      })
      .catch((cause) => {
        if (!controller.signal.aborted) setError(String(cause));
      });
    return () => controller.abort();
  }, [member, path, revision]);
  useEffect(() => {
    selectedNameRef.current = members[selected]?.name;
  }, [members, selected]);
  const item = members[selected] ?? members[0];
  const src = item ? `data:image/svg+xml;charset=utf-8,${encodeURIComponent(item.svg)}` : "";
  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 items-center gap-2 border-b border-border px-2 py-1 text-xs">
        <span className="truncate text-muted-foreground">{path}</span>
        {members.length > 1 && (
          <select
            aria-label={`${kind} member`}
            value={selected}
            onChange={(e) => setSelected(Number(e.target.value))}
            className="ml-auto min-w-0 rounded border border-border bg-background px-2 py-1"
          >
            {members.map((m, i) => (
              <option key={`${m.name}-${i}`} value={i}>
                {m.name}
              </option>
            ))}
          </select>
        )}
        <button
          type="button"
          className="kicad-icon-button"
          aria-label="Zoom out"
          onClick={() => setZoom((z) => z / 1.4)}
        >
          <Minus size={14} />
        </button>
        <button
          type="button"
          className="kicad-icon-button"
          aria-label={`Fit ${kind}`}
          onClick={() => setZoom(1)}
        >
          <Maximize size={14} />
        </button>
        <button
          type="button"
          className="kicad-icon-button"
          aria-label="Zoom in"
          onClick={() => setZoom((z) => z * 1.4)}
        >
          <Plus size={14} />
        </button>
      </div>
      <div className="relative min-h-0 flex-1 overflow-hidden bg-[#0b0f14]">
        {src ? (
          <img
            src={src}
            alt={`${kind} ${item?.name ?? ""}`}
            className="h-full w-full object-contain"
            style={{ transform: `scale(${zoom})` }}
          />
        ) : (
          <div
            role="status"
            className="flex h-full items-center justify-center p-4 text-center text-xs text-muted-foreground"
          >
            {error ||
              (members.length === 0 && path
                ? "No exported preview available."
                : "Loading library preview…")}
          </div>
        )}
      </div>
    </div>
  );
}
