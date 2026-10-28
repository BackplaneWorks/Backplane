// Provider-owned authentication and child-process IO. No inference requests.
// Only allowlisted metadata leaves this actor; raw CLI output is never logged.
const bp_account_managers = new Map();
const bp_account_unset = [
  "CODEX_HOME", "CODEX_SQLITE_HOME", "CODEX_ACCESS_TOKEN", "OPENAI_API_KEY",
  "OPENAI_BASE_URL", "ACCESS_TOKEN", "OPENAI_IDENTITY_TOKEN_FILE",
  "OPENAI_FEDERATION_RULE_ID", "OPENAI_ORGANIZATION_ID", "CODEX_AUTH_TOKEN",
  "CLAUDE_CONFIG_DIR", "CLAUDE_CODE_OAUTH_TOKEN", "CLAUDE_CODE_OAUTH_REFRESH_TOKEN",
  "CLAUDE_CODE_OAUTH_SCOPES", "ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN",
  "ANTHROPIC_BASE_URL", "ANTHROPIC_CUSTOM_HEADERS", "ANTHROPIC_PROFILE",
  "ANTHROPIC_CONFIG_DIR", "ANTHROPIC_FEDERATION_RULE_ID", "ANTHROPIC_ORGANIZATION_ID",
  "ANTHROPIC_IDENTITY_TOKEN_FILE", "CLAUDE_CODE_USE_BEDROCK", "CLAUDE_CODE_USE_VERTEX",
  "CLAUDE_CODE_USE_FOUNDRY", "CLAUDE_CODE_USE_ANTHROPIC_AWS", "ANTHROPIC_AWS_API_KEY",
  "CLAUDE_CODE_API_KEY_HELPER", "CLAUDE_CODE_PROJECT_DIR_NAME",
];

class BPAccountRpc {
  constructor(command, args, env, timeout = 12000) {
    const cp = require("node:child_process");
    this.next = 0; this.pending = new Map(); this.handlers = new Set(); this.buffer = "";
    this.child = cp.spawn(command, args, {env, stdio: ["pipe", "pipe", "ignore"], windowsHide: true});
    this.timer = setTimeout(() => this.close(), timeout);
    this.child.stdout.on("data", chunk => this.data(chunk));
    this.child.on("error", () => this.close()); this.child.on("exit", () => this.close());
  }
  data(chunk) {
    this.buffer += chunk.toString("utf8");
    if (Buffer.byteLength(this.buffer) > 262144) return this.close();
    for (let cut; (cut = this.buffer.indexOf("\n")) >= 0;) {
      const line = this.buffer.slice(0, cut); this.buffer = this.buffer.slice(cut + 1);
      try {
        const msg = JSON.parse(line);
        if (Object.hasOwn(msg, "id") && this.pending.has(msg.id)) {
          const {resolve, reject} = this.pending.get(msg.id); this.pending.delete(msg.id);
          if (msg.error) reject(new Error("Provider request failed")); else resolve(msg.result);
        } else if (typeof msg.method === "string" && !Object.hasOwn(msg, "id")) {
          for (const f of this.handlers) f(msg);
        }
      } catch (_) { /* Non-protocol output carries no authority. */ }
    }
  }
  request(method, params = {}) {
    if (this.closed) return Promise.reject(new Error("Provider connection closed"));
    const id = ++this.next;
    return new Promise((resolve, reject) => {
      this.pending.set(id, {resolve, reject});
      this.child.stdin.write(JSON.stringify({id, method, params}) + "\n");
    });
  }
  async initialize() {
    await this.request("initialize", {clientInfo: {name: "backplane_accounts", version: "1"}, capabilities:{experimentalApi:true}});
    this.child.stdin.write('{"method":"initialized","params":{}}\n');
  }
  close() {
    if (this.closed) return;
    this.closed = true; clearTimeout(this.timer);
    for (const {reject} of this.pending.values()) reject(new Error("Provider connection closed"));
    this.pending.clear(); this.handlers.clear();
    this.child.stdin.destroy(); this.child.stdout.destroy(); this.child.kill();
  }
}

