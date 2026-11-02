// A v4 peer opts into a JSON envelope; old peers receive the original body.
// The decoded response is limited independently of curl and Content-Length.
const FAR_CODEC_LIMIT = 1048576;
function far_codec_pack(response) {
  const bytes = [];
  for (let p = response; p.$ === CID(Con); p = p.tail) bytes.push(p.head);
  const original = Buffer.from(bytes), split = original.indexOf('\r\n\r\n');
  if (split < 0 || !original.subarray(0, split).toString('ascii').startsWith('HTTP/1.1 200 OK\r\n')) return response;
  const body = original.subarray(split + 4);
  if (body.length < 1024 || body.length > FAR_CODEC_LIMIT) return response;
  try {
    const data = require('node:zlib').gzipSync(body, { level: 1 }).toString('base64');
    const encoded = Buffer.from(JSON.stringify({ farEncoding: 'gzip-base64-v1', data }));
    if (encoded.length >= body.length || encoded.length > FAR_CODEC_LIMIT) return response;
    const head = Buffer.from(`HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: ${encoded.length}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n`);
    const result = Buffer.concat([head, encoded]);
    let out = { $: CID(Nil) };
    for (let i = result.length - 1; i >= 0; i--) out = { $: CID(Con), head: result[i], tail: out };
    return out;
  } catch { return response; }
}
function far_codec_unpack(body) {
  let obj;
  try { obj = JSON.parse(body); } catch { return body; }
  try {
    if (!obj || typeof obj !== 'object' || !Object.hasOwn(obj, 'farEncoding')) return body;
    if (obj.farEncoding !== 'gzip-base64-v1' || typeof obj.data !== 'string' || obj.data.length > FAR_CODEC_LIMIT || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(obj.data)) return '';
    const bytes = require('node:zlib').gunzipSync(Buffer.from(obj.data, 'base64'), { maxOutputLength: FAR_CODEC_LIMIT });
    const text = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
    JSON.parse(text); // malformed payloads never become a partial mirror batch
    return text;
  } catch { return ''; }
}
io_eff(CID(FarCodec.pack), far_codec_pack);
io_eff(CID(FarCodec.unpack), far_codec_unpack);
