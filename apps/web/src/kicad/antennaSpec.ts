export type AntennaSpecForm = {
  boardWidthMm: number;
  boardHeightMm: number;
  radiatorWidthMm: number;
  radiatorLengthMm: number;
  feedXmm: number;
  feedYmm: number;
  groundXmm: number;
  groundYmm: number;
  lowerMhz: number;
  upperMhz: number;
  samples: number;
  targetDb: number;
  maxSteps: number;
  backend: "emerge" | "openems";
  secondBandEnabled: boolean;
  lowerMhz2: number;
  upperMhz2: number;
};

export const defaultAntennaSpecForm: AntennaSpecForm = {
  boardWidthMm: 40,
  boardHeightMm: 25,
  radiatorWidthMm: 20,
  radiatorLengthMm: 12,
  feedXmm: 10,
  feedYmm: 2,
  groundXmm: 10,
  groundYmm: 20,
  lowerMhz: 2400,
  upperMhz: 2500,
  samples: 5,
  targetDb: -10,
  maxSteps: 100,
  backend: "emerge",
  secondBandEnabled: false,
  lowerMhz2: 3500,
  upperMhz2: 3700,
};

type Point = {
  position_m: [number, number, number];
  triangle_index: number;
  barycentric: [number, number, number];
  component_id: string;
};

export type AntennaProblem = {
  schema_version: 1;
  units: { length: "m"; frequency: "Hz" };
  mesh: {
    vertices: [number, number, number][];
    triangles: [number, number, number][];
    component_ids: string[];
    coordinate_frame: "right_handed_cartesian";
    winding: "unknown";
  };
  feed: { point: Point; impedance_ohm: 50 };
  ground: { permitted: Point[]; defined: Point };
  bands: { lower_hz: number; upper_hz: number; samples: number; weight: 1 }[];
  objectives: [{ metric: "s11_db"; sense: "minimize"; target: number; weight: 1 }];
  max_steps: number;
};

const metres = (millimetres: number) => millimetres / 1000;

export function validateAntennaSpec(form: AntennaSpecForm): string[] {
  const numeric = Object.entries(form).filter(([, value]) => typeof value === "number") as [
    string,
    number,
  ][];
  const errors = numeric
    .filter(([, value]) => !Number.isFinite(value))
    .map(([key]) => `${key} must be finite`);
  if (form.boardWidthMm <= 0 || form.boardHeightMm <= 0)
    errors.push("Board dimensions must be positive");
  if (form.radiatorWidthMm <= 0 || form.radiatorLengthMm <= 0)
    errors.push("Radiator dimensions must be positive");
  if (form.radiatorWidthMm > form.boardWidthMm || form.radiatorLengthMm > form.boardHeightMm)
    errors.push("Radiator must fit inside the board");
  if (
    form.feedXmm < (form.boardWidthMm - form.radiatorWidthMm) / 2 ||
    form.feedXmm > (form.boardWidthMm + form.radiatorWidthMm) / 2 ||
    form.feedYmm < 0 ||
    form.feedYmm > form.radiatorLengthMm
  )
    errors.push("Feed must lie inside the radiator");
  if (
    form.groundXmm < 0 ||
    form.groundXmm > form.boardWidthMm ||
    form.groundYmm < 0 ||
    form.groundYmm + 2 > form.boardHeightMm
  )
    errors.push("Ground must fit inside the board");
  const radiatorX = (form.boardWidthMm - form.radiatorWidthMm) / 2;
  if (
    form.groundYmm < form.radiatorLengthMm &&
    form.groundYmm + 2 > 0 &&
    form.groundXmm + form.boardWidthMm > radiatorX &&
    form.groundXmm < radiatorX + form.radiatorWidthMm
  )
    errors.push("Ground must not overlap the radiator");
  if (form.lowerMhz <= 0 || form.upperMhz <= form.lowerMhz)
    errors.push("Frequency band must be positive and ascending");
  if (form.secondBandEnabled && (form.lowerMhz2 <= 0 || form.upperMhz2 <= form.lowerMhz2))
    errors.push("Second frequency band must be positive and ascending");
  if (form.samples < 1 || form.maxSteps < 1)
    errors.push("Samples and max steps must be at least one");
  return errors;
}

function barycentric(
  point: [number, number],
  a: [number, number],
  b: [number, number],
  c: [number, number],
): [number, number, number] {
  const denominator = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
  const u = ((b[1] - c[1]) * (point[0] - c[0]) + (c[0] - b[0]) * (point[1] - c[1])) / denominator;
  const v = ((c[1] - a[1]) * (point[0] - c[0]) + (a[0] - c[0]) * (point[1] - c[1])) / denominator;
  return [u, v, 1 - u - v];
}

