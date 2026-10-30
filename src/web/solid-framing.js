// Fit saved geometry to the actual camera axes and viewport, in the same units
// as its bounds. A thin rectangular PCB need not fit inside an empty sphere.
export function fittedDistance(box, camera, aspect, orthographic = false, margin = 1.08) {
  const halfY = Math.tan(camera.fov / 2), halfX = halfY * Math.max(.01, aspect);
  const radius = Math.max(1, Math.hypot(box[3] - box[0], box[4] - box[1], box[5] - box[2]) / 2);
  const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
  let distance = radius * .02;
  for (const x of [box[0], box[3]]) for (const y of [box[1], box[4]]) for (const z of [box[2], box[5]]) {
    const delta = [x - camera.pivot[0], y - camera.pivot[1], z - camera.pivot[2]], depth = dot(delta, camera.f);
    distance = Math.max(distance, -depth + radius * .02,
      Math.abs(dot(delta, camera.r)) / halfX - (orthographic ? 0 : depth),
      Math.abs(dot(delta, camera.u)) / halfY - (orthographic ? 0 : depth));
  }
  return distance * margin;
}
