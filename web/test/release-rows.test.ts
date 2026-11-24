import { describe, expect, test } from "bun:test";
import { releaseRows } from "../src/viewers/panes";

describe("release rows", () => {
  const made = {
    tag: "v1",
    rel: ".backplane/releases/v1",
    manifest: {
      files: ["amp-bom.csv", "drawings/amp-drill.pdf", "drawings/amp-fabrication.pdf", "gerbers/amp-F_Cu.gtl"],
      drawings: [
        { kind: "fabrication", title: "Fabrication drawing", file: "drawings/amp-fabrication.pdf" },
        { kind: "drill", title: "Drill drawing", file: "drawings/amp-drill.pdf" },
      ],
    },
  };

  test("the drawing package comes first, in the manifest's order, then the other outputs once", () => {
    const rows = releaseRows("t 1", made);
    expect(rows.map((r) => [r.title, r.drawing])).toEqual([
      ["Fabrication drawing", true],
      ["Drill drawing", true],
      ["amp-bom.csv", false],
      ["gerbers/amp-F_Cu.gtl", false],
    ]);
    expect(rows[0].url).toBe("/files/get?thread=t%201&path=.backplane%2Freleases%2Fv1%2Fdrawings%2Famp-fabrication.pdf");
  });

  test("a manifest from before the drawing package lists its files only", () => {
    expect(releaseRows("t", { rel: "r", manifest: { files: ["a.step"] } })).toEqual([
      { title: "a.step", file: "a.step", url: "/files/get?thread=t&path=r%2Fa.step", drawing: false },
    ]);
    expect(releaseRows("t", { rel: "r", manifest: null })).toEqual([]);
  });

  test("a file that would leave the release's folder is not offered", () => {
    const rows = releaseRows("t", {
      rel: "r",
      manifest: { files: ["../x", "/etc/passwd", "ok.csv"], drawings: [{ kind: "drill", file: "drawings/../../y.pdf" }] },
    });
    expect(rows.map((r) => r.file)).toEqual(["ok.csv"]);
  });
});
