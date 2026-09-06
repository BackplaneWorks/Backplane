export type GerberPreset = "layer" | "copper" | "front" | "back" | "all";
const layerSuffix =
  /(?:[-_.](?:F|B|In\d+)[-_.](?:Cu|SilkS|Mask|Paste|Fab|CrtYd)|[-_.]Edge[-_.]Cuts|[-_.](?:PTH|NPTH))$/i;
export function gerberFamily(path: string): string {
  return path.replace(/\.[^./]+$/, "").replace(layerSuffix, "");
}
export function gerberLayer(path: string): string {
  const name = path.split("/").at(-1) ?? path;
  if (/(?:F[-_.]Cu\.|\.gtl$)/i.test(name)) return "front-copper";
  if (/(?:B[-_.]Cu\.|\.gbl$)/i.test(name)) return "back-copper";
  if (/(?:In\d+[-_.]Cu\.|\.g\d+$)/i.test(name)) return "inner-copper";
  if (/(?:F[-_.](?:SilkS|Fab)\.|\.gto$)/i.test(name)) return "front-placement";
  if (/(?:B[-_.](?:SilkS|Fab)\.|\.gbo$)/i.test(name)) return "back-placement";
  if (/(?:F[-_.]Paste\.|\.gtp$)/i.test(name)) return "front-paste";
  if (/(?:B[-_.]Paste\.|\.gbp$)/i.test(name)) return "back-paste";
  if (/(?:Edge[-_.]Cuts\.|\.(?:gko|gm1)$)/i.test(name)) return "outline";
  if (/\.(?:drl|xln)$/i.test(name)) return "drill";
  return "other";
}
export function gerberPresetPaths(
  paths: string[],
  selected: string,
  preset: GerberPreset,
): string[] {
  if (preset === "layer")
    return [
      selected,
      ...paths.filter(
        (path) =>
          path !== selected &&
          gerberFamily(path) === gerberFamily(selected) &&
          gerberLayer(path) === "outline",
      ),
    ];
  const family = gerberFamily(selected);
  return paths.filter((path) => {
    if (gerberFamily(path) !== family) return false;
    const layer = gerberLayer(path);
    if (preset === "all") return true;
    if (layer === "outline" || layer === "drill") return true;
    if (preset === "copper") return layer.endsWith("copper");
    return layer === `${preset}-placement` || layer === `${preset}-paste`;
  });
}
