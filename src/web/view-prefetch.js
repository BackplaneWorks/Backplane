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
const QUEUE=24, ATTEMPTS=64, PACE=300, REFRESH=30000;
export class ViewPrefetch {
  constructor(api) {this.api=api;this.queue=[];this.attempted=new Set();this.generation=0;this.fetchedAt=new Map();this.later=new Map();}
  // fresh: the picture held is known to be behind (its sidebar row moved
  // since): fetched again even though one is held
  enqueue(target, priority=false, fresh=false) {
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
    if (priority) this.queue.unshift({target,key,fresh});else this.queue.push({target,key,fresh});
    this.queue=this.queue.slice(0,QUEUE);
    if (!this.timer) this.timer=setTimeout(()=>{this.timer=null;this.pump();},priority?75:750);
  }
  pump() {
    if (this.active || !this.queue.length) return;
    // (not idle: tried again later, never dropped)
    if (!this.api.idle()) { if (!this.timer) this.timer=setTimeout(()=>{this.timer=null;this.pump();},PACE*3);return; }
    while(this.queue.length && !this.queue[0].fresh && this.api.has(this.queue[0].target))this.queue.shift();
    if(!this.queue.length)return;
    if(!this.socket){this.connect();return;}
    if(!this.greeted)return;
    const job=this.queue.shift(),scope=this.api.scope();
    if(this.attempted.size>=ATTEMPTS)return;
    this.attempted.add(job.key);this.fetchedAt.set(job.key,(this.api.now||performance.now.bind(performance))());
    this.active={...job,scope};
    const target=job.target,session={sel:target.kind==='project'?'project-home-'+target.id:target.kind==='thread'?target.id:'',rid:1,drafts:{'@lang':scope.language},open:{},info:{},adding:false,panel:false,wantNew:false,
      ...(target.kind==='bot'?{route:{kind:'bot-tab',bot:target.id,tab:'chat'}}:{})};
    const result=this.client.select(this.api.hub,target,session,'preview');
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
        if(reply.remoteReady && reply.shown && !reply.result.cmds.length && current.identity===job.scope.identity && current.epoch===job.scope.epoch
          && current.origin===job.scope.origin && (job.fresh || !this.api.has(job.target)))this.api.picture(reply.shown,reply.session,job.target,job.scope);
        if(this.queue.length)this.timer=setTimeout(()=>{this.timer=null;this.pump();},PACE);
      }catch{this.stop();}
    };
    socket.onclose=socket.onerror=()=>{if(this.generation===generation)this.stop();};
  }
  stop() {
    for (const t of this.later.values()) clearTimeout(t);this.later.clear();
    this.generation++;clearTimeout(this.timer);clearTimeout(this.deadline);this.timer=null;this.active=null;this.queue=[];this.greeted=false;
    const socket=this.socket;this.socket=null;this.client=null;socket?.close();
  }
}
