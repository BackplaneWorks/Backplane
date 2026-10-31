// The original saved model of a board is authorized by the PCB's revision as
// soon as it is known: the assembly (2|root|path) and the PCB (0|root|path)
// are one .kicad_pcb, and the plot actor sends the assembly's revision only
// after building its own thinned mesh. Other models keep their own revision.
// bun test/tools/viewer_assembly_revision_test.mjs
import assert from 'node:assert/strict';
import { sameBoardFile, assemblyRevision } from '../../src/web/viewer-tools.js';

const key = '2|/home/p|boards/a/a.kicad_pcb', board = '0|/home/p|boards/a/a.kicad_pcb';
assert.equal(sameBoardFile(key, board), true);
assert.equal(sameBoardFile(key, ''), false, 'no board key');
assert.equal(sameBoardFile('2|/home/p|mech/case.step', board), false, 'a STEP is not the board');
assert.equal(sameBoardFile(key, '0|/home/p|boards/b/b.kicad_pcb'), false, 'another board');
assert.equal(sameBoardFile('0|/home/p|boards/a/a.kicad_pcb', board), false, 'the PCB itself is not an assembly');
assert.equal(sameBoardFile(key, '1|/home/p|boards/a/a.kicad_pcb'), false, 'a schematic key');

const known = new Map();
assert.equal(assemblyRevision(key, board, known), undefined, 'nothing known yet');
known.set(board, 'r1');
assert.equal(assemblyRevision(key, board, known), 'r1', 'the PCB revision authorizes before the assembly mesh exists');
known.set(key, 'r1');
assert.equal(assemblyRevision(key, board, known), 'r1');
known.set(board, 'r2');
assert.equal(assemblyRevision(key, board, known), 'r2', 'a newer PCB revision wins over a stale assembly one');

const step = '2|/home/p|mech/case.step', other = new Map([[board, 'r9']]);
assert.equal(assemblyRevision(step, board, other), undefined, 'a non-board model waits for its own revision');
other.set(step, 's1');
assert.equal(assemblyRevision(step, board, other), 's1');
{
 const known=new Map(),stamped=new Map();
 stamped.set(board,'s1');assert.equal(assemblyRevision(key,board,known,stamped),'s1','the PCB stamp authorizes before its plot revision');
 known.set(board,'s1');assert.equal(assemblyRevision(key,board,known,stamped),'s1','the plot revision for that stamp changes nothing');
 stamped.set(board,'s2');assert.equal(assemblyRevision(key,board,known,stamped),'s2','a later stamp (the file saved again) wins');
 assert.equal(assemblyRevision(step,board,known,stamped),undefined,'a stamp never authorizes another file');
}
console.log('ok assembly revision: same-file PCB revision authorizes the original board model early; other models keep their own');
