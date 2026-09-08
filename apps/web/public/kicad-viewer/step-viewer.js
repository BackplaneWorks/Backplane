import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";

/** On-demand rendering keeps an idle model from continually repainting. */
export async function showStep(host, url, status) {
  status.textContent = "Loading STEP file…";
  const response = await fetch(url);
  if (!response.ok) throw new Error((await response.text()).slice(0, 700));
  const buffer = await response.arrayBuffer();
  if (buffer.byteLength > 100 * 1024 * 1024)
    throw new Error("STEP preview is limited to 100 MB per file.");
  status.textContent = "Preparing STEP geometry…";
  const worker = new Worker("/kicad-viewer/step-worker.js");
  const stop = () => worker.terminate();
  window.addEventListener("pagehide", stop, { once: true });
  let meshes;
  try {
    meshes = await new Promise((resolve, reject) => {
      worker.onmessage = ({ data }) =>
        data.error ? reject(new Error(data.error)) : resolve(data.meshes);
      worker.onerror = () => reject(new Error("Unable to load the STEP importer."));
      worker.postMessage(buffer, [buffer]);
    });
  } finally {
    stop();
    window.removeEventListener("pagehide", stop);
  }
  const scene = new THREE.Scene();
  scene.background = new THREE.Color("#101214");
  const group = new THREE.Group();
  for (const mesh of meshes) {
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute(
      "position",
      new THREE.Float32BufferAttribute(mesh.attributes.position.array, 3),
    );
    geometry.setIndex(mesh.index.array);
    if (mesh.attributes.normal)
      geometry.setAttribute(
        "normal",
        new THREE.Float32BufferAttribute(mesh.attributes.normal.array, 3),
      );
    else geometry.computeVertexNormals();
    const material = new THREE.MeshStandardMaterial({
      color: mesh.color ? new THREE.Color(...mesh.color) : new THREE.Color("#a8b5bf"),
      metalness: 0.15,
      roughness: 0.55,
      side: THREE.DoubleSide,
    });
    const object = new THREE.Mesh(geometry, material);
    object.name = mesh.name;
    group.add(object);
  }
  const bounds = new THREE.Box3().setFromObject(group);
  const size = bounds.getSize(new THREE.Vector3());
  const radius = size.length() / 2;
  if (!Number.isFinite(radius) || radius <= 0)
    throw new Error("The STEP file has no viewable geometry.");
  group.position.sub(bounds.getCenter(new THREE.Vector3()));
  scene.add(group);
  scene.add(new THREE.HemisphereLight(0xffffff, 0x58606b, 2.5));
  const light = new THREE.DirectionalLight(0xffffff, 3);
  light.position.set(1, -2, 3);
  scene.add(light);
  const camera = new THREE.PerspectiveCamera(45, 1, radius / 1000, radius * 100);
  camera.up.set(0, 0, 1);
  const renderer = new THREE.WebGLRenderer({ antialias: true });
  renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
  renderer.domElement.style.touchAction = "none";
  host.appendChild(renderer.domElement);
  const controls = new OrbitControls(camera, renderer.domElement);
  controls.enableDamping = false;
  const render = () => renderer.render(scene, camera);
  controls.addEventListener("change", render);
  const fit = () => {
    const angle = Math.min(
      (camera.fov * Math.PI) / 360,
      Math.atan(Math.tan((camera.fov * Math.PI) / 360) * camera.aspect),
    );
    camera.position.copy(
      new THREE.Vector3(1, -1, 0.8).normalize().multiplyScalar((radius / Math.sin(angle)) * 1.15),
    );
    controls.target.set(0, 0, 0);
    controls.update();
    render();
  };
  let firstSize = true;
  const resize = new ResizeObserver(() => {
    const width = host.clientWidth;
    const height = host.clientHeight;
    if (!width || !height) return;
    renderer.setSize(width, height);
    camera.aspect = width / height;
    camera.updateProjectionMatrix();
    if (firstSize) {
      firstSize = false;
      fit();
    }
    render();
  });
  resize.observe(host);
  const button = document.createElement("button");
  button.textContent = "Fit model";
  button.style.cssText =
    "position:absolute;top:12px;left:12px;padding:8px 12px;background:#202428;color:#eee;border:1px solid #596168;border-radius:4px;cursor:pointer";
  button.onclick = fit;
  host.appendChild(button);
  status.textContent = "";
  window.addEventListener(
    "pagehide",
    () => {
      resize.disconnect();
      controls.dispose();
      for (const mesh of group.children) {
        mesh.geometry.dispose();
        mesh.material.dispose();
      }
      renderer.dispose();
    },
    { once: true },
  );
}
