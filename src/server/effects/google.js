// The JS twin of google.c: the Google OAuth client baked in at compile
// time. scripts/build-js.sh compiles a prelude in (scripts/google-header.py
// writing .js) that sets globalThis.BACKPLANE_BAKED; the emitted JS never
// holds it. Without one every value is "" and Backplane's own
// G.Google.shared (or Own client) is used.

function gf_baked(name) {
  const baked = globalThis.BACKPLANE_BAKED || {};
  const v = name === "BACKPLANE_GOOGLE_CLIENT_ID" || name === "BACKPLANE_GOOGLE_CLIENT_SECRET"
    ? baked[name] : undefined;
  return typeof v === "string" ? v : "";
}

io_eff(CID(Gf.baked), gf_baked);
