/* OpenCascade runs off the UI thread; the owning frame terminates this worker on close. */
importScripts("./occt/occt-import-js.js");
self.onmessage = async ({ data }) => {
  try {
    const occt = await occtimportjs({
      locateFile: (filename) => new URL(`./occt/${filename}`, self.location.href).href,
    });
    const result = occt.ReadStepFile(new Uint8Array(data), {
      linearUnit: "millimeter",
      linearDeflectionType: "bounding_box_ratio",
      linearDeflection: 0.001,
      angularDeflection: 0.5,
    });
    if (!result.success || !result.meshes?.length)
      throw new Error("No solid geometry could be read from this STEP file.");
    self.postMessage({ meshes: result.meshes });
  } catch (error) {
    self.postMessage({ error: error.message || String(error) });
  }
};
