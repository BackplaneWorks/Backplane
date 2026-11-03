// Exact core/utf8.bend decoding, including its permissive overlong forms.
// Each invalid lead or interrupted/incomplete sequence emits U+FFFD.
function __bpUtf8Decode(bs) {
  const text = []; let cp = 0, owe = 0;
  for (let cell = bs; cell.$ !== 'Nil'; cell = cell.tail) {
    const x = cell.head;
    if (owe) {
      if ((x & 192) === 128) {
        cp = ((cp << 6) | (x & 63)) >>> 0;
        if (--owe === 0) text.push(String.fromCodePoint(cp > 1114111 || cp >= 55296 && cp <= 57343 ? 65533 : cp));
        continue;
      }
      text.push('\ufffd'); owe = 0;
    }
    if (x < 128) text.push(String.fromCodePoint(x));
    else if ((x & 224) === 192) { cp = x & 31; owe = 1; }
    else if ((x & 240) === 224) { cp = x & 15; owe = 2; }
    else if ((x & 248) === 240) { cp = x & 7; owe = 3; }
    else text.push('\ufffd');
  }
  if (owe) text.push('\ufffd');
  return text.join('');
}

function __bpLinesFeed(l, bs, tag) {
  let part = l.part, done = l.done;
  for (let cell = bs; cell.$ !== 'Nil'; cell = cell.tail) {
    if (cell.head === 10) {
      let forward = {$: 'Nil'};
      for (let p = part; p.$ !== 'Nil'; p = p.tail) forward = {$: 'Con', head: p.head, tail: forward};
      done = {$: 'Con', head: __bpUtf8Decode(forward), tail: done};
      part = {$: 'Nil'};
    } else part = {$: 'Con', head: cell.head, tail: part};
  }
  let lines = {$: 'Nil'};
  for (let p = done; p.$ !== 'Nil'; p = p.tail) lines = {$: 'Con', head: p.head, tail: lines};
  return {$: 'Tuple', fst: {$: tag, part, done: {$: 'Nil'}}, snd: lines};
}

// Utf8.put encodes raw surrogate code points as their original three bytes.
// TextEncoder would normalize them and cannot serve as this implementation.
function __bpUtf8Encode(s) {
  const bytes = [];
  for (const c of s) {
    const cp = c.codePointAt(0);
    if (cp < 128) bytes.push(cp);
    else if (cp < 2048) bytes.push(192 | cp >>> 6, 128 | cp & 63);
    else if (cp < 65536) bytes.push(224 | cp >>> 12, 128 | cp >>> 6 & 63, 128 | cp & 63);
    else bytes.push(240 | cp >>> 18, 128 | cp >>> 12 & 63, 128 | cp >>> 6 & 63, 128 | cp & 63);
  }
  let out = {$: 'Nil'};
  for (let i = bytes.length - 1; i >= 0; i--) out = {$: 'Con', head: bytes[i], tail: out};
  return out;
}
