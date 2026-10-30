// Run after: bend src/core/components.bend -o /tmp/components.mjs
// bun test/tools/components_test.mjs /tmp/components.mjs
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {pathToFileURL} from 'node:url';
import {resolve} from 'node:path';
import {ComponentStore,hitComponent,padOutline} from '../../src/web/components.js';
const {default:C}=await import(pathToFileURL(resolve(process.argv[2]||'/tmp/components.mjs')));
export const pcb=`(kicad_pcb (version 20250114) (general (thickness 1.6))
 (footprint "Resistor_SMD:R_0603_1608Metric" (uuid "resistor") (layer "F.Cu") (at 20 30 90)
  (property "Reference" "R1") (property "Value" "10k") (path "/root/rsym")
  (fp_line (start -2 -1) (end 2 -1) (stroke (width 0.1)) (layer "F.SilkS"))
  (pad "1" smd roundrect (uuid "p1") (at 2 0 90) (size 2 1) (roundrect_rratio 0.25) (layers "F.Cu") (net 1 "GND"))
  (pad "2" smd rect (uuid "p2") (at -2 0 90) (size 2 1) (layers "F.Cu") (net 2 "signal;quoted=net"))
  (model "source-resistor.step" (offset (xyz 1 2 3)) (scale (xyz 1 1 1)) (rotate (xyz 0 0 0))))
 (footprint "Connector:Real" (uuid "connector") (layer "B.Cu") (transform (translate 40 50) (rotate -90) (scale 1 1))
  (property "Reference" "J2") (property "Value" "USB-C") (property "MPN" "<not-html>&quoted")
  (pad "A1" smd oval (at 0 2 -90) (size 4 2) (layers "B.Cu") (net 1 "GND"))
  (pad "NP" np_thru_hole circle (at 0 0) (size 1 1) (drill 1) (layers "*.Cu"))
  (pad "C" smd custom (at 5 0) (size 1 1) (layers "B.Cu"))))`;
export const sch=`(kicad_sch (version 20250114) (uuid "root")
 (lib_symbols (symbol "Device:R" (pin_names (offset 0))
  (symbol "R_0_1" (rectangle (start -1 2) (end 1 -2) (stroke (width 0.2)) (fill (type none))))
  (symbol "R_1_1" (pin passive line (at 0 4 270) (length 2) (name "~" (effects (font (size 1 1)))) (number "1" (effects (font (size 1 1)))))
   (pin passive line (at 0 -4 90) (length 2) (name "~" (effects (font (size 1 1)))) (number "2" (effects (font (size 1 1))))))))
 (symbol (lib_id "Device:R") (at 50 60 0) (unit 1) (uuid "rsym")
  (property "Reference" "R1" (at 52 60 0)) (property "Value" "10k" (at 52 62 0)) (property "MPN" "SOURCE-MPN" (at 50 60 0) (effects (font (size 1 1)) (hide yes)))))`;
const inspect=(p=pcb,s=sch)=>JSON.parse(C.inspect(p,s,'/saved/main.kicad_pcb','/saved/main.kicad_sch'));
export const data=inspect();
assert.equal(data.schema,1);assert.equal(data.thicknessMm,1.6);assert.equal(data.components.length,2);
const [r,j]=data.components;
assert.equal(r.id,'resistor');assert.equal(r.mpn,'SOURCE-MPN');assert.equal(r.symbol.uuid,'rsym');assert.equal(r.symbol.libId,'Device:R');assert(r.symbol.items.length>5,'actual cached symbol body, pins and fields');
assert.deepEqual(r.pads[0].board,[20,28]);assert.deepEqual(r.pads[1].board,[20,32]);assert.equal(r.pads[1].net,'signal;quoted=net');assert.equal(r.pads[0].number,'1');
assert.equal(r.graphics.length,1);assert.equal(r.models[0].path,'source-resistor.step');assert.deepEqual(r.models[0].offset,[1,2,3]);
assert.deepEqual(j.at,[40,50,-90]);assert.deepEqual(j.pads[0].board,[38,50]);assert.equal(j.symbol,null);assert.equal(j.mpn,'<not-html>&quoted');
assert.equal(inspect(pcb,sch.replace('(uuid "rsym")','(uuid "another")')).components[0].symbol,null,'reference alone cannot join a symbol');
assert.equal(inspect(pcb,sch.replace('(property "Reference" "R1"','(property "Reference" "R9"')).components[0].symbol,null,'UUID with different reference must not silently join');
assert.equal(inspect(pcb,sch.replace('(symbol (lib_id','(symbol (lib_name "not-cached") (lib_id')).components[0].symbol,null,'missing cached definition is not substituted');
for(const p of ['',pcb.slice(0,-1),'(not_a_board (footprint "x"))'])assert(inspect(p).error);
assert.equal(hitComponent(data,20,28)?.id,'resistor');assert.equal(hitComponent(data,20.49,28.99),null,'roundrect corner is outside real copper');
assert.equal(hitComponent(data,20,28,{off:2**12}),null,'hidden top pads do not intercept');
assert.equal(hitComponent(data,38,50)?.id,'connector');assert.equal(hitComponent(data,40,50),null,'NPTH is not copper');assert.equal(hitComponent(data,40,45),null,'unsupported custom pad is not approximated');
assert.equal(hitComponent(data,20,30),null,'gap between pads is not a bounding box hit');assert.equal(hitComponent(data,0,0),null,'no nearest-ref guess');
assert.equal(hitComponent({...data,components:[r,{...r,id:'overlapping'}]},20,28),null,'overlapping identities remain ambiguous');
assert(padOutline(r,r.pads[0]).every(p=>p.every(Number.isFinite)));assert.deepEqual(padOutline(j,j.pads[2]),[]);
const store=new ComponentStore('0|root|',data);let selected=[];const off=store.subscribe(c=>selected.push(c?.id||''));store.select('resistor');assert.equal(store.current(),r);store.select('unknown');assert.equal(store.current(),null);store.select('resistor');
store.setData('0|root|',{...data,sourceRevision:'another-save'});assert.equal(store.current(),null,'same content fingerprint does not survive saved revision change');
store.setData('0|other|',data);store.select('connector');store.setData('0|root|',data);assert.equal(store.current(),null,'cross-source selection invalidates');
store.setData('0|root|',{...data,components:[r,r]});assert(store.error);assert.equal(store.current(),null,'ambiguous UUIDs cannot select');off();assert(selected.length>=4);
// The repository's real TRS fixture must agree with the same board parser.
const actual=inspect(readFileSync('test/fixtures/fork-viewers/transforms.kicad_pcb','utf8'),'');
assert.deepEqual(actual.components[0].at,[20,30,90]);assert.deepEqual(actual.components[0].pads[0].board,[20,28]);assert.equal(actual.components[1].layer,'B.Cu');
console.log('ok actual Bend saved-component extraction: UUID+ref+cached-symbol identity, pad geometry/nets, source MPN, TRS, absent data, source revisions and unambiguous copper picking');
