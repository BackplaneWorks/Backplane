import { describe, expect, test } from "bun:test";
import {
  basenameOf,
  fileDownloadUrl,
  findChatLinkSpans,
  formatFileSize,
  isSafeSharePath,
  viewerKindForFile,
  type EntryFile,
} from "../src/chat-files";

describe("chat-files", () => {
  test("fileDownloadUrl encodes path and adds thread only for far threads", () => {
    expect(fileDownloadUrl("/home/user/proj/mfg/bom.csv", "t1")).toBe(
      "/file?path=%2Fhome%2Fuser%2Fproj%2Fmfg%2Fbom.csv",
    );
    expect(fileDownloadUrl("mfg/bom.csv", "mac~t42")).toBe(
      "/file?thread=mac~t42&path=mfg%2Fbom.csv",
    );
    expect(fileDownloadUrl("/tmp/bad\npath", "t1")).toBe("");
    expect(fileDownloadUrl("", "t1")).toBe("");
  });

  test("isSafeSharePath validates /s/<token>/<name> strictly", () => {
    expect(isSafeSharePath("/s/abcdefghijklmnopqrstuvwxyz012345/report.pdf")).toBe(true);
    expect(isSafeSharePath("/s/short/report.pdf")).toBe(false);
    expect(isSafeSharePath("/s/abcdefghijklmnopqrstuvwxyz012345/..")).toBe(false);
    expect(isSafeSharePath("/s/abcdefghijklmnopqrstuvwxyz012345/a/b.pdf")).toBe(false);
    expect(isSafeSharePath("https://evil.example/s/abcdefghijklmnopqrstuvwxyz012345/a.pdf")).toBe(false);
  });

  test("findChatLinkSpans matches file paths and share links without overlap", () => {
    const files: EntryFile[] = [
      { path: "mfg/gerbers.zip", name: "gerbers.zip", size: 4096, open: "download" },
      { path: "ecc83-pp.kicad_pcb", name: "ecc83-pp.kicad_pcb", size: 51200, open: "view" },
    ];
    const text =
      "Exported mfg/gerbers.zip from ecc83-pp.kicad_pcb. Share: /s/0123456789abcdef01234567/gerbers.zip.";
    const spans = findChatLinkSpans(text, files);
    expect(spans.length).toBe(3);
    expect(spans[0].kind).toBe("file");
    expect(spans[0].file?.name).toBe("gerbers.zip");
    expect(spans[1].kind).toBe("file");
    expect(spans[1].file?.open).toBe("view");
    expect(spans[2].kind).toBe("share");
    expect(spans[2].sharePath).toBe("/s/0123456789abcdef01234567/gerbers.zip");
  });

  test("findChatLinkSpans prefers longest matching path and respects word boundaries", () => {
    const files: EntryFile[] = [
      { path: "mfg/bom.csv", name: "bom.csv", open: "download" },
      { path: "/proj/mfg/bom.csv", name: "bom.csv", open: "download" },
    ];
    const spans = findChatLinkSpans("Wrote /proj/mfg/bom.csv and xmfg/bom.csv", files);
    expect(spans.length).toBe(1);
    expect(spans[0].label).toBe("/proj/mfg/bom.csv");
  });

  test("viewerKindForFile and formatFileSize", () => {
    expect(viewerKindForFile("a/b.kicad_pcb")).toBe("board");
    expect(viewerKindForFile("a/b.kicad_sch")).toBe("schematic");
    expect(viewerKindForFile("mech/box.step")).toBe("mech");
    expect(viewerKindForFile("mfg/board.gbr")).toBe("gerbers");
    expect(viewerKindForFile("docs/notes.pdf")).toBe("files");
    expect(basenameOf("mfg/gerbers.zip")).toBe("gerbers.zip");
    expect(formatFileSize(512)).toBe("512 B");
    expect(formatFileSize(2048)).toBe("2.0 KB");
    expect(formatFileSize(3 * 1024 * 1024)).toBe("3.0 MB");
  });
});
