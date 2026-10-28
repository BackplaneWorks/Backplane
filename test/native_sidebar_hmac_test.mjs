import {createHmac} from 'node:crypto';
import assert from 'node:assert/strict';
// Independent UTF8/domain/hex-key vector also asserted by the Bend fixture.
const payload={version:1,hub:'h',generation:'1',epoch:'epoch',viewId:'view',target:{kind:'thread',id:'a'},metadataRevision:7,listKind:'settled',projectOwner:'p',choicesDigest:'external-vector',membershipDigest:'ordered-ids',offset:16,limit:16};
assert.equal(createHmac('sha256',Buffer.from('00112233445566778899aabbccddeeff','hex')).update('backplane.native-sidebar.v1\n'+JSON.stringify(payload),'utf8').digest('hex'),'31b6ca450712a692a077c3b9f85e0a6387a4d18195c3ef6c1fea86dedaf36a72');
console.log('ok independent Node HMAC domain/key/canonical JSON vector');
