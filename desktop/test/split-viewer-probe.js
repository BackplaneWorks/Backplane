// Private-fixture instrumentation only. Observes dimensions/identities/timing;
// never reads or changes drafts/message text, selection, scroll or viewer mode.
(() => {
 const native=window.ResizeObserver,ids=new WeakMap();let next=0;
 const id=node=>{if(!node)return 0;if(!ids.has(node))ids.set(node,++next);return ids.get(node);};
 const cap=2500,events=[],counts={resize:0,viewerResize:0,timelineResize:0,canvasReplaced:0};
 const push=(kind,data={})=>{if(events.length>=cap)events.shift();events.push({at:performance.now(),kind,...data});};
 const rect=n=>{if(!n)return null;const r=n.getBoundingClientRect();return {id:id(n),x:r.x,y:r.y,w:r.width,h:r.height};};
 const sample=()=>{
  const tl=document.querySelector('#timeline'),canvas=document.querySelector('#solid'),track=tl?.querySelector('.timeline-virtual-track'),rows=[...(tl?.querySelectorAll('.timeline-virtual-row')||[])];
  return {timeline:tl?{...rect(tl),top:tl.scrollTop,total:tl.scrollHeight,view:tl.clientHeight,track:rect(track),rows:rows.length,first:rect(rows[0]),last:rect(rows.at(-1))}:null,viewer:rect(document.querySelector('#viewer')),canvas:rect(canvas),buffer:canvas?{w:canvas.width,h:canvas.height}:null,mode:document.documentElement.dataset.backplaneMode,ready:document.documentElement.dataset.backplaneRemoteReady,held:document.documentElement.dataset.backplaneHeld};
 };
 if(native)window.ResizeObserver=class extends native{constructor(callback){super((entries,observer)=>{counts.resize++;for(const entry of entries){const node=entry.target,isViewer=!!node.closest?.('#viewer'),isTimeline=node.id==='timeline'||!!node.closest?.('#timeline');if(isViewer)counts.viewerResize++;if(isTimeline)counts.timelineResize++;if(isViewer||isTimeline)push('resize',{target:id(node),viewer:isViewer,timeline:isTimeline,w:entry.contentRect.width,h:entry.contentRect.height});}callback(entries,observer);});}};
 document.addEventListener('scroll',event=>{if(event.target.id==='timeline')push('scroll',sample());},true);
 let before=0;
 new MutationObserver(()=>{const canvas=document.querySelector('#solid'),current=id(canvas);if(current!==before){if(before&&current)counts.canvasReplaced++;before=current;push('canvas-lifetime',{canvas:current,...sample()});}}).observe(document,{subtree:true,childList:true});
 document.addEventListener('click',event=>{const node=event.target.closest?.('[data-on-click]');if(node&&['view-kind','panel'].includes(node.dataset.onClick))push('viewer-action',{action:node.dataset.onClick,kind:node.dataset.value,...sample()});},true);
 window.__viewerScrollProbe={events,counts,sample};
})();
