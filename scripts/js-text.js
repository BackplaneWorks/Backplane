// Base lowercases ASCII capitals only. Its Char constructor owns errors
// for lone surrogates; the retained pure path keeps that error behavior.
function __bpLower(s, fallback) {
  if (/[\ud800-\udfff]/u.test(s)) return fallback(s);
  return s.replace(/[A-Z]/g, c => String.fromCharCode(c.charCodeAt(0) + 32));
}
function __bpTake(s, n) {
  if (s.length <= n) return s;
  let i = 0;
  while (n-- > 0 && i < s.length) i += s.codePointAt(i) > 65535 ? 2 : 1;
  return s.slice(0, i);
}
// A pair is one step and a lone surrogate one, as in Base's walk.
function __bpDrop(s, n) {
  if (n >= s.length) return "";
  let i = 0;
  while (n-- > 0 && i < s.length) i += s.codePointAt(i) > 65535 ? 2 : 1;
  return s.slice(i);
}
// The Unicode 16 folding table stays authoritative outside ASCII.
function __bpFoldText(s, fallback) {
  return /^[\x00-\x7f]*$/.test(s) ? __bpLower(s, fallback) : fallback(s);
}
