import { describe, expect, test } from "bun:test";
import {
  filesGetUrl,
  isInlineImageContentType,
  isInlineImageExt,
  isSafeDownloadUrl,
  resolveMechDownload,
  resolveRowDownload,
  shouldShowMfgButton,
} from "../src/viewers/file-links";

describe("file-links", () => {
  test("builds /files/get URLs and rejects traversal or control chars", () => {
    expect(filesGetUrl("t1", "mfg/gerbers.zip")).toBe(
      "/files/get?thread=t1&path=mfg%2Fgerbers.zip"
    );
    expect(filesGetUrl("link~t1", "docs/readme.md")).toBe(
      "/files/get?thread=link~t1&path=docs%2Freadme.md"
    );
    expect(filesGetUrl("", "a.txt")).toBe("");
    expect(filesGetUrl("t1", "")).toBe("");
    expect(filesGetUrl("t1", "../secret")).toBe("");
    expect(filesGetUrl("t1", "a/../b")).toBe("");
    expect(filesGetUrl("t1", "a\nb")).toBe("");
  });

  test("never allows html or svg inline", () => {
    expect(isInlineImageExt("png")).toBe(true);
    expect(isInlineImageExt("JPG")).toBe(true);
    expect(isInlineImageExt("webp")).toBe(true);
    expect(isInlineImageExt("svg")).toBe(false);
    expect(isInlineImageExt("html")).toBe(false);
    expect(isInlineImageExt("htm")).toBe(false);

    expect(isInlineImageContentType("image/png")).toBe(true);
    expect(isInlineImageContentType("image/jpeg; charset=binary")).toBe(true);
    expect(isInlineImageContentType("image/svg+xml")).toBe(false);
    expect(isInlineImageContentType("text/html; charset=utf-8")).toBe(false);
  });

  test("validates safe same-origin download URLs", () => {
    expect(isSafeDownloadUrl("/files/get?thread=t1&path=a.step")).toBe(true);
    expect(isSafeDownloadUrl("/file?path=%2Fp%2Fa.step")).toBe(true);
    expect(isSafeDownloadUrl("/s/abc123/board.zip")).toBe(true);
    expect(isSafeDownloadUrl("https://evil.example/a.step")).toBe(false);
    expect(isSafeDownloadUrl("javascript:alert(1)")).toBe(false);
    expect(isSafeDownloadUrl("")).toBe(false);
  });

  test("shouldShowMfgButton matches local thread, root mfg, and currentDir rules", () => {
    expect(shouldShowMfgButton("t1", "", true)).toBe(true);
    expect(shouldShowMfgButton("t1", "docs", true)).toBe(true);
    expect(shouldShowMfgButton("t1", "", false, "mfg")).toBe(true);
    expect(shouldShowMfgButton("t1", "mfg", true, "mfg")).toBe(false);
    expect(shouldShowMfgButton("t1", "mfg/gerbers", true, "mfg")).toBe(false);
    expect(shouldShowMfgButton("remote~t1", "", true, "mfg")).toBe(false);
    expect(shouldShowMfgButton("t1", "", false, "")).toBe(false);
  });

  test("resolveRowDownload prefers hub-provided down URL and falls back to /files/get", () => {
    expect(resolveRowDownload("t1", "", { dir: true, name: "mfg" })).toBeNull();
    expect(
      resolveRowDownload("t1", "mfg", {
        dir: false,
        name: "bom.csv",
        down: "/files/get?thread=t1&path=mfg/bom.csv",
        downName: "bom.csv",
      })
    ).toEqual({ url: "/files/get?thread=t1&path=mfg/bom.csv", name: "bom.csv" });
    expect(
      resolveRowDownload("t1", "mfg", { dir: false, name: "cpl.csv" })
    ).toEqual({ url: "/files/get?thread=t1&path=mfg%2Fcpl.csv", name: "cpl.csv" });
    // Unsafe hub URL falls back safely to /files/get
    expect(
      resolveRowDownload("t1", "", {
        dir: false,
        name: "a.bin",
        down: "javascript:alert(1)",
      })
    ).toEqual({ url: "/files/get?thread=t1&path=a.bin", name: "a.bin" });
  });

  test("resolveMechDownload builds plain /files/get link for active part", () => {
    expect(resolveMechDownload("t1", "")).toBeNull();
    expect(resolveMechDownload("t1", "mech/case.step")).toEqual({
      url: "/files/get?thread=t1&path=mech%2Fcase.step",
      name: "case.step",
    });
    expect(
      resolveMechDownload("t1", "mech/case.step", "/files/get?thread=t1&path=mech/case.step", "case.step")
    ).toEqual({
      url: "/files/get?thread=t1&path=mech/case.step",
      name: "case.step",
    });
  });
});
