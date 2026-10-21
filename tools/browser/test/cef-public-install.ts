// Run under bp-admit. Exact release installer/archive, disposable HOME/display.
import {mkdirSync,readFileSync,realpathSync,writeFileSync,chmodSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {check,jsonProcess,privateDisplay,until} from '../../cef/test/harness.ts';
const archive=resolve(process.env.CEF_PUBLIC_ARCHIVE!);
const helper=resolve(process.env.CEF_ACCEPTED_HELPER!);
const host=resolve(process.env.CEF_ACCEPTED_HOST!);
const hash=(p:string)=>new Bun.CryptoHasher('sha256').update(readFileSync(p)).digest('hex');
const d=await privateDisplay();
const prefix=join(d.root,'prefix'),bin=join(d.root,'bin'),home=join(d.root,'home');
for(const path of [bin,home])mkdirSync(path);
const env={...d.env,HOME:home,XDG_CACHE_HOME:join(d.root,'empty-cache'),BACKPLANE_PREFIX:prefix,BACKPLANE_BIN:join(d.root,'links'),PATH:bin+':'+process.env.PATH,BACKPLANE_CEF_NO_GPU:'1',TEST_RELEASE_ARCHIVE:archive,BACKPLANE_NO_DESKTOP_APP:'1'};
delete env.BACKPLANE_CEF;
let h:ReturnType<typeof jsonProcess>|undefined;
async function run(args:string[]){const p=Bun.spawn(args,{env,stdout:'pipe',stderr:'pipe'});const out=await new Response(p.stdout).text(),err=await new Response(p.stderr).text();if(await p.exited)throw Error(err+'\n'+out);return out;}
try{
  const installer=join(d.root,'install.sh');
  writeFileSync(installer,readFileSync('deploy/install.sh.in','utf8').replaceAll('@VERSION@','v9.0.0-cef-test').replaceAll('@SHA_LINUX_X64@',hash(archive)));
  const curl=join(bin,'curl');writeFileSync(curl,'#!/bin/sh\nwhile [ "$#" -gt 0 ]; do case "$1" in -o) out=$2; shift 2;; *) url=$1; shift;; esac; done\ncase "$url" in */backplane-v9.0.0-cef-test-classic-linux-x64.tar.gz) ;; *) exit 2;; esac\ncp "$TEST_RELEASE_ARCHIVE" "$out"\n');chmodSync(curl,0o755);
  await run(['sh',installer]);
  const bundle=realpathSync(join(prefix,'browser-current'));
  const manifest=JSON.parse(readFileSync(join(bundle,'manifest.json'),'utf8'));
  check('clean public installer selects a complete immutable CEF pair',realpathSync(join(prefix,'backplane-browser'))===join(bundle,'backplane-browser')&&realpathSync(join(prefix,'cef'))===join(bundle,'cef'));
  check('clean installation includes pinned license and redistribution notices',!!manifest.files['cef/LICENSE.txt']&&!!manifest.files['cef/CEF-README.txt']);
  h=jsonProcess([join(prefix,'backplane-browser'),'--parent',String(d.parent),'--profile',join(home,'browser-profile'),'--cache',join(home,'cef-profile')],env);
  await h.call('open',{page:'public',url:'data:text/html,'+encodeURIComponent('<title>Installed CEF</title><input id=q><a id=next href="data:text/html,Next">Next</a>')});
  const status=await h.call('status',{page:'public'});
  check('clean installed helper defaults to native CEF',status.engine==='cef'&&!status.fallback);
  check('running helper and sibling host match the shipped manifest',hash(`/proc/${h.proc.pid}/exe`)===hash(helper)&&hash(`/proc/${status.hostPid}/exe`)===hash(host)&&realpathSync(`/proc/${status.hostPid}/exe`)===join(bundle,'cef/backplane-cef'));
  await h.call('show',{page:'public',x:20,y:20,w:600,h:400});
  check('installed native page is mapped',await until(async()=> (await d.x('children')).children.some((v:any)=>v.mapped&&!v.inputOnly&&v.w===600)));
  await h.call('type',{page:'public',selector:'#q',text:'Clean install 日本語'});
  check('installed helper drives the same native page',await h.call('evaluate',{page:'public',expression:"q.value==='Clean install 日本語'"}));
  await h.call('hide');check('installed native page hides',(await d.x('children')).children.every((v:any)=>!v.mapped));
  await h.call('show',{page:'public',x:20,y:20,w:600,h:400});
  await h.call('open',{page:'public',url:'data:text/html,<title>Navigation works</title><input id=q>'});
  check('installed native navigation works',(await h.call('status',{page:'public'})).title==='Navigation works');
  await h.call('engine',{engine:'chrome'});
  check('explicit Chrome choice survives public CEF installation',(await h.call('status',{page:'public'})).engine==='chrome');
  console.log('all actual clean public CEF install checks passed');
}finally{if(h){h.proc.stdin.end();await h.proc.exited;}await d.cleanup();}
