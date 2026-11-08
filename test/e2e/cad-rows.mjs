// C02 (KiCad) from the hub: bun test/e2e/cad-rows.mjs HUB_BIN OUTDIR [port]. A fixture hub with the ecc83 board; Settings > CAD's kicad.path is set; the hub then runs kicad-cli
// itself for review.checks (ERC + DRC) and review.release (exports). Prints {id,check,ok,evidence} lines. KICAD_CLI (default /usr/bin/kicad-cli) is the path to set.
import {writeFileSync,existsSync,readdirSync,readFileSync} from 'node:fs';
import {startFixtureHub} from './lib/hubfx.mjs';
import {client,sleep} from '../perf/lib.mjs';
const [bin,out,port0]=process.argv.slice(2);process.env.NO_SCOPE='1';Object.assign(process.env,{GIT_AUTHOR_NAME:'e2e',GIT_AUTHOR_EMAIL:'e2e@x',GIT_COMMITTER_NAME:'e2e',GIT_COMMITTER_EMAIL:'e2e@x'}); // a release tags: git wants an identity
const cli=process.env.KICAD_CLI||'/usr/bin/kicad-cli';const res=[];const rec=(id,check,ok,evidence)=>{const o={id,check,ok,evidence};res.push(o);console.log(JSON.stringify(o))};
const fx=await startFixtureHub({bin,root:out,port:Number(port0||3860),threads:1,seedTurns:false});
import('node:child_process').then(()=>{});
const {execSync}=await import('node:child_process');execSync(`git add -A && git -c user.name=e2e -c user.email=e2e@x commit -qm fixture`,{cwd:fx.state.root+'/proj'});
const c=fx.client;const th=fx.state.threads[0];
const s=await c.rpc('setting.set',{key:'kicad.path',value:cli},10000);rec('C02','Settings > CAD kicad.path accepted',s?.o?.ok===true,`setting.set kicad.path=${cli} -> ${JSON.stringify(s?.o).slice(0,100)}`);
// the CAD page shows what the hub found
const {viewClient}=await import('../perf/view.mjs');
const v=await viewClient(fx.hub,{mobile:true,delta:false,native:true});await v.open();await v.req('view.action',{action:'flag',value:'settings'});
let rows=[],note='';for(let i=0;i<20;i++){const pg=await v.req('view.action',{action:'settings-page',value:'cad'});rows=pg?.x?.result?.shown?.settings?.rows||[];note=rows.find(r=>r.label==='KiCad')?.note||'';if(note.includes(cli))break;await sleep(1500)}   // until the page names the CLI that was set: the hub republishes it a moment after the setting; it counts only if it never does within 30 srec('C02','CAD page reports the CLI the hub will run',note.includes(cli),`KiCad row: ${note}`);
rec('S05','CAD page: FreeCAD row',!!rows.find(r=>r.label==='FreeCAD'),`FreeCAD row: ${rows.find(r=>r.label==='FreeCAD')?.note} (read-only: no path control on this page)`);
v.close();
const r=await c.rpc('review.checks',{thread:th,trigger:'manual'},180000);const rep=r?.o?.result?.report||r?.o?.result||{};
const txt=JSON.stringify(rep);rec('C02','hub runs kicad-cli ERC+DRC (review.checks)',r?.o?.ok===true&&!rep.err&&/Design checks/.test(txt),`ok=${r?.o?.ok} report ${txt.slice(0,360)}`);
execSync(`git add -A && git -c user.name=e2e -c user.email=e2e@x commit -qm checks --allow-empty`,{cwd:fx.state.root+'/proj'}); // the checks wrote into .backplane; a release needs a clean tree
const rel=await c.rpc('review.release',{thread:th,tag:'e2e-r1',publish:false,force:true},300000);const rt=JSON.stringify(rel?.o?.result||rel?.o).slice(0,500);
const relDir=fx.state.root+'/proj/.backplane/releases/e2e-r1';let files=[];try{files=readdirSync(relDir,{recursive:true})}catch{}
rec('C02','hub runs kicad-cli exports (review.release: gerbers, bom, ...)',files.length>0&&!JSON.parse(JSON.stringify(rel?.o?.result?.report?.err||'')),`files under ${relDir}: ${files.slice(0,12).join(', ')}; reply ${rt.slice(0,250)}`);
writeFileSync(out+'/cad-rows.json',JSON.stringify(res,null,1));await fx.stop();process.exit(0);
