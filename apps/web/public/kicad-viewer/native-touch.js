// Adapt touch input to Prism's existing camera without replacing its renderer.
const installed = new WeakSet();

export function installNativeTouch(element) {
  for (const name of ["kc-board-app", "kc-schematic-app"]) {
    const viewer = element.shadowRoot?.querySelector(name)?.viewer;
    if (!viewer || installed.has(viewer)) continue;
    installed.add(viewer);
    installTouch(viewer);
  }
}

function installTouch(viewer) {
  const canvas = viewer.renderer.canvas;
  const camera = viewer.viewport.camera;
  const fitZoom = camera.zoom;
  let previous;
  canvas.style.touchAction = "none";

  const position = (touches) => {
    if (!touches.length) return undefined;
    const bounds = canvas.getBoundingClientRect();
    const first = touches[0];
    const second = touches[1] ?? first;
    return {
      x: (first.clientX + second.clientX) / 2 - bounds.left,
      y: (first.clientY + second.clientY) / 2 - bounds.top,
      distance: Math.hypot(second.clientX - first.clientX, second.clientY - first.clientY),
      count: Math.min(touches.length, 2),
    };
  };
  const worldPoint = (point) => {
    const screen = camera.center.copy();
    screen.set(point.x, point.y);
    return camera.screen_to_world(screen);
  };
  const begin = (event) => {
    if (!viewer.active) return;
    event.stopImmediatePropagation();
    if (event.touches.length > 1) event.preventDefault();
    // Copy coordinates: retaining a browser TouchList can lose the previous position.
    previous = position(event.touches);
  };
  const move = (event) => {
    if (!viewer.active) {
      previous = undefined;
      return;
    }
    event.stopImmediatePropagation();
    event.preventDefault();
    const current = position(event.touches);
    if (previous && current && previous.count === current.count) {
      const anchor = worldPoint(previous);
      if (previous.distance > 0 && current.distance > 0) {
        camera.zoom = Math.max(
          fitZoom / 20,
          Math.min(fitZoom * 500, (camera.zoom * current.distance) / previous.distance),
        );
      }
      const next = worldPoint(current);
      camera.center.set(camera.center.x + anchor.x - next.x, camera.center.y + anchor.y - next.y);
      viewer.notify_viewport_change();
    }
    previous = current;
  };
  const end = (event) => {
    event.stopImmediatePropagation();
    previous = position(event.touches);
  };
  // Capture prevents Prism's legacy touch handler from applying a second camera move.
  const options = { capture: true, passive: false };
  canvas.addEventListener("touchstart", begin, options);
  canvas.addEventListener("touchmove", move, options);
  canvas.addEventListener("touchend", end, options);
  canvas.addEventListener("touchcancel", end, options);
}
