const presentations = new WeakMap();
const fitted = new WeakSet();

/** Apply host presentation after Prism initializes its renderer, without changing layer colors. */
export function installCanvasPresentation(element) {
  const dark = parent.document.documentElement.classList.contains("dark");
  document.documentElement.dataset.colorScheme = dark ? "dark" : "light";
  const background = dark ? "#111719" : "#eef1f0";
  for (const name of ["kc-board-app", "kc-schematic-app"]) {
    const viewer = element.shadowRoot?.querySelector(name)?.viewer;
    if (!viewer || presentations.get(viewer) === background) continue;
    const color = viewer.renderer.background_color.constructor;
    viewer.theme.background = color.from_css(background);
    viewer.renderer.background_color = viewer.theme.background;
    if (viewer.renderer.gl) viewer.renderer.gl.clearColor(...viewer.theme.background.to_array());
    if (!fitted.has(viewer)) {
      const fit = viewer.zoom_fit_top_item.bind(viewer);
      viewer.zoom_fit_top_item = () => {
        fit();
        // Leave room for pads and mounting features that extend beyond the board outline.
        viewer.viewport.camera.zoom *= 0.88;
        viewer.draw();
      };
      viewer.zoom_fit_top_item();
      fitted.add(viewer);
    }
    viewer.paint();
    viewer.draw();
    presentations.set(viewer, background);
  }
  if (element.parentElement.querySelector(".canvas-actions")) return;
  const bar = document.createElement("div");
  bar.className = "canvas-actions";
  bar.setAttribute("role", "toolbar");
  bar.setAttribute("aria-label", "Canvas navigation");
  const activeViewer = () =>
    ["kc-board-app", "kc-schematic-app"]
      .map((name) => element.shadowRoot?.querySelector(name)?.viewer)
      .find((viewer) => viewer?.active);
  for (const [label, text, action] of [
    [
      "Zoom out",
      "−",
      (viewer) => {
        viewer.viewport.camera.zoom /= 1.25;
        viewer.draw();
      },
    ],
    ["Fit design", "Fit design", (viewer) => viewer.zoom_fit_top_item()],
    [
      "Zoom in",
      "+",
      (viewer) => {
        viewer.viewport.camera.zoom *= 1.25;
        viewer.draw();
      },
    ],
  ]) {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = text;
    button.setAttribute("aria-label", label);
    button.addEventListener("click", () => {
      const viewer = activeViewer();
      if (viewer) action(viewer);
    });
    bar.appendChild(button);
  }
  element.parentElement.appendChild(bar);
}
