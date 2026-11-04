'use strict';
// Diagnostic only: real native child_process -> packaged Cygwin env/sh.
// No app overlay, mocked command or rewritten health file is involved.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
const {spawnSync}=require('node:child_process');
const cfg=JSON.parse(fs.readFileSync(process.argv[2],'utf8'));
const label=process.argv[3];
const bin=path.join(path.dirname(cfg.executable),'resources','backplane-service','runtime','bin');
const envExe=path.join(bin,'env.exe'),sh=path.join(bin,'sh.exe');
const env={SystemRoot:process.env.SystemRoot,PATH:[bin,path.join(process.env.SystemRoot,'System32'),process.env.SystemRoot].join(';')};
const source=fs.readFileSync(path.join(cfg.frozenSourceRoot,'src/server/kicad.bend'),'utf8');
const line=source.split('\n').find(l=>l.includes('home ++ "/kicad-health.json"')&&l.includes('printf'));
const script=JSON.parse(line.match(/\["-c", ("(?:[^"\\]|\\.)*")/)[1]);
const safe=['GIT_CONFIG_COUNT=5','GIT_CONFIG_KEY_0=core.fsmonitor','GIT_CONFIG_VALUE_0=false','GIT_CONFIG_KEY_1=core.hooksPath','GIT_CONFIG_VALUE_1=/dev/null','GIT_CONFIG_KEY_2=core.pager','GIT_CONFIG_VALUE_2=cat','GIT_CONFIG_KEY_3=diff.ignoreSubmodules','GIT_CONFIG_VALUE_3=dirty','GIT_CONFIG_KEY_4=log.showSignature','GIT_CONFIG_VALUE_4=false','GIT_PAGER=cat'];
const report={scope:'Native argv diagnostic, not application acceptance',engine:{executable:process.execPath,versions:process.versions,sha256:crypto.createHash('sha256').update(fs.readFileSync(process.execPath)).digest('hex')},cases:[]};
const points=s=>Array.from(s,c=>c.codePointAt(0));
for(const [name,payload] of [['raw','C:\\backplane_kicad\\new tools\\test\\'],['json',JSON.stringify({path:'C:\\backplane_kicad\\',other:'D:\\new tools\\test\\',quote:'a"b'})]]) {
  for(const kind of ['direct-sh','env-sh','production-cache-script']) {
    const dest=path.join(cfg.out,'argv-'+label+'-'+name+'.json').replaceAll('\\','/');
    const args=kind==='production-cache-script' ? [...safe,'sh','-c',script,'sh',dest,payload] : [...(kind==='env-sh'?safe:[]),...(kind==='env-sh'?['sh']:[]),'-c','printf \'%s\' "$1"','sh',payload];
    const command=kind==='direct-sh'?sh:envExe;
    const r=spawnSync(command,args,{env,encoding:'utf8',timeout:15000,windowsHide:true});
    let actual=r.stdout||'';
    if(kind==='production-cache-script'&&fs.existsSync(dest))actual=fs.readFileSync(dest,'utf8');
    report.cases.push({name,kind,expected:payload,actual,equal:payload===actual,expectedCodePoints:points(payload),actualCodePoints:points(actual),exit:r.status,error:r.error?.message,stderr:r.stderr});
  }
}
fs.writeFileSync(path.join(cfg.out,'argv-probe-'+label+'.json'),JSON.stringify(report,null,2)+'\n');
