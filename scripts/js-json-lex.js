// Server-only core/json.bend Json.lex. Keep the pure parser and raw lexemes.
// Unsupported escapes and malformed/incomplete strings take the pure lexer.
function __bpJsonLex(s, fallback, ns) {
  // The reference's Char construction owns errors for raw lone surrogates.
  if (/[\ud800-\udfff]/u.test(s)) return fallback(s);
  const tokens = [], tag = kind => ns + '.' + kind;
  const structural = c => c <= 32 || c === 34 || c === 91 || c === 93 ||
    c === 123 || c === 125 || c === 58 || c === 44;
  for (let i = 0; i < s.length;) {
    const c = s.charCodeAt(i++);
    if (c <= 32) continue;
    if (c === 123 || c === 91) { tokens.push({$: tag('TOpen'), obj: c === 123}); continue; }
    if (c === 125 || c === 93) { tokens.push({$: tag('TClose')}); continue; }
    if (c === 58) { tokens.push({$: tag('TColon')}); continue; }
    if (c === 44) { tokens.push({$: tag('TComma')}); continue; }
    if (c === 34) {
      const start = i - 1; let escaped = false, closed = false;
      for (; i < s.length; i++) {
        const q = s.charCodeAt(i);
        if (q === 92) { escaped = true; i++; }
        else if (q === 34) { closed = true; break; }
      }
      if (!closed) return fallback(s);
      let text;
      if (!escaped) text = s.slice(start + 1, i);
      else {
        try { text = JSON.parse(s.slice(start, i + 1)); }
        catch { return fallback(s); }
        // Escaped unpaired UTF-16 units become replacement characters in
        // the reference lexer. Unicode mode leaves valid pairs untouched.
        text = text.replace(/[\ud800-\udfff]/gu, '\ufffd');
      }
      i++; tokens.push({$: tag('TStr'), text}); continue;
    }
    const start = i - 1;
    while (i < s.length && !structural(s.charCodeAt(i))) i++;
    const raw = s.slice(start, i);
    tokens.push(c === 45 || c >= 48 && c <= 57 ?
      {$: tag('TNum'), raw} : {$: tag('TWord'), word: raw});
  }
  let out = {$: 'Nil'};
  for (let i = tokens.length - 1; i >= 0; i--) out = {$: 'Con', head: tokens[i], tail: out};
  return out;
}
