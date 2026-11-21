// Pure helpers for Files and Mech download links and Manufacturing shortcut.
// Mirrors docs/file-links.md and native/host/src/present/viewer.rs (`down_url`, `mfg`).
// Downloads always use plain `<a href="..." download>` so the browser's own download
// manager handles large files and Range/If-Range resume (never fetch-and-blob).

export interface FileEntryLike {
  dir: boolean;
  name: string;
  down?: string;
  downName?: string;
}

export interface DownloadLink {
  url: string;
  name: string;
}

function basename(path: string): string {
  const clean = path.replace(/\\/g, "/").replace(/\/+$/, "");
  const idx = clean.lastIndexOf("/");
  return idx >= 0 ? clean.slice(idx + 1) : clean;
}

/** Build a same-origin `/files/get` URL for a thread's relative project path. */
export function filesGetUrl(threadId: string, relPath: string): string {
  if (!threadId || !relPath) return "";
  if (/[\0\n\r\t]/.test(threadId) || /[\0\n\r\t]/.test(relPath)) return "";
  const clean = relPath.replace(/\\/g, "/").replace(/^\/+/, "");
  if (!clean || clean.split("/").some((seg) => seg === "..")) return "";
  return `/files/get?thread=${encodeURIComponent(threadId)}&path=${encodeURIComponent(clean)}`;
}

/** Allow only same-origin hub file download routes (never external origins or javascript:). */
export function isSafeDownloadUrl(url: string | undefined): url is string {
  if (!url) return false;
  return url.startsWith("/files/get?") || url.startsWith("/file?") || url.startsWith("/s/");
}

/**
 * Only raster image formats that the hub allows inline (`docs/file-links.md`).
 * HTML and SVG are NEVER opened inline at the web app's origin.
 */
export function isInlineImageExt(ext: string): boolean {
  return ["png", "jpg", "jpeg", "gif", "webp", "bmp"].includes(ext.toLowerCase());
}

/** Match only safe raster image MIME types (explicitly excludes image/svg+xml and text/html). */
export function isInlineImageContentType(contentType: string): boolean {
  const base = contentType.split(";")[0].trim().toLowerCase();
  return (
    base === "image/png" ||
    base === "image/jpeg" ||
    base === "image/gif" ||
    base === "image/webp" ||
    base === "image/bmp"
  );
}

/**
 * Whether to show the Manufacturing (`mfg/`) shortcut button in the Files toolbar.
 * Mirrors `present/viewer.rs`: local threads only (`!far`), not already inside `mfg`,
 * when `mfg` is reported by the hub or present in the project root listing.
 */
export function shouldShowMfgButton(
  threadId: string,
  currentDir: string,
  hasMfgRoot: boolean,
  mfgField?: string
): boolean {
  if (!threadId || threadId.includes("~")) return false;
  if (currentDir === "mfg" || currentDir.startsWith("mfg/")) return false;
  return mfgField === "mfg" || hasMfgRoot;
}

/** Resolve a Files pane row's download link (non-directory rows only). */
export function resolveRowDownload(
  threadId: string,
  dir: string,
  entry: FileEntryLike
): DownloadLink | null {
  if (entry.dir || !entry.name) return null;
  if (isSafeDownloadUrl(entry.down)) {
    return { url: entry.down, name: entry.downName || entry.name };
  }
  const rel = dir ? `${dir}/${entry.name}` : entry.name;
  const url = filesGetUrl(threadId, rel);
  if (!url) return null;
  return { url, name: entry.downName || entry.name };
}

/** Resolve the Mech pane's active part download link. */
export function resolveMechDownload(
  threadId: string,
  selectedMech: string,
  down?: string,
  downName?: string
): DownloadLink | null {
  if (!selectedMech) return null;
  const fallbackName = basename(selectedMech);
  if (isSafeDownloadUrl(down)) {
    return { url: down, name: downName || fallbackName };
  }
  const url = filesGetUrl(threadId, selectedMech);
  if (!url) return null;
  return { url, name: downName || fallbackName };
}