function locatePoint(
  point: [number, number],
  triangles: [[number, number], [number, number], [number, number]][],
): { triangle_index: number; barycentric: [number, number, number] } {
  for (const [triangle_index, triangle] of triangles.entries()) {
    const weights = barycentric(point, ...triangle);
    if (weights.every((weight) => weight >= -1e-9 && weight <= 1 + 1e-9))
      return { triangle_index, barycentric: weights };
  }
  throw new Error("Contact point is outside the generated mesh");
}

/** Build the strict antenna-rl problem document used by the EMerge dashboard. */
export function buildAntennaProblem(form: AntennaSpecForm): AntennaProblem {
  const errors = validateAntennaSpec(form);
  if (errors.length) throw new Error(errors.join("; "));
  const boardWidth = metres(form.boardWidthMm);
  const radiatorWidth = metres(form.radiatorWidthMm);
  const radiatorLength = metres(form.radiatorLengthMm);
  const radiatorX = (boardWidth - radiatorWidth) / 2;
  const groundWidth = boardWidth;
  const groundHeight = metres(2);
  const groundX = (boardWidth - groundWidth) / 2;
  const groundY = metres(form.groundYmm);
  const z = 0;
  const vertices: [number, number, number][] = [
    [radiatorX, 0, z],
    [radiatorX + radiatorWidth, 0, z],
    [radiatorX + radiatorWidth, radiatorLength, z],
    [radiatorX, radiatorLength, z],
    [groundX, groundY, z],
    [groundX + groundWidth, groundY, z],
    [groundX + groundWidth, groundY + groundHeight, z],
    [groundX, groundY + groundHeight, z],
  ];
  const feedPosition: [number, number] = [metres(form.feedXmm), metres(form.feedYmm)];
  const radiatorTriangles: [[number, number], [number, number], [number, number]][] = [
    [
      [radiatorX, 0],
      [radiatorX + radiatorWidth, 0],
      [radiatorX + radiatorWidth, radiatorLength],
    ],
    [
      [radiatorX, 0],
      [radiatorX + radiatorWidth, radiatorLength],
      [radiatorX, radiatorLength],
    ],
  ];
  const feedLocation = locatePoint(feedPosition, radiatorTriangles);
  const feed: Point = {
    position_m: [...feedPosition, z],
    triangle_index: feedLocation.triangle_index,
    barycentric: feedLocation.barycentric,
    component_id: "radiator",
  };
  const groundPosition: [number, number] = [metres(form.groundXmm), groundY + groundHeight / 2];
  const groundLocation = locatePoint(groundPosition, [
    [
      [groundX, groundY],
      [groundX + groundWidth, groundY],
      [groundX + groundWidth, groundY + groundHeight],
    ],
    [
      [groundX, groundY],
      [groundX + groundWidth, groundY + groundHeight],
      [groundX, groundY + groundHeight],
    ],
  ]);
  const ground: Point = {
    position_m: [...groundPosition, z],
    triangle_index: groundLocation.triangle_index + 2,
    barycentric: groundLocation.barycentric,
    component_id: "ground",
  };
  return {
    schema_version: 1,
    units: { length: "m", frequency: "Hz" },
    mesh: {
      vertices,
      triangles: [
        [0, 1, 2],
        [0, 2, 3],
        [4, 5, 6],
        [4, 6, 7],
      ],
      component_ids: ["radiator", "radiator", "ground", "ground"],
      coordinate_frame: "right_handed_cartesian",
      winding: "unknown",
    },
    feed: { point: feed, impedance_ohm: 50 },
    ground: { permitted: [ground], defined: ground },
    bands: [
      {
        lower_hz: form.lowerMhz * 1_000_000,
        upper_hz: form.upperMhz * 1_000_000,
        samples: Math.max(1, Math.round(form.samples)),
        weight: 1,
      },
      ...(form.secondBandEnabled
        ? [
            {
              lower_hz: form.lowerMhz2 * 1_000_000,
              upper_hz: form.upperMhz2 * 1_000_000,
              samples: Math.max(1, Math.round(form.samples)),
              weight: 1 as const,
            },
          ]
        : []),
    ],
    objectives: [{ metric: "s11_db", sense: "minimize", target: form.targetDb, weight: 1 }],
    max_steps: Math.max(1, Math.round(form.maxSteps)),
  };
}

export function antennaProblemJson(form: AntennaSpecForm): string {
  return `${JSON.stringify(buildAntennaProblem(form), null, 2)}\n`;
}
