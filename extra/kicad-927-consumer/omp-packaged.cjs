'use strict';
// PREPARED ONLY. Called by the sole consumer after the actual new ZIP handoff.
// Uses the real packaged UI/hub. No lifted effects, seeded DB or RPC injection.
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const crypto=require('node:crypto');
const {execFileSync}=require('node:child_process');
const {DatabaseSync}=require('node:sqlite');
const sha=file=>crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const expectedCLI='c249a9ed0a57c748fb1247d8ee9e2d0fe1b13229266ce74d8810a76a591e21fc';
const normal=s=>s.replaceAll('\\','/').replace(/\/$/,'').toLowerCase();
function read(home,sql,...args) {
  const db=new DatabaseSync(path.join(home,'store.db'),{readOnly:true});
  try {return db.prepare(sql).all(...args);} finally {db.close();}
}
function observers(folder) {
  return fs.readdirSync(folder).filter(s=>/^observer-[0-9]+\.jsonl$/.test(s))
    .flatMap(s=>fs.readFileSync(path.join(folder,s),'utf8').split('\n').filter(Boolean).map(s=>JSON.parse(s)));
}
async function run({cfg,out,openCase,until,save,result}) {
  assert.equal(process.platform,'win32');
  assert.equal(process.env.GITHUB_ACTIONS,'true');
  assert.equal(cfg.omp?.newSourceBoundCandidate,true,'New authoritative ZIP handoff is required');
  assert.notEqual(cfg.packageSHA256,'de613310949b72b0a70bebf9a07aff021cdea98f894856fabdc42134d678cd22','Old frozen package does not qualify');
  assert.equal(sha(path.join(__dirname,'omp-fixture.cjs')),'bc02675857f9339e3ffbe0218674133e4d04e0f421fb85e4dc675364ed7892e0');
  assert.equal(sha(cfg.omp.downloadedCLI),expectedCLI,'Actual real OMP 18.16.0 executable required');
  const shell=path.join(process.env.SystemRoot,'System32/WindowsPowerShell/v1.0/powershell.exe');
  const identity=JSON.parse(execFileSync(shell,['-NoProfile','-NonInteractive','-Command',
    "$i=[Security.Principal.WindowsIdentity]::GetCurrent();$p=[Security.Principal.WindowsPrincipal]::new($i);$d=Get-ItemPropertyValue 'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\AppModelUnlock' AllowDevelopmentWithoutDevLicense -ErrorAction SilentlyContinue;@{sid=$i.User.Value;admin=$p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator);developerMode=($d -eq 1)}|ConvertTo-Json -Compress"],{encoding:'utf8'}));
  assert.equal(identity.sid,cfg.standardUserSid);assert.equal(identity.admin,false);assert.equal(identity.developerMode,false);
  const privileges=execFileSync(path.join(process.env.SystemRoot,'System32/whoami.exe'),['/priv'],{encoding:'utf8'});
  assert(!privileges.includes('SeCreateSymbolicLinkPrivilege'));
  const profile=process.env.USERPROFILE;
  const providerBin=path.join(profile,'.local','bin');fs.mkdirSync(providerBin,{recursive:true});
  const cli=path.join(providerBin,'omp.exe');
  assert(!fs.existsSync(cli),'OMP must be freshly installed into the disposable standard profile');
  fs.copyFileSync(cfg.omp.downloadedCLI,cli);assert.equal(sha(cli),expectedCLI);
  const version=execFileSync(cli,['--version'],{encoding:'utf8',timeout:30000}).trim();
  assert(version.includes('18.16.0'));
  const target=path.join(out,'omp-symlink-target');fs.writeFileSync(target,'synthetic');
  assert.throws(()=>fs.symlinkSync(target,target+'-link'),e=>['EPERM','EACCES'].includes(e.code));
  const project=fs.mkdtempSync(path.join(profile,'bp949-packaged-project-'));
  const fixture=require('./omp-fixture.cjs').prepare(profile,project,out);
  let context;
  try {
    await fixture.startFixture();
    fs.copyFileSync(fixture.evidencePaths.models,path.join(out,'omp-models.yml'));
    fs.copyFileSync(fixture.evidencePaths.extension,path.join(out,'omp-observer-extension.js'));
    const env={...fixture.env,PATH:providerBin+';'+path.join(process.env.SystemRoot,'System32')+';'+process.env.SystemRoot};
    context=await openCase('packaged-omp',null,{env,realOMP:true});
    const {page,home,entry}=context;
    Object.assign(entry,{identity,privileges,installedCLI:cli,cliSHA256:sha(cli),version,
      package:result.package,evidencePaths:fixture.evidencePaths,
      readinessEvidence:'Derived from matching actual packaged-hub persisted session and OMP session_start, plus successful turn; no raw RPC frame capture'});save();
    await page.locator('[data-on-click="proj-open"]').first().click();
    await page.locator('#proj').waitFor({state:'visible'});
    // Drive-prefixed text is a name search in the current project picker.
    // Browse the disposable profile's actual folder rows instead of typing it.
    await page.locator('[data-on-click="proj-browse"]').click();
    const directory=await until('listed disposable profile project folder',async()=>{
      for(const row of await page.locator('[data-on-click="proj-cd"]').all()) {
        if(normal(await row.getAttribute('data-value')||'')===normal(project))return row;
      }
      return false;
    });
    await directory.click();
    const add=await until('real Add project control',async()=>{
      for(const row of await page.locator('[data-on-click="proj-add"]').all()) {
        if(normal(await row.getAttribute('data-value')||'')===normal(project))return row;
      }
      return false;
    });
    await add.click();
    const projectRow=await until('persisted UI-created project',()=>read(home,'SELECT id,root FROM project WHERE removed=0').find(p=>normal(p.root)===normal(project)));
    const thread=await until('real new project thread',()=>read(home,'SELECT id,provider,model,session FROM thread WHERE project=? AND deleted=0 ORDER BY created DESC',projectRow.id)[0]);
    assert(/^[A-Za-z0-9_-]+$/.test(thread.id));entry.threadId=thread.id;save();
    const threadRow=page.locator('[data-on-click="select"][data-value="'+thread.id+'"]');
    if(await threadRow.count())await threadRow.first().click();
    const composer=page.locator('#composer');await composer.waitFor({state:'visible'});
    await page.locator('.model-chip[data-value="models"]').first().click();
    await page.locator('[data-on-click="mp-agent"][data-value="omp"]').click();
    await until('real OMP provider selection',()=>read(home,'SELECT provider FROM thread WHERE id=?',thread.id)[0]?.provider==='omp');
    const model=page.locator('[data-on-click="mp-model"][data-value="'+fixture.model+'"]');
    await model.waitFor({state:'visible',timeout:120000});await model.click();
    await until('real custom model selection',()=>read(home,'SELECT model FROM thread WHERE id=?',thread.id)[0]?.model===fixture.model);
    // Dismiss the picker with its actual shade, leaving the composer active.
    const shade=page.locator('.shade[data-on-click="flag"][data-value="models"]');
    if(await shade.isVisible())await shade.click({position:{x:5,y:5}});
    const text='Synthetic packaged OMP thread acceptance '+fixture.markers.start;
    await composer.fill(text);await composer.press('Enter');
    await until('actual user message persisted',()=>read(home,'SELECT text FROM entry WHERE thread=? AND role=?',thread.id,'user').some(r=>r.text.includes(text)));
    const chosen=await until('same native OMP PID/session and both prompt phases',()=>{
      const rows=observers(fixture.evidencePaths.observerDir);
      const session=read(home,'SELECT provider,model,session,turn FROM thread WHERE id=?',thread.id)[0];
      if(!session?.session||session.provider!=='omp'||session.model!==fixture.model)return false;
      for(const started of rows.filter(r=>r.phase==='session_start'&&r.attestationPass&&r.sessionId===session.session)) {
        const before=rows.find(r=>r.phase==='before_agent_start'&&r.attestationPass&&r.pid===started.pid&&r.sessionId===started.sessionId);
        if(before)return {pid:started.pid,sessionId:started.sessionId,session,rows:[started,before]};
      }
      return false;
    });
    for(const row of chosen.rows) {
      assert.equal(sha(row.execPath),expectedCLI,'Observer must run inside the genuine pinned native OMP executable');
      assert.equal(normal(row.execPath),normal(cli));
      assert(row.promptUTF16Length>32767&&row.allSkillDescriptionsInPrompt&&row.allSkillDescriptionsInSystem);
    }
    entry.observer=chosen.rows;entry.session=chosen.session;
    entry.chosenProcessSession={pid:chosen.pid,sessionId:chosen.sessionId};
    entry.successfulGetStateDerived=true;save();
    await page.getByText(fixture.reply,{exact:true}).first().waitFor({state:'visible',timeout:120000});
    const completed=await until('packaged OMP turn completed',()=>{
      const t=read(home,'SELECT turn,session,provider,model FROM thread WHERE id=?',thread.id)[0];
      return t?.turn==='completed'&&t.session===chosen.sessionId&&t.provider==='omp'&&t.model===fixture.model?t:false;
    });
    const replies=read(home,'SELECT role,text,kind FROM entry WHERE thread=?',thread.id);
    assert(replies.some(r=>r.role==='assistant'&&r.text.includes(fixture.reply)));
    assert(!replies.some(r=>/spawn.*EIO|Input\/output error|no runnable omp|ENOENT|ENAMETOOLONG|E2BIG/.test(r.text)));
    const summary=JSON.parse(fs.readFileSync(fixture.evidencePaths.summary,'utf8'));
    assert(summary.completions>=1);assert.deepEqual(summary.errors,[]);
    entry.completed=completed;entry.fixture=summary;entry.reply=fixture.reply;entry.status='PASS';save();
    await page.screenshot({path:path.join(out,'packaged-omp-complete.png')});
  } finally {
    if(context)await context.page.screenshot({path:path.join(out,'packaged-omp-final.png')}).catch(()=>{});
    await fixture.closeFixture();
  }
}
module.exports={run};
