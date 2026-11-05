import {ThinViews, viewTarget} from './thin.js';
import {validateThinResult} from './thin-results.js';
// One serial, read-only transport for a few likely destinations. It never
// shares the foreground actor, sends an action, follows history, executes a
// command, acknowledges unread content, or installs a returned session.
// Every destination the sidebar shows is fetched once (a click on any of them
// then draws at once, with no round trip to wait for; the hub's own view
// corrects it), one at a time and only while the page is idle, PACE apart.
// A picture fetched again because its row moved waits REFRESH since its
// last fetch (a streaming bot's row moves with every delta: it was fetched
// again every ~340 ms, ~100 KB each, for as long as it streamed); a timer
// asks once the floor has passed, so the last move is still fetched.
const QUEUE=24, ATTEMPTS=64, PACE=300, REFRESH=30000, VIEWERS=96;
export class ViewPrefetch {
  constructor(api) {this.api=api;this.queue=[];this.attempted=new Set();this.viewersDone=new Set();this.generation=0;this.fetchedAt=new Map();this.later=new Map();}
  // fresh: the picture held is known to be behind (its sidebar row moved
  // since): fetched again even though one is held
  // soon: asked for now, not after two seconds without input (the palette's
  // chosen thread: the next press is likely its Enter)
  enqueue(target, priority=false, fresh=false, soon=false) {
    if (!viewTarget(target) || !['thread','bot','project'].includes(target.kind) || !fresh && this.api.has(target)) return;
    const scope=this.api.scope(),key=JSON.stringify([scope.origin,scope.identity,scope.epoch,target]);
    if (!scope.identity || !scope.epoch) return;
    if (fresh) {
      const now=(this.api.now||performance.now.bind(performance))(),last=this.fetchedAt.get(key);
      if (last!==undefined && now-last<REFRESH) {
        if (!this.later.has(key)) this.later.set(key,setTimeout(()=>{this.later.delete(key);this.enqueue(target,false,true);},REFRESH-(now-last)));
        return;
      }
      this.attempted.delete(key);
    }
    else if (this.attempted.has(key)) return;
    this.queue=this.queue.filter(x=>x.key!==key);
    if (priority) this.queue.unshift({target,key,fresh,soon});else this.queue.push({target,key,fresh,soon});
    this.queue=this.queue.slice(0,QUEUE);
    if (!this.timer) this.timer=setTimeout(()=>{this.timer=null;this.pump();},priority?75:750);
  }
  // A thread's view with its shut runs of tool calls open (ids: their first
  // entries): what each run holds is learned (fold-feedback.js), so a run's
  // first open draws at the click too. Never a picture; once per thread and
  // page load; only while the page is idle, like the rest.
  // A thread's viewers, each as the hub draws it open (opens: {action, value,
  // kind}: the rail tab's press and the view.kind it shows): learned at the
  // press layer (press-feedback.js's ahead), so a viewer's first open draws
  // at the click too. Never a picture; once per thread and viewer per page
  // load, VIEWERS at most; only while the page is idle, like the rest.
  enqueueViewers(target, opens) {
    if (!viewTarget(target) || target.kind !== 'thread' || !opens.length) return;
    const scope=this.api.scope();
    if (!scope.identity || !scope.epoch) return;
    // (first in the queue: the thread on show's, ahead of the sidebar's)
    const jobs=[];
    for (const open of opens) {
      const key=JSON.stringify([scope.origin,scope.identity,scope.epoch,target,'viewer',open.kind]);
      if (this.viewersDone.has(key) || this.viewersDone.size+jobs.length>=VIEWERS || this.queue.some(x=>x.key===key)) continue;
      jobs.push({target,key,viewer:open});
    }
    this.queue=[...jobs,...this.queue];
    this.queue=this.queue.slice(0,QUEUE);
    if (!this.timer) this.timer=setTimeout(()=>{this.timer=null;this.pump();},750);
  }
  enqueueFolds(target, ids) {
    if (!viewTarget(target) || target.kind !== 'thread' || !ids.length) return;
    const scope=this.api.scope(),key=JSON.stringify([scope.origin,scope.identity,scope.epoch,target,'folds']);
    if (!scope.identity || !scope.epoch || this.attempted.has(key) || this.queue.some(x=>x.key===key)) return;
    this.queue.push({target,key,folds:ids.slice()});
    this.queue=this.queue.slice(0,QUEUE);
    if (!this.timer) this.timer=setTimeout(()=>{this.timer=null;this.pump();},750);
  }
  pump() {
    if (this.active || !this.queue.length) return;
    // (not idle: tried again later, never dropped)
    if (!this.api.idle(this.queue[0].soon)) { if (!this.timer) this.timer=setTimeout(()=>{this.timer=null;this.pump();},this.queue[0].soon?PACE/4:PACE*3);return; }
    while(this.queue.length && !this.queue[0].fresh && !this.queue[0].folds && !this.queue[0].viewer && this.api.has(this.queue[0].target))this.queue.shift();
    if(!this.queue.length)return;
    // (a viewer's render on a connection of its own: the hub keeps one view
    // per thread and connection, whose viewer the first render chose)
    if(this.socket && this.used && this.queue[0].viewer)this.drop();
    if(!this.socket){this.connect();return;}
    if(!this.greeted)return;
    const job=this.queue.shift(),scope=this.api.scope();
    if(!job.viewer && this.attempted.size>=ATTEMPTS)return;
    (job.viewer?this.viewersDone:this.attempted).add(job.key);this.fetchedAt.set(job.key,(this.api.now||performance.now.bind(performance))());
    this.active={...job,scope};this.used=true;
    const target=job.target,session={sel:target.kind==='project'?'project-home-'+target.id:target.kind==='thread'?target.id:'',rid:1,drafts:{'@lang':scope.language},open:{},info:job.folds?Object.fromEntries(job.folds.map(id=>['fold:'+id,'1'])):job.viewer?{['panel:'+target.id]:'1',['view.kind:'+target.id]:job.viewer.kind,['view.path:'+target.id]:''}:{},adding:false,panel:!!job.viewer,wantNew:false,
      ...(target.kind==='bot'?{route:{kind:'bot-tab',bot:target.id,tab:'chat'}}:{})};
    // (cid "ahead-<kind>": unlike a "preview", a viewer's render is given the
    // thread's kept viewer reads: its rows, refs and diff)
    const result=this.client.select(this.api.hub,target,session,job.viewer?'ahead-'+job.viewer.kind:'preview');
    if(result.failed){this.active=null;return;}
    this.deadline=setTimeout(()=>this.stop(),10000);
  }
  connect() {
    const generation=this.generation,scope=this.api.scope(),socket=this.api.socket();this.socket=socket;
    socket.binaryType='arraybuffer';
    this.client=new ThinViews({now:()=>performance.now(),validateResult:validateThinResult,
      send:(_hub,request)=>{socket.send(this.api.encode({...request,p:{...request.p,workspaceLayout:'rail-v1',workspaceChrome:'top-v1'}}));return null;},command:()=>null});
    this.deadline=setTimeout(()=>this.stop(),10000);
    socket.onmessage=event=>{
      if(this.generation!==generation)return;
      try{
        const frame=this.api.decode(event.data);
        if(frame.t==='view'){
          if(frame.epoch!==scope.epoch || this.client.connect(this.api.hub,frame).connected!==true){this.stop();return;}
          clearTimeout(this.deadline);this.greeted=true;this.pump();return;
        }
        if(frame.t!=='reply' || !this.active)return;
        const reply=this.client.reply(this.api.hub,frame),job=this.active;
        if(!reply)return;
        clearTimeout(this.deadline);this.active=null;
        const current=this.api.scope();
        // (a viewer's render asks its reads again: those asks are not sent)
        const same=reply.remoteReady && reply.shown && (job.viewer || !reply.result.cmds.length) && current.identity===job.scope.identity && current.epoch===job.scope.epoch && current.origin===job.scope.origin;
        if(same && job.viewer)this.api.viewer?.(reply.shown,job.target,job.viewer);
        else if(same && job.folds)this.api.folds?.(reply.shown,job.target,reply.window);
        else if(same && (job.fresh || !this.api.has(job.target)))this.api.picture(reply.shown,reply.session,job.target,job.scope);
        if(this.queue.length)this.timer=setTimeout(()=>{this.timer=null;this.pump();},PACE);
      }catch{this.stop();}
    };
    socket.onclose=socket.onerror=()=>{if(this.generation===generation)this.stop();};
  }
  // (the connection only: what is queued stays, asked on the next one)
  drop() {
    this.generation++;clearTimeout(this.deadline);this.greeted=false;this.used=false;
    const socket=this.socket;this.socket=null;this.client=null;socket?.close();
  }
  stop() {
    for (const t of this.later.values()) clearTimeout(t);this.later.clear();
    this.generation++;clearTimeout(this.timer);clearTimeout(this.deadline);this.timer=null;this.active=null;this.queue=[];this.greeted=false;this.used=false;
    const socket=this.socket;this.socket=null;this.client=null;socket?.close();
  }
}
