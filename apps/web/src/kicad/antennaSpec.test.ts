import { expect, it } from "vite-plus/test";
import {
  antennaProblemJson,
  buildAntennaProblem,
  defaultAntennaSpecForm,
  validateAntennaSpec,
} from "./antennaSpec";

it("emits the antenna-rl v1 contract in SI units", () => {
  const problem = buildAntennaProblem(defaultAntennaSpecForm);
  expect(problem.schema_version).toBe(1);
  expect(problem.units).toEqual({ length: "m", frequency: "Hz" });
  expect(problem.mesh.triangles).toHaveLength(4);
  expect(problem.mesh.component_ids).toEqual(["radiator", "radiator", "ground", "ground"]);
  expect(problem.bands[0]!.lower_hz).toBe(2_400_000_000);
  expect(problem.bands[0]!.upper_hz).toBe(2_500_000_000);
  expect(problem.feed.point.component_id).toBe("radiator");
  expect(problem.ground.defined.component_id).toBe("ground");
  expect(
    problem.feed.point.barycentric.every((weight) => weight >= -1e-9 && weight <= 1 + 1e-9),
  ).toBe(true);
  expect(
    problem.ground.defined.barycentric.every((weight) => weight >= -1e-9 && weight <= 1 + 1e-9),
  ).toBe(true);
  const point = problem.feed.point.position_m;
  const triangle = problem.mesh.triangles[problem.feed.point.triangle_index]!;
  const bary = problem.feed.point.barycentric;
  const reconstructed = triangle.reduce(
    (sum, vertex, index) => sum + problem.mesh.vertices[vertex]![0] * bary[index]!,
    0,
  );
  expect(reconstructed).toBeCloseTo(point[0]);
});

it("rejects geometry that cannot be simulated", () => {
  expect(validateAntennaSpec({ ...defaultAntennaSpecForm, feedXmm: 0 })).toContain(
    "Feed must lie inside the radiator",
  );
  expect(validateAntennaSpec({ ...defaultAntennaSpecForm, groundYmm: 24 })).toContain(
    "Ground must fit inside the board",
  );
  expect(() =>
    buildAntennaProblem({ ...defaultAntennaSpecForm, lowerMhz: 2500, upperMhz: 2400 }),
  ).toThrow("Frequency band");
  expect(
    buildAntennaProblem({ ...defaultAntennaSpecForm, secondBandEnabled: true }).bands,
  ).toHaveLength(2);
});

it("produces stable, newline-terminated JSON suitable for download", () => {
  const json = antennaProblemJson(defaultAntennaSpecForm);
  expect(json.endsWith("\n")).toBe(true);
  expect(JSON.parse(json).max_steps).toBe(100);
});
