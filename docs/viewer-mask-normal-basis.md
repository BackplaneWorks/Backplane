# Generated mask normal basis

The original KiCad GLB has no tangents or normal textures. Pinned Three0.181.1 GLTFLoader.assignFinalMaterial detects absent tangents and negates copied `normalScale.y` for its derivative-tangent convention. The loaded original mask materials therefore have `[1,-1]` even without authored normal maps. Later Realistic Physical policy copies that scale while introducing a default clearcoat scale `[1,1]`.

Generated copper-relief geometry supplies explicit UV and tangents. Its U is native X and V native Z, with tangentW−1 front/+1 back. The explicit bitangent is cross(N,T)*W and points+nativeZ on both faces. Thus the inherited derivative-only flip is inappropriate for this new, owned normal map: base and coat would perturb in opposite V directions.

After the existing authored-map/extension precedence guard, only private generated mask materials explicitly set BOTH normalScale and clearcoatNormalScale to `[1,1]`. Original material objects/JSON/scales, source geometry/normals/matrices, source maps, source PBR/color factors and solder anchors remain untouched. No global loader or source-material behavior changes. The normal texel data and generated UV/tangents are unchanged.

Pinned normal shader chunks multiply mapN.xy by the corresponding scale and normalize TBN*mapN. For texel `[153,166,246]`, explicit front/back native expected normals are approximately `[.200503,+.931750,.302721]` and `[.200503,-.931750,.302721]`. Properly rotating/translating the board preserves that basis. CPU arithmetic does not establish GPU shader, sampling, depth or appearance correctness.

`bun test/tools/solid_mask_normal_basis_test.mjs` includes a synthetic inherited `[1,-1]` regression that fails before the correction. `MASK_NORMAL_GLB=/absolute/full.glb MASK_NORMAL_RECEIPT=/tmp/normal-basis.json bun test/tools/solid_mask_normal_basis_test.mjs` checks both actual loaded mask materials, original scale/JSON preservation and signed texel behavior on both faces under proper rotation. A later authorized actual shader/pixel gate remains necessary. Authored normal maps/extensions still skip generated relief through the existing eligibility guard.
