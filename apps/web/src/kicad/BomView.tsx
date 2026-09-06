import type { KiCadBom } from "@t3tools/contracts";
import { Download, Search } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

export type BomViewProps = {
  readonly path: string;
  readonly revision: string;
  readonly read: (path: string, signal: AbortSignal) => Promise<string>;
};

const ROWS_PER_PAGE = 100;

function parseBom(value: string): KiCadBom {
  const parsed: unknown = JSON.parse(value);
  if (!parsed || typeof parsed !== "object") throw new Error("BOM response was not an object.");
  return parsed as KiCadBom;
}

function downloadBom(content: string, sourcePath: string) {
  const filename =
    sourcePath
      .split(/[\\/]/)
      .findLast(Boolean)
      ?.replace(/\.kicad_sch$/i, "") || "project";
  const url = URL.createObjectURL(new Blob([content], { type: "text/csv;charset=utf-8" }));
  const link = document.createElement("a");
  link.href = url;
  link.download = `${filename}-bom.csv`;
  link.click();
  window.setTimeout(() => URL.revokeObjectURL(url), 0);
}

export function BomView({ path, revision, read }: BomViewProps) {
  const [bom, setBom] = useState<KiCadBom | null>(null);
  const [query, setQuery] = useState("");
  const [page, setPage] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const readRef = useRef(read);
  const requestKey = `${path}\u0000${revision}`;
  useEffect(() => {
    readRef.current = read;
  }, [read]);

  useEffect(() => {
    const controller = new AbortController();
    setBom(null);
    setError(null);
    setQuery("");
    setPage(0);
    void readRef
      .current(path, controller.signal)
      .then((value) => {
        if (!controller.signal.aborted) setBom(parseBom(value));
      })
      .catch((cause: unknown) => {
        if (!controller.signal.aborted) {
          setError(cause instanceof Error ? cause.message : "Unable to load the BOM.");
        }
      });
    return () => controller.abort();
  }, [requestKey]);

  const filteredRows = useMemo(() => {
    if (!bom) return [];
    const normalized = query.trim().toLocaleLowerCase();
    if (!normalized) return bom.rows;
    return bom.rows.filter((row) =>
      row.some((cell) => cell.toLocaleLowerCase().includes(normalized)),
    );
  }, [bom, query]);
  const pageCount = Math.max(1, Math.ceil(filteredRows.length / ROWS_PER_PAGE));
  const currentPage = Math.min(page, pageCount - 1);
  const visibleRows = filteredRows.slice(
    currentPage * ROWS_PER_PAGE,
    (currentPage + 1) * ROWS_PER_PAGE,
  );

  if (error) {
    return (
      <div
        role="alert"
        className="flex h-full items-center justify-center p-6 text-center text-xs text-destructive"
      >
        {error}
      </div>
    );
  }
  if (!bom) {
    return (
      <div
        role="status"
        className="flex h-full items-center justify-center p-6 text-xs text-muted-foreground"
      >
        Loading BOM…
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col bg-background text-foreground">
      <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-border px-3 py-2 text-xs">
        <span className="font-medium">Bill of materials</span>
        <span className="min-w-0 flex-1 truncate text-muted-foreground">{path}</span>
        <button
          type="button"
          className="inline-flex items-center gap-1 rounded border border-border px-2 py-1 hover:bg-accent"
          onClick={() => downloadBom(bom.content, path)}
        >
          <Download className="size-3.5" aria-hidden="true" /> Download CSV
        </button>
      </div>
      <div className="flex shrink-0 flex-wrap items-center gap-x-4 gap-y-1 border-b border-border px-3 py-2 text-[11px] text-muted-foreground">
        <span>
          Preset: <strong className="font-medium text-foreground">{bom.preset}</strong>
        </span>
        <span>
          Source project:{" "}
          <strong className="font-medium text-foreground">{bom.sourceProject ?? "None"}</strong>
        </span>
        {bom.warnings.map((warning) => (
          <span key={warning} className="text-warning">
            {warning}
          </span>
        ))}
      </div>
      <div className="flex shrink-0 items-center gap-2 border-b border-border px-3 py-2">
        <Search className="size-3.5 text-muted-foreground" aria-hidden="true" />
        <label htmlFor="kicad-bom-search" className="sr-only">
          Search BOM
        </label>
        <input
          id="kicad-bom-search"
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
            setPage(0);
          }}
          placeholder="Search components…"
          className="min-w-0 flex-1 bg-transparent text-xs outline-none placeholder:text-muted-foreground"
        />
        <span className="text-[11px] text-muted-foreground">
          {filteredRows.length} {filteredRows.length === 1 ? "row" : "rows"}
        </span>
      </div>
      <div className="min-h-0 flex-1 overflow-auto">
        <table
          className="w-full border-collapse text-left text-xs"
          aria-label="KiCad bill of materials"
        >
          <caption className="sr-only">KiCad bill of materials for {path}</caption>
          <thead className="sticky top-0 z-10 bg-background">
            <tr>
              {bom.columns.map((column) => (
                <th
                  key={column}
                  scope="col"
                  className="border-b border-border px-3 py-2 font-medium"
                >
                  {column}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {visibleRows.map((row) => (
              <tr key={row.join("\u001f")} className="odd:bg-muted/20">
                {bom.columns.map((column, columnIndex) => (
                  <td
                    key={column}
                    className="max-w-[28rem] border-b border-border/60 px-3 py-1.5 align-top whitespace-pre-wrap"
                  >
                    {row[columnIndex] ?? ""}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
        {visibleRows.length === 0 && (
          <div className="p-8 text-center text-xs text-muted-foreground">
            No matching components.
          </div>
        )}
      </div>
      <div className="flex shrink-0 items-center justify-between border-t border-border px-3 py-2 text-xs">
        <span className="text-muted-foreground">
          Page {currentPage + 1} of {pageCount}
        </span>
        <div className="flex gap-1">
          <button
            type="button"
            className="rounded border border-border px-2 py-1 disabled:opacity-40"
            disabled={currentPage === 0}
            onClick={() => setPage((value) => Math.max(0, value - 1))}
          >
            Previous
          </button>
          <button
            type="button"
            className="rounded border border-border px-2 py-1 disabled:opacity-40"
            disabled={currentPage >= pageCount - 1}
            onClick={() => setPage((value) => Math.min(pageCount - 1, value + 1))}
          >
            Next
          </button>
        </div>
      </div>
    </div>
  );
}
