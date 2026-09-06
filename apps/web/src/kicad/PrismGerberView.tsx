import { useEffect, useMemo, useRef, useState } from "react";
import { Maximize, Minus, Plus } from "lucide-react";

import { paneLayout, useBoardViewport, type BoardRect } from "./vendor/fabrication-viewport";

/** Prism's fabrication camera and SVG pane, with T3 toolbar styling. */
export function PrismGerberView({ svg, label }: { svg: string; label: string }) {
  const board = useMemo<BoardRect | null>(() => {
    const values = new DOMParser()
      .parseFromString(svg, "image/svg+xml")
      .documentElement.getAttribute("viewBox")
      ?.split(/\s+/)
      .map(Number);
    if (!values || values.length !== 4 || values.some((value) => !Number.isFinite(value)))
      return null;
    return { x: values[0]!, y: values[1]!, width: values[2]!, height: values[3]! };
  }, [svg]);
  const viewport = useBoardViewport(board);
  const ref = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const [url, setUrl] = useState("");
  useEffect(() => {
    const blobUrl = URL.createObjectURL(new Blob([svg], { type: "image/svg+xml" }));
    setUrl(blobUrl);
    return () => URL.revokeObjectURL(blobUrl);
  }, [svg]);
  useEffect(() => {
    const node = ref.current;
    if (!node) return;
    const observer = new ResizeObserver(([entry]) => {
      if (entry) setSize({ width: entry.contentRect.width, height: entry.contentRect.height });
    });
    observer.observe(node);
    return () => observer.disconnect();
  }, []);
  const layout =
    board && size.width && size.height ? paneLayout(board, board, viewport.view, size) : null;
  return (
    <div className="relative flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 items-center justify-end gap-1 border-b border-border px-2 py-1">
        <button
          className="kicad-icon-button"
          aria-label="Zoom out"
          onClick={() => viewport.zoomBy(1 / 1.4)}
        >
          <Minus size={14} />
        </button>
        <button
          className="kicad-icon-button"
          aria-label="Fit Gerber layer"
          onClick={viewport.reset}
        >
          <Maximize size={14} />
        </button>
        <button
          className="kicad-icon-button"
          aria-label="Zoom in"
          onClick={() => viewport.zoomBy(1.4)}
        >
          <Plus size={14} />
        </button>
      </div>
      <div
        ref={ref}
        className="relative min-h-0 flex-1 touch-none overflow-hidden bg-[#0b0f14] cursor-grab active:cursor-grabbing"
        {...viewport.handlers}
      >
        <div
          className="absolute"
          style={
            layout
              ? { width: layout.width, height: layout.height, left: layout.left, top: layout.top }
              : { inset: 0 }
          }
        >
          {url && (
            <img
              src={url}
              alt={label}
              draggable={false}
              className="h-full w-full select-none object-contain"
            />
          )}
        </div>
      </div>
    </div>
  );
}
