// A pairing link keeps its token in the URL fragment
// (http://host:3787/#token=<hex>) so the token never reaches a server log.
// The page then drops the fragment. Read and store the token first.

export const TOKEN_KEY = "backplane-token";

// the hex token in a fragment, or ""
export function tokenIn(hash) {
  const m = String(hash ?? "").match(/token=([0-9a-f]+)/);
  return m ? m[1] : "";
}

// Persist a fragment's token, then run clear (the page uses it to drop the
// fragment). Returns the token to send: the one just stored, or the one
// kept from an earlier visit.
export function takeToken(hash, storage, clear) {
  const found = tokenIn(hash);
  if (found) storage.setItem(TOKEN_KEY, found);
  if (hash) clear();
  return storage.getItem(TOKEN_KEY) ?? "";
}
