// Keep the upstream Prism renderer intact; adapt its CSS-pixel canvas to this host.
const installed = new WeakSet();

export function installSchematicSizing(element) {
  const app = element.shadowRoot?.querySelector("kc-schematic-app");
  const schematic = app?.viewer;
  if (!schematic || installed.has(schematic)) return;
  installed.add(schematic);
  const renderer = schematic.renderer;
  const canvas = renderer.canvas;
  const clear = renderer.clear_canvas.bind(renderer);
  renderer.update_canvas_size = () => {
    const bounds = canvas.getBoundingClientRect();
    const width = Math.round(bounds.width * devicePixelRatio);
    const height = Math.round(bounds.height * devicePixelRatio);
    if (canvas.width !== width || canvas.height !== height) {
      canvas.width = width;
      canvas.height = height;
    }
  };
  renderer.clear_canvas = () => {
    clear();
    // Prism's layer transforms and pointer coordinates remain in CSS pixels.
    renderer.ctx2d.setTransform(devicePixelRatio, 0, 0, devicePixelRatio, 0, 0);
  };

  const panel = app.shadowRoot?.querySelector("kc-schematic-properties-panel");
  let previousFit;
  let previousSize;
  const resize = () => {
    const bounds = canvas.getBoundingClientRect();
    if (!bounds.width || !bounds.height) return;
    const sidebar = panel?.getBoundingClientRect();
    const mobile = matchMedia("(max-width: 640px)").matches;
    const right = !mobile && sidebar?.width ? Math.max(0, bounds.right - sidebar.left) : 0;
    const bottom = mobile && sidebar?.height ? Math.max(0, bounds.bottom - sidebar.top) : 0;
    const size = `${bounds.width}:${bounds.height}:${right}:${bottom}`;
    const camera = schematic.viewport.camera;
    const zoom = camera.zoom;
    const center = { x: camera.center.x, y: camera.center.y };
    const fitted =
      previousFit === undefined ||
      (Math.abs(zoom / previousFit.zoom - 1) < 0.001 &&
        Math.hypot(center.x - previousFit.x, center.y - previousFit.y) * zoom < 1);
    renderer.update_canvas_size();
    schematic.viewport.sync_from_canvas();
    element.setViewportInsets({ right, bottom });
    if (previousSize !== size) {
      schematic.zoom_fit_top_item();
      previousFit = { zoom: camera.zoom, x: camera.center.x, y: camera.center.y };
      if (!fitted) {
        camera.zoom = zoom;
        camera.center.set(center.x, center.y);
      }
      previousSize = size;
    }
    schematic.draw();
  };
  const observer = new ResizeObserver(resize);
  observer.observe(canvas);
  if (panel) observer.observe(panel);
  // Browser zoom can change DPR without changing the canvas's CSS dimensions.
  window.addEventListener("resize", resize);
  resize();
}