class BPAccountsIO {
  constructor(home) {
    this.fs = require("node:fs"); this.path = require("node:path"); this.crypto = require("node:crypto");
    const canonicalHome = this.fs.realpathSync(home);
    this.privateDir(this.path.join(canonicalHome, "providers"));
    this.root = this.path.join(canonicalHome, "providers", "accounts-v1");
    this.profiles = []; this.attempts = new Map(); this.jobs = new Map(); this.launchQueues = new Map(); this.children = new Map(); this.revision = 0;
    this.privateDir(this.root);
    this.index = this.path.join(this.root, "profiles.json");
    try {
      const st = this.fs.lstatSync(this.index);
      if (!st.isFile() || st.isSymbolicLink() || st.size > 131072 || (st.mode & 0o077)) throw new Error();
      const saved = JSON.parse(this.fs.readFileSync(this.index, "utf8"));
      if (saved.version !== 1 || !Array.isArray(saved.profiles) || saved.profiles.length > 40) throw new Error();
      for (const p of saved.profiles) {
        if (!this.validId(p.id) || !["claude", "codex"].includes(p.provider) || !Number.isSafeInteger(p.epoch) || p.epoch < 1) throw new Error();
        this.profiles.push({id:p.id, provider:p.provider, label:this.label(p.label), epoch:p.epoch,
          fingerprint:typeof p.fingerprint === "string" ? p.fingerprint : "", store:this.validId(p.store)?p.store:"", state:"unchecked",
          identity: {}, catalog: [], allowance: null, checkedAt: 0});
      }
    } catch (e) {
      if (e.code !== "ENOENT") this.loadError = "The private account registry could not be verified.";
    }
    this.fs.chmodSync(this.root, 0o700);
  }
  validId(id) { return typeof id === "string" && /^[a-f0-9-]{36}$/.test(id); }
  label(value) { return String(value ?? "").replace(/[\u0000-\u001f\u007f]/g, "").slice(0, 64).trim(); }
  privateDir(dir) {
    const parent = this.path.dirname(dir);
    if (!this.fs.existsSync(parent)) this.fs.mkdirSync(parent, {recursive:true, mode:0o700});
    try { this.fs.mkdirSync(dir, {mode:0o700}); } catch (e) { if (e.code !== "EEXIST") throw e; }
    const st = this.fs.lstatSync(dir);
    if (!st.isDirectory() || st.isSymbolicLink() || (typeof process.getuid === "function" && st.uid !== process.getuid())) throw new Error("Unsafe provider directory");
    this.fs.chmodSync(dir, 0o700);
  }
  save() {
    const bytes = JSON.stringify({version:1, profiles:this.profiles.map(p => ({id:p.id, provider:p.provider, label:p.label, epoch:p.epoch, fingerprint:p.fingerprint, store:p.store||""}))});
    const tmp = this.index + "." + this.crypto.randomUUID() + ".tmp";
    this.fs.writeFileSync(tmp, bytes, {mode:0o600, flag:"wx"}); this.fs.renameSync(tmp, this.index); this.revision++;
  }
  profile(id) {
    const p = this.profiles.find(p => p.id === id);
    if (!p || this.loadError) throw new Error("Account profile is unavailable");
    return p;
  }
  directory(p) {
    const base = this.path.join(this.root, p.id); this.privateDir(base);
    if (!p.store) return base;
    if (!this.validId(p.store)) throw new Error("Invalid account store");
    const dir=this.path.join(base,p.store);this.privateDir(dir);return dir;
  }
  busy(id) { return (this.children.get(id)?.size ?? 0)>0; }
  child(id, child) {
    let children=this.children.get(id);if(!children){children=new Set();this.children.set(id,children);}children.add(child);
    const gone=()=>{children.delete(child);if(!children.size)this.children.delete(id);};
    child.once("exit",gone);child.once("error",gone);
  }
  removeStore(p, keepActive=true) {
    if(!this.validId(p.store) || (keepActive && this.profile(p.id).store===p.store))return;
    const dir=this.path.join(this.root,p.id,p.store);
    try {const st=this.fs.lstatSync(dir);if(st.isDirectory()&&!st.isSymbolicLink()&&(typeof process.getuid!=="function"||st.uid===process.getuid()))this.fs.rmSync(dir,{recursive:true,force:true});}catch(e){if(e.code!=="ENOENT")throw e;}
  }
  discardAttempt(a) {
    if(a.state==="done" || this.profile(a.profile).store===a.candidate.store)return;
    const child=a.child||a.rpc?.child;
    const clean=()=>{try{this.removeStore(a.candidate);}catch(_){/* private orphan remains inaccessible; status never promotes it */}};
    if(child && child.exitCode===null && child.signalCode===null){child.once("exit",clean);child.once("error",clean);}else clean();
  }
  environment(p) {
    const env = {...process.env}; for (const key of bp_account_unset) delete env[key];
    for (const key of Object.keys(env)) {
      if (/^(OPENAI_|ANTHROPIC_|CODEX_.*(?:TOKEN|AUTH|WIF)|CLAUDE_CODE_USE_|CLAUDE_CODE_(?:OAUTH|API_KEY|AUTH|PROFILE|SESSION))/.test(key)) delete env[key];
    }
    env.CLAUDE_CODE_PROVIDER_MANAGED_BY_HOST = "1";
    env.CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC = "1";
    env.DISABLE_AUTOUPDATER = "1";
    env.MCP_TOOL_TIMEOUT = "86400000";
    env.CLAUDE_CODE_ENABLE_TODO_TOOLS = "1";
    const dir = this.directory(p);
    if (p.provider === "codex") {env.CODEX_HOME = dir;env.RUST_LOG="error";}
    else env.CLAUDE_CONFIG_DIR = dir;
    return env;
  }
  codex(p, timeout) {
    return new BPAccountRpc("codex", ["-c", 'cli_auth_credentials_store="file"', "-c", "features.daemon_auto_start=false", "-c", "analytics.enabled=false", "app-server", "--listen", "stdio://"], this.environment(p), timeout);
  }
  async cli(p, args, timeout=12000) {
    const cp = require("node:child_process");
    return await new Promise((resolve, reject) => {
      let out = ""; const child = cp.spawn(p.provider, args, {env:this.environment(p), stdio:["ignore","pipe","ignore"], windowsHide:true});
      const timer = setTimeout(() => { child.kill(); reject(new Error("Provider status timed out")); }, timeout);
      child.stdout.on("data", b => {out += b.toString(); if (Buffer.byteLength(out)>131072) {child.kill(); reject(new Error("Provider response exceeded its limit"));}});
      child.on("error", () => {clearTimeout(timer); reject(new Error("Provider CLI could not start"));});
      child.on("exit", code => {clearTimeout(timer); try {resolve({code, data:(args[1]==="logout" ? {} : JSON.parse(out))});} catch (_) {reject(new Error("Provider status was not valid JSON"));}});
    });
  }
  fingerprint(p, identity) {
    // Equality only, private registry. Email is never a profile key or pool ID.
    return this.crypto.createHash("sha256").update(JSON.stringify([p.id, identity])).digest("hex");
  }
  windows(limits) {
    const buckets = limits?.rateLimitsByLimitId ?? (limits?.rateLimits ? {codex:limits.rateLimits} : {});
    const windows = [];
    for (const [key, b] of Object.entries(buckets).slice(0, 20)) {
      if (!b || typeof b !== "object") continue;
      for (const kind of ["primary","secondary"]) {
        const w=b[kind]; if (!w || !Number.isFinite(w.usedPercent) || w.usedPercent<0 || w.usedPercent>100) continue;
        windows.push({name:this.label(b.limitName || b.limitId || key)+"/"+kind,
          scope:this.label(b.limitId || key), shared:true, used_percent:w.usedPercent,
          ...(Number.isFinite(w.resetsAt) ? {resets_at:w.resetsAt}:{}),
          ...(Number.isFinite(w.windowDurationMins) ? {window_minutes:w.windowDurationMins}:{})});
      }
    }
    return windows;
  }
  async claudeModels(p) {
    const cp=require("node:child_process");
    return await new Promise((resolve)=>{
      let buffer="", settled=false;
      const child=cp.spawn("claude",["--setting-sources","user","--settings",'{"forceLoginMethod":"claudeai"}',"-p","--input-format","stream-json","--output-format","stream-json","--verbose"],{env:this.environment(p),stdio:["pipe","pipe","ignore"],windowsHide:true});
      const finish=models=>{if(settled)return;settled=true;clearTimeout(timer);child.stdin.destroy();child.stdout.destroy();child.kill();resolve(models);};
      const timer=setTimeout(()=>finish([]),12000);
      child.on("error",()=>finish([]));child.on("exit",()=>finish([]));
      child.stdout.on("data",b=>{
        buffer+=b.toString();if(Buffer.byteLength(buffer)>131072)return finish([]);
        for(let cut;(cut=buffer.indexOf("\n"))>=0;){
          const line=buffer.slice(0,cut);buffer=buffer.slice(cut+1);
          try{const r=JSON.parse(line);if(r.type==="control_response" && r.response?.request_id==="models"){
            const models=r.response?.response?.models;
            return finish((Array.isArray(models)?models:[]).slice(0,40).filter(x=>typeof x.value==="string").map(x=>({id:this.label(x.value),label:this.label(x.displayName||x.value),efforts:(x.supportedEffortLevels||[]).slice(0,12).map(x=>this.label(x))})));
          }}catch(_){}
        }
      });
      child.stdin.end(JSON.stringify({type:"control_request",request_id:"models",request:{subtype:"initialize"}})+"\n");
    });
  }
  async verify(p, newLogin=false, guard=null) {
    let identity, fingerprintIdentity, catalog=[], allowance=null;
    if (p.provider === "claude") {
      const {code,data} = await this.cli(p, ["auth","status"]);
      if (code !== 0 || data.loggedIn !== true || data.authMethod !== "claude.ai") throw new Error("Claude subscription sign-in could not be verified; Console profiles and API credentials use a different store.");
      if (typeof data.configDirectory!=="string" || this.path.resolve(data.configDirectory) !== this.directory(p)) throw new Error("Claude must report its isolated account directory (Claude Code 2.1.268 or later)");
      identity={email:this.label(data.email), plan:this.label(data.subscriptionType), auth:"claude.ai"};
      fingerprintIdentity=[identity.email, this.label(data.accountUuid || data.userId), this.label(data.orgUuid || data.organizationId)];
      catalog=await this.claudeModels(p);
    } else {
      const rpc=this.codex(p);
      try {
        await rpc.initialize(); const a=await rpc.request("account/read", {refreshToken:false});
        if (a?.account?.type !== "chatgpt") throw new Error("Codex ChatGPT sign-in could not be verified");
        identity={email:this.label(a.account.email), plan:this.label(a.account.planType), auth:"chatgpt"};
        const fresh=!newLogin && p.allowance?.at && Date.now()/1000-p.allowance.at<300;
        let limits; if(!fresh)try {limits=await rpc.request("account/rateLimits/read");} catch (_) { /* honest unknown */ }
        fingerprintIdentity=[identity.email, this.label(a.workspaceRouting?.chatgptAccountId)];
        catalog=fresh?p.catalog:[];
        if(!fresh)try {
          const m=await rpc.request("model/list", {limit:100, includeHidden:false});
          catalog=(m?.data ?? []).slice(0,40).filter(x=>typeof x.model==="string").map(x=>({id:this.label(x.model), label:this.label(x.displayName || x.model), efforts:(x.supportedReasoningEfforts??[]).slice(0,12).map(y=>this.label(y.reasoningEffort))}));
        } catch (_) { /* catalog unavailable, never copy another profile */ }
        const windows=this.windows(limits); const now=Math.floor(Date.now()/1000);
        allowance=fresh?p.allowance:{provider:"codex", state:windows.length?"ok":"unavailable", at:now, attempted_at:now, auth:"chatgpt", source:"codex-app-server", windows};
      } finally {rpc.close();}
    }
    if (guard && this.jobs.get(p.id)!==guard) throw new Error("Account request was replaced");
    if (!fingerprintIdentity.some(Boolean)) throw new Error("Provider did not report a verifiable account identity");
    const digest=this.fingerprint(p,fingerprintIdentity);
    if (newLogin || (p.fingerprint && p.fingerprint !== digest)) {
      p.epoch++;if(!newLogin){catalog=[];allowance=null;}
    }
    p.fingerprint=digest; p.identity=identity; p.catalog=catalog; p.allowance=allowance;
    p.checkedAt=Math.floor(Date.now()/1000); p.state="connected"; p.message=""; this.save(); return p;
  }
  observe(profile, epoch, raw) {
    const p=this.profile(profile);
    if(p.provider!=="claude" || String(p.epoch)!==epoch || p.state!=="connected")return this.snapshot();
    const data=JSON.parse(raw),windows=[];
    for(const key of ["five_hour","seven_day","seven_day_opus","seven_day_sonnet"]){
      const w=data?.[key];if(!w || typeof w!=="object")continue;
      const used=w.utilization;
      windows.push({name:key+" (provider pool)",scope:key,shared:true,
        ...(typeof used==="number" && Number.isFinite(used) && used>=0 && used<=1?{used_percent:used*100}:{}),
        ...(typeof w.resetsAt==="number" && Number.isFinite(w.resetsAt)?{resets_at:w.resetsAt}:{})});
    }
    if(!windows.length)return this.snapshot();
    const now=Math.floor(Date.now()/1000);
    if(p.allowance && now-p.allowance.at<60 && JSON.stringify(windows)===JSON.stringify(p.allowance.windows))return this.snapshot();
    p.allowance={provider:"claude",state:"ok",at:now,source:"claude-stream",auth:"claude.ai",windows};this.revision++;
    return this.snapshot();
  }
  publicProfile(p) {
    return {id:p.id, provider:p.provider, label:p.label, epoch:p.epoch, state:p.state,
      identity:p.identity, checkedAt:p.checkedAt, catalog:p.catalog, allowance:p.allowance ? {...p.allowance,state:Date.now()/1000-p.allowance.at>300?"stale":p.allowance.state}:null,
      message:p.message || ""};
  }
  snapshot() {
    return {version:1, revision:this.revision, error:this.loadError || "", profiles:this.profiles.map(p=>this.publicProfile(p)),
      attempts:[...this.attempts.values()].map(a=>({id:a.id, profile:a.profile, state:a.state, url:a.url||"", userCode:a.userCode||"", expiresAt:a.expiresAt, message:a.message||""}))};
  }
  url(value, provider) {
    try {
      const u=new URL(value); const hosts=provider==="codex" ? ["auth.openai.com","chatgpt.com"] : ["claude.ai","platform.claude.com","console.anthropic.com"];
      if (u.protocol!=="https:" || u.username || u.password || !hosts.includes(u.hostname) || u.searchParams.has("access_token") || u.searchParams.has("refresh_token") || u.searchParams.has("code")) return "";
      return u.href.length<=8192 ? u.href : "";
    } catch (_) {return "";}
  }
  async start(p, mode="device") {
    for (const [id,a] of this.attempts) if (this.jobs.get(a.profile)!==a) this.attempts.delete(id);
    if (this.attempts.size>=8) throw new Error("Finish an existing sign-in first");
    if (this.jobs.has(p.id) || this.launchQueues.has(p.id) || this.busy(p.id)) throw new Error("This account already has a request in progress");
    const id=this.crypto.randomUUID(); const candidate={...p,store:id};
    const a={id, profile:p.id, candidate, previousState:p.state, state:"starting", expiresAt:Date.now()+600000, url:"", userCode:""};
    this.attempts.set(id,a); this.jobs.set(p.id,a); p.state="connecting"; this.revision++;
    a.timer=setTimeout(()=>this.cancel(a,"expired"),600000);
    const finish=async success=>{
      if (this.jobs.get(p.id)!==a || a.finishing) return;
      if (Date.now()>=a.expiresAt) return this.cancel(a,"expired");
      a.finishing=true;
      if (!success) return this.cancel(a,"failed");
      try {await this.verify(candidate,true,a); if (this.jobs.get(p.id)!==a) return;
        const old={...p};Object.assign(p,candidate);this.save();a.state="done"; a.url=""; a.userCode=""; this.jobs.delete(p.id); clearTimeout(a.timer); this.revision++;this.removeStore(old);}
      catch (_) {this.cancel(a,"failed");}
      finally {a.rpc?.close(); a.child?.kill();this.discardAttempt(a);}
    };
    if (p.provider==="codex") {
      const rpc=this.codex(candidate,610000); a.rpc=rpc;const early=[];
      rpc.handlers.add(msg=>{
        if (msg.method!=="account/login/completed") return;
        if(!a.loginId){if(early.length<4)early.push(msg);return;}
        if(msg.params?.loginId===a.loginId)finish(msg.params?.success===true);
      });
      try {
        await rpc.initialize(); const r=await rpc.request("account/login/start", {type:mode==="browser"?"chatgpt":"chatgptDeviceCode"});
        if (this.jobs.get(p.id)!==a) {rpc.close();return a;}
        if(typeof r.loginId!=="string" || !r.loginId || r.loginId.length>256)throw new Error();
        a.loginId=r.loginId; a.url=this.url(r.verificationUrl || r.authUrl,p.provider); a.userCode=this.label(r.userCode);
        if (!a.url) throw new Error(); a.state="waiting"; this.revision++;
        for(const msg of early)if(msg.params?.loginId===a.loginId)finish(msg.params?.success===true);
      } catch (_) {this.cancel(a,"failed");}
    } else {
      const cp=require("node:child_process"); const env=this.environment(candidate);
      // CLI prints its own authorization URL; suppress automatic browser launch.
      env.BROWSER=process.platform==="win32" ? "cmd /c exit 0" : "/bin/true";
      const child=cp.spawn("claude", ["auth","login","--claudeai"], {env,stdio:["pipe","pipe","pipe"],windowsHide:true}); a.child=child;
      let output="";
      const read=b=>{
        output += b.toString("utf8"); if (Buffer.byteLength(output)>131072) return this.cancel(a,"failed");
        for (const candidate of output.match(/https:\/\/[^\s\u001b<>]+/g) ?? []) {
          const url=this.url(candidate,p.provider);
          if (url && this.jobs.get(p.id)===a) {a.url=url;a.state="waiting";this.revision++;break;}
        }
        // No terminal transcript is retained or sent to a client.
      };
      child.stdout.on("data",read); child.stderr.on("data",read);
      child.on("error",()=>this.cancel(a,"failed")); child.on("exit",code=>finish(code===0));
    }
    return a;
  }
  cancel(a, state="cancelled") {
    if (this.jobs.get(a.profile)!==a) return;
    this.jobs.delete(a.profile); a.state=state;a.url="";a.userCode="";a.message=state==="failed"?"Sign-in could not be verified. Retry or check the provider CLI installation.":"";
    clearTimeout(a.timer);
    if (a.rpc && a.loginId) a.rpc.request("account/login/cancel",{loginId:a.loginId}).catch(()=>{}).finally(()=>a.rpc.close()); else a.rpc?.close();
    a.child?.stdin.destroy(); a.child?.kill(); const p=this.profile(a.profile);p.state=a.previousState;this.revision++;
    if(!a.finishing)this.discardAttempt(a);
  }
  async request(r) {
    if (!r || typeof r!=="object" || typeof r.op!=="string") throw new Error("Invalid account request");
    if (this.loadError) throw new Error(this.loadError);
    if (r.op==="list") return this.snapshot();
    if (r.op==="add") {
      if (!["claude","codex"].includes(r.provider) || !this.label(r.label) || this.profiles.length>=12) throw new Error("Choose a provider and account label");
      const p={id:this.crypto.randomUUID(),provider:r.provider,label:this.label(r.label),epoch:1,state:"disconnected",identity:{},fingerprint:"",catalog:[],allowance:null,checkedAt:0};
      this.directory(p); this.profiles.push(p); this.save(); return this.snapshot();
    }
    const p=this.profile(r.profile);
    if (r.op==="cancel") {const a=this.attempts.get(r.attempt);if (!a || a.profile!==p.id) throw new Error("Sign-in attempt no longer belongs to this account");this.cancel(a);return this.snapshot();}
    if (r.op==="respond") {
      const a=this.attempts.get(r.attempt);
      if (!a || this.jobs.get(p.id)!==a || a.state!=="waiting" || Date.now()>=a.expiresAt || p.provider!=="claude" || typeof r.code!=="string" || r.code.length>4096 || /[\r\n\0]/.test(r.code)) throw new Error("Sign-in response does not belong to an active attempt");
      // Claude validates its own OAuth state/code. No callback is fetched by Backplane.
      a.child.stdin.write(r.code.trim()+"\n"); return this.snapshot();
    }
    if (r.op==="login") {await this.start(p,r.mode);return this.snapshot();}
    if (this.jobs.has(p.id) || this.launchQueues.has(p.id)) throw new Error("Finish or cancel this account's current request first");
    this.jobs.set(p.id,{status:true});
    try {
      if (r.op==="status") {
        try {await this.verify(p);} catch (_) {p.state="disconnected";p.identity={};p.catalog=[];p.allowance=null;p.message="Account status could not be verified.";this.revision++;}
      } else if (r.op==="disconnect") {
        if(this.busy(p.id))throw new Error("Finish this account's active provider requests before disconnecting");
        if (p.provider==="claude") {const result=await this.cli(p,["auth","logout"]);if(result.code!==0) throw new Error("Provider logout did not complete");}
        else {const rpc=this.codex(p);try {await rpc.initialize();await rpc.request("account/logout");} finally {rpc.close();}}
        p.epoch++;p.state="disconnected";p.fingerprint="";p.identity={};p.catalog=[];p.allowance=null;this.save();this.removeStore(p,false);
      } else if (r.op==="rename") {if (!this.label(r.label)) throw new Error("Choose an account label");p.label=this.label(r.label);this.save();}
      else throw new Error("Unknown account operation");
    } finally {this.jobs.delete(p.id);}
    return this.snapshot();
  }
  async launch(profile, epoch, cmd, args, cwd, err, deadline=0) {
    this.profile(profile);
    const prior=this.launchQueues.get(profile)||Promise.resolve();
    const pending=prior.catch(()=>{}).then(()=>this.launchOne(profile,epoch,cmd,args,cwd,err,deadline));
    this.launchQueues.set(profile,pending);
    try{return await pending;}finally{if(this.launchQueues.get(profile)===pending)this.launchQueues.delete(profile);}
  }
  async launchOne(profile, epoch, cmd, args, cwd, err, deadline=0) {
    const p=this.profile(profile);
    if (!["connected","unchecked"].includes(p.state) || this.jobs.has(p.id) || String(p.epoch)!==epoch || cmd!==p.provider) throw new Error("Reconnect and select this account before starting a turn");
    const lease={launch:true}; this.jobs.set(p.id,lease);
    try {
      await this.verify(p,false,lease);
      if (String(p.epoch)!==epoch || p.state!=="connected") throw new Error("Account identity changed; select this account again");
      const env=this.environment(p);
      const launchArgs=p.provider==="codex" ? ["-c",'cli_auth_credentials_store="file"',"-c",'model_provider="openai"',...args] : ["--setting-sources","user","--settings",'{"forceLoginMethod":"claudeai"}',...args];
      return globalThis.BP_ACCOUNT_SPAWN(cmd,launchArgs,cwd,err,env,deadline,child=>this.child(p.id,child));
    } finally {if(this.jobs.get(p.id)===lease)this.jobs.delete(p.id);}
  }
}

