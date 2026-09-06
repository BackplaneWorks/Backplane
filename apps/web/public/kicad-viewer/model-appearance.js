import { ACESFilmicToneMapping, MeshPhysicalMaterial, MeshStandardMaterial, Mesh } from "three";
import { mergeGeometries } from "three/addons/utils/BufferGeometryUtils.js";

// KiCad emits one primitive per copper face. Batch only static board surfaces;
// component models and their hierarchy are left intact.
function batchBoardSurfaces(content) {
  const groups = [];
  content.traverse((object) => {
    if (object.isGroup && /_(copper|pad|via|silkscreen|PCB)(?:_|$)/i.test(object.name))
      groups.push(object);
  });
  for (const group of groups) {
    const batches = new Map();
    for (const mesh of group.children) {
      if (!mesh.isMesh || Array.isArray(mesh.material) || mesh.isSkinnedMesh) continue;
      const batch = batches.get(mesh.material) ?? [];
      batch.push(mesh);
      batches.set(mesh.material, batch);
    }
    for (const [material, meshes] of batches) {
      if (meshes.length < 2) continue;
      const geometries = meshes.map((mesh) => {
        mesh.updateMatrix();
        return mesh.geometry.clone().applyMatrix4(mesh.matrix);
      });
      const geometry = mergeGeometries(geometries);
      geometries.forEach((item) => item.dispose());
      if (!geometry) continue;
      const merged = new Mesh(geometry, material);
      merged.name = group.name;
      for (const mesh of meshes) {
        group.remove(mesh);
        mesh.geometry.dispose();
      }
      group.add(merged);
    }
  }
}

// Prism owns the scene, controls, loading, and rendering. Preserve KiCad's stackup
// colors and opacity; only give the exported soldermask its resin surface finish.
export function finishBoardModel(element) {
  const viewer = element._viewer_container;
  batchBoardSurfaces(viewer.content);
  viewer.content.traverse((mesh) => {
    if (!mesh.isMesh || !/_soldermask(?:_|$)/i.test(mesh.name)) return;
    const finish = (source) => {
      const material = new MeshPhysicalMaterial();
      // MeshStandardMaterial.copy also copies the exporter-supplied color/alpha.
      MeshStandardMaterial.prototype.copy.call(material, source);
      material.roughness = 0.38;
      material.clearcoat = 0.65;
      material.clearcoatRoughness = 0.24;
      material.depthWrite = false;
      return material;
    };
    mesh.material = Array.isArray(mesh.material)
      ? mesh.material.map(finish)
      : finish(mesh.material);
  });
  viewer.state.toneMapping = ACESFilmicToneMapping;
  viewer.state.ambientIntensity = 0.18;
  viewer.state.directIntensity = 2;
  viewer.state.exposure = -0.25;
  viewer.updateLights();
  viewer.render();
}
