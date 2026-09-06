import { installSchematicSizing } from "./schematic-sizing.js";
import { installNativeTouch } from "./native-touch.js";
import { installMobileProperties } from "./properties-mobile.js";

// Adapter for unmodified KiCAD-Prism custom elements. A frame owns their workers and WebGL lifetime.
const host = document.getElementById("viewer");
const error = document.getElementById("error");
let viewer;
let chain = Promise.resolve();
let revision;
let probeId;
const send = (message) =>
  parent.postMessage(message, location.origin === "null" ? "*" : location.origin);
window.addEventListener("message", (event) => {
  if (
    event.source !== parent ||
    event.origin !== location.origin ||
    event.data?.type !== "k3eda-snapshot"
  )
    return;
  const snapshot = event.data;
  chain = chain
    .then(async () => {
      error.textContent = "";
      if (snapshot.kind === "model") {
        if (viewer) return;
        error.textContent = "Generating 3D preview…";
        const response = await fetch(snapshot.url);
        if (!response.ok) throw new Error((await response.text()).slice(0, 700));
        const blobUrl = URL.createObjectURL(await response.blob());
        const project = Object.assign(new EventTarget(), {
          loaded: Promise.resolve(),
          ov_3d_url: blobUrl,
        });
        host.addEventListener("context-request", (request) => {
          if (request.context_name === "project") request.callback(project);
        });
        await import("./3d-viewer.js");
        const { finishBoardModel } = await import("./model-appearance.js");
        project.addEventListener("3d:viewer:loaded", () => finishBoardModel(viewer));
        viewer = document.createElement("ecad-3d-viewer");
        host.appendChild(viewer);
        error.textContent = "";
      } else {
        if (!viewer) {
          await import("./ecad-viewer.js");
          viewer = document.createElement("ecad-viewer");
          viewer.setAttribute("source-mode", "host");
          viewer.setAttribute("show-header", "false");
          host.appendChild(viewer);
          viewer.addEventListener("ecad-viewer:selection", (event) =>
            send({ type: "k3eda-selection", selection: event.detail }),
          );
          viewer.addEventListener("ecad-viewer:crossprobe", (event) =>
            send({ type: "k3eda-crossprobe", selection: event.detail }),
          );
        }
        if (revision !== snapshot.revision) {
          await viewer.replaceSources({
            revisionKey: snapshot.revision,
            sources: snapshot.sources,
          });
          revision = snapshot.revision;
        }
        await viewer.ready;
        viewer.setActive(snapshot.active !== false);
        if (snapshot.active !== false) {
          viewer.resize();
          installSchematicSizing(viewer);
          installNativeTouch(viewer);
          installMobileProperties(viewer);
          if (snapshot.probe && probeId !== snapshot.probe.id) {
            probeId = snapshot.probe.id;
            const found = viewer.requestCrossProbe(snapshot.probe);
            send({ type: "k3eda-probe-result", found, value: snapshot.probe.value });
          }
        }
      }
    })
    .catch((cause) => {
      error.textContent = cause.message || String(cause);
    });
});
parent.postMessage(
  { type: "k3eda-runtime-ready" },
  location.origin === "null" ? "*" : location.origin,
);