function bp_accounts_manager(home) {
  let m=bp_account_managers.get(home); if (!m) {m=new BPAccountsIO(home);bp_account_managers.set(home,m);} return m;
}
function accountsfx_request(home, text, k) {
  const p=Promise.resolve().then(()=>bp_accounts_manager(home).request(JSON.parse(text))).then(x=>JSON.stringify(x));
  return io_park_promise(p,k,(v,e)=>e===undefined?v:JSON.stringify({...bp_accounts_manager(home).snapshot(),error:"Account operation failed. Check its current status before retrying."}));
}
function accountsfx_snapshot(home) {
  try {return JSON.stringify(bp_accounts_manager(home).snapshot());} catch (_) {return '{"error":"Account registry unavailable","profiles":[],"attempts":[]}';}
}
function accountsfx_observe(home, profile, epoch, reading) {
  try{return JSON.stringify(bp_accounts_manager(home).observe(profile,epoch,reading));}catch(_){return accountsfx_snapshot(home);}
}
function accountsfx_spawn(home, profile, epoch, cmd, args, cwd, err, k) {
  const a=[];for (;args.$==="Con";args=args.tail) a.push(args.head);
  const p=Promise.resolve().then(()=>bp_accounts_manager(home).launch(profile,epoch,cmd,a,cwd,Number(err)));
  return io_park_promise(p,k,(v,e)=>e===undefined?v:io_fail(13,"Account identity or login could not be verified; reconnect and select the account."));
}
function accountsfx_spawn_text(home, profile, epoch, cmd, args, cwd, err, k) {
  const a=[];for(;args.$==="Con";args=args.tail)a.push(args.head);
  const p=Promise.resolve().then(()=>bp_accounts_manager(home).launch(profile,epoch,cmd,a,cwd,Number(err),60000));
  return io_park_promise(p,k,(v,e)=>e===undefined?v:io_fail(13,"Account identity or login could not be verified."));
}
io_eff(CID(AccountsFx.request),accountsfx_request);
io_eff(CID(AccountsFx.snapshot),accountsfx_snapshot);
io_eff(CID(AccountsFx.spawn),accountsfx_spawn);

io_eff(CID(AccountsFx.observe),accountsfx_observe);

io_eff(CID(AccountsFx.spawn_text),accountsfx_spawn_text);
