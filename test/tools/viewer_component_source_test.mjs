// Every page of a selected board read keeps that board, even after another
// board is opened. The complete response still needs one consistent identity.
import assert from 'node:assert/strict';
import {ViewerRpc} from '../../src/web/viewer-rpc.js';
const calls = [], path = '/project/boards/beta/beta.kicad_pcb';
const rpc = new ViewerRpc(request => {
  calls.push(request);
  queueMicrotask(() => {
    const offset = request.p.offset || 0;
    rpc.receive({t:'reply',id:request.id,ok:true,result:{components:{schema:1,source:path,schematic:'/project/boards/beta/beta.kicad_sch',sourceRevision:'beta-pcb',schematicRevision:'beta-sch',fingerprint:'beta',thicknessMm:1.6,
      components:[{id:`beta-${offset}`,reference:`R${offset+1}`}],page:{offset,total:2,next:offset === 0 ? 1 : null,complete:offset === 1}}}});
  });
});
const data = await rpc.components('thread', {path});
assert.equal(data.source, path);
assert.equal(data.components.length, 2);
assert.deepEqual(calls.map(x=>x.p), [{thread:'thread',path},{thread:'thread',path,offset:1,sourceRevision:'beta-pcb',schematicRevision:'beta-sch'}]);
console.log('ok selected board path stays on every component page');
