// Saved-CAD component identity and previews. This module never guesses a
// component from an anonymous 3D triangle or its colour. All dimensions are mm.
const NS = 'http://www.w3.org/2000/svg';
const finite = a => Array.isArray(a) && a.every(Number.isFinite);
const supported = new Set(['rect', 'roundrect', 'circle', 'oval']);
const rotate = ([x,y], degrees) => { const a=degrees*Math.PI/180,c=Math.cos(a),s=Math.sin(a);return [c*x-s*y,s*x+c*y]; };

export class ComponentStore {
  constructor(key='', data=null) { this.listeners=new Set();this.key='';this.data=null;this.selected='';this.setData(key,data); }
  setData(key,data) {
    const same=this.key===key && this.data?.source===data?.source && this.data?.fingerprint===data?.fingerprint && this.data?.sourceRevision===data?.sourceRevision && this.data?.schematicRevision===data?.schematicRevision;
    const ids=new Set();
    this.error=data?.error || '';
    if(data && (!Array.isArray(data.components)||data.schema!==1)) this.error ||= 'Unsupported component metadata.';
    for(const c of data?.components||[]) {
      if(!c.id||ids.has(c.id)) this.error ||= 'Component UUIDs are missing or ambiguous; selection unavailable.';
      ids.add(c.id);
    }
    // A repeated paged read creates a fresh object even for the exact same
    // saved data. Keep the existing renderer binding in that case: replacing
    // it unnecessarily rebuilds illustrative solder and cancels tracing.
    // Compare the complete response, not just its fingerprint/revision; a
    // changed model, pad, symbol or qualification field must still propagate.
    const snapshot=this.error||!data?'':JSON.stringify(data);
    // Consumers can hold mutable objects. An old cached snapshot alone does
    // not prove that the currently retained object is still the same data.
    const unchanged=same&&this.snapshot===snapshot&&JSON.stringify(this.data)===snapshot;
    this.key=key;this.data=this.error?null:unchanged?this.data:data;this.snapshot=snapshot;
    // A saved revision change invalidates the selection, even if UUIDs survived.
    if(!same || !this.data?.components.some(c=>c.id===this.selected)) this.selected='';
    this.emit();
  }
  select(id='') { const next=this.data?.components.some(c=>c.id===id)?id:'';if(next!==this.selected){this.selected=next;this.emit();}return this.current(); }
  current() { return this.data?.components.find(c=>c.id===this.selected)||null; }
  subscribe(fn) { this.listeners.add(fn);return ()=>this.listeners.delete(fn); }
  emit() { for(const fn of this.listeners) fn(this.current(),this); }
}

// Pad centres have already been transformed by Board.Geo.place on the hub.
// The saved pad orientation follows the board renderer's pad convention.
export function padOutline(component,pad,board=true) {
  if(!supported.has(pad.shape)||!finite(pad.size)||pad.size.length!==2||pad.size.some(n=>n<=0))return [];
  const [w,h]=pad.size, x=w/2,y=h/2;
  let points=[];
  if(pad.shape==='circle') for(let i=0;i<40;i++){const a=i*Math.PI/20;points.push([Math.cos(a)*x,Math.sin(a)*x]);}
  else {
    const r=pad.shape==='oval'?Math.min(x,y):pad.shape==='roundrect'?Math.max(0,Math.min(Math.min(w,h)*Number(pad.roundrectRatio||0),x,y)):0;
    if(r===0) points=[[-x,-y],[x,-y],[x,y],[-x,y]];
    else for(const [cx,cy,start] of [[x-r,y-r,0],[-x+r,y-r,90],[-x+r,-y+r,180],[x-r,-y+r,270]])
      for(let i=0;i<=10;i++){const a=(start+i*9)*Math.PI/180;points.push([cx+r*Math.cos(a),cy+r*Math.sin(a)]);}
  }
  const center=board?pad.board:pad.local;
  if(!finite(center)||center.length!==2)return [];
  const angle=Number(pad.angle||0)+(board?0:Number(component.at?.[2]||0));
  return points.map(p=>{const r=rotate(p,angle);return[r[0]+center[0],r[1]+center[1]];});
}

export function hitComponent(data,x,y,{off=0,side=null}={}) {
  if(!Number.isFinite(x)||!Number.isFinite(y))return null;
  let best=null,area=Infinity;
  for(const c of data?.components||[]) {
    if(side && c.layer!==side)continue;
    // Layer ordering/hidden-state are explicit. A B.Cu pad cannot intercept a
    // tap while its side is hidden. Through-hole pads remain on either copper.
    const layer=c.layer==='B.Cu'?5:12;
    for(const p of c.pads||[]) {
      if(p.type==='np_thru_hole'||(!String(p.type).includes('thru_hole')&&Math.floor(off/2**layer)%2))continue;
      if(!supported.has(p.shape)||!finite(p.board)||!finite(p.size)||p.size.some(n=>n<=0))continue;
      const [dx,dy]=rotate([x-p.board[0],y-p.board[1]],-Number(p.angle||0));
      const [w,h]=p.size,ax=Math.abs(dx),ay=Math.abs(dy),hx=w/2,hy=h/2;
      if(ax>hx||ay>hy)continue;
      let inside=true;
      if(p.shape==='circle')inside=dx*dx+dy*dy<=hx*hx;
      else if(p.shape==='oval'||p.shape==='roundrect') {
        const r=p.shape==='oval'?Math.min(hx,hy):Math.max(0,Math.min(Math.min(w,h)*Number(p.roundrectRatio||0),hx,hy));
        if(r>0){const a=Math.max(0,ax-(hx-r)),b=Math.max(0,ay-(hy-r));inside=a*a+b*b<=r*r;}
      }
      if(inside){if(best&&best.id!==c.id)return null;if(w*h<area){best=c;area=w*h;}}
    }
  }
  return best;
}

const el=(tag,text,cls)=>{const n=document.createElement(tag);if(text!=null)n.textContent=text;if(cls)n.className=cls;return n;};
const svgEl=(name,attrs={})=>{const n=document.createElementNS(NS,name);for(const [k,v] of Object.entries(attrs))n.setAttribute(k,String(v));return n;};
const bounds=points=>points.reduce(([x0,y0,x1,y1],[x,y])=>[Math.min(x0,x),Math.min(y0,y),Math.max(x1,x),Math.max(y1,y)],[Infinity,Infinity,-Infinity,-Infinity]);
function geometry(svg,items,points) {
  for(const it of items||[]) {
    const s=it.shape||{}, w=Math.max(.03,Number(s.width)||0),attrs={stroke:'currentColor','stroke-width':w,fill:s.fill?'currentColor':'none','stroke-linejoin':'round','stroke-linecap':'round'};
    if(s.kind==='circle'&&finite(s.center)&&Number.isFinite(s.radius)) {
      svg.append(svgEl('circle',{...attrs,cx:s.center[0],cy:s.center[1],r:s.radius}));points.push([s.center[0]-s.radius,s.center[1]-s.radius],[s.center[0]+s.radius,s.center[1]+s.radius]);
    } else if(Array.isArray(s.points)&&s.points.length&&s.points.every(finite)) {
      svg.append(svgEl(s.kind==='polygon'?'polygon':'polyline',{...attrs,points:s.points.map(p=>p.join(',')).join(' ')}));points.push(...s.points);
    }
  }
}
function fitSvg(svg,points) {
  const b=bounds(points);if(!b.every(Number.isFinite)){svg.setAttribute('viewBox','0 0 10 10');return;}
  const w=Math.max(.5,b[2]-b[0]),h=Math.max(.5,b[3]-b[1]),m=Math.max(w,h)*.12;
  svg.setAttribute('viewBox',`${b[0]-m} ${b[1]-m} ${w+2*m} ${h+2*m}`);
}
export function footprintPreview(c) {
  const svg=svgEl('svg',{role:'img','aria-label':`${c.reference} saved footprint, numbered pads`,width:'100%',height:180}),points=[];
  geometry(svg,c.graphics,points);
  for(const p of c.pads||[]) {
    const ps=padOutline(c,p,false);
    if(ps.length){svg.append(svgEl('polygon',{points:ps.map(p=>p.join(',')).join(' '),fill:'#537588',stroke:'#c1e0e9','stroke-width':.025}));points.push(...ps);}
    if(!finite(p.local))continue;
    const t=svgEl('text',{x:p.local[0],y:p.local[1],'text-anchor':'middle','dominant-baseline':'central',fill:'#fff','font-size':Math.max(.2,Math.min(...(p.size||[1,1]))*.5)});t.textContent=p.number;svg.append(t);points.push(p.local);
  }
  fitSvg(svg,points);return svg;
}
export function symbolPreview(c) {
  if(!c.symbol?.items?.length)return el('p','Symbol unavailable: no unique source UUID/reference and cached library match.','component-limit');
  const svg=svgEl('svg',{role:'img','aria-label':`${c.reference} source schematic symbol`,width:'100%',height:180}),points=[];
  geometry(svg,c.symbol.items,points);fitSvg(svg,points);return svg;
}

export function mountInspector(container,store) {
  if(!container)return()=>{};
  let query='';
  const render=()=>{
    const focus=container.querySelector('input')===document.activeElement;
    container.replaceChildren();container.classList.add('component-inspector');
    if(store.error||!store.data){container.append(el('p',store.error||'Reading saved component metadata…'));return;}
    const search=el('input');search.type='search';search.placeholder='Find reference, value or MPN';search.setAttribute('aria-label','Find saved component');search.value=query;
    search.addEventListener('input',()=>{query=search.value;render();});container.append(search);
    const table=el('table',null,'component-bom'),head=el('tr');for(const label of ['Ref','Value','MPN'])head.append(el('th',label));const thead=el('thead');thead.append(head);table.append(thead);
    const body=el('tbody');
    for(const c of [...store.data.components].sort((a,b)=>a.reference.localeCompare(b.reference,undefined,{numeric:true}))) {
      if(query&&!`${c.reference} ${c.value} ${c.mpn} ${c.footprint}`.toLowerCase().includes(query.toLowerCase()))continue;
      const row=el('tr');row.dataset.component=c.id;row.setAttribute('aria-selected',String(c.id===store.selected));
      const ref=el('td'),button=el('button',c.reference||'(unannotated)');button.type='button';button.title=`Select saved footprint ${c.id}`;button.addEventListener('click',()=>store.select(c.id));ref.append(button);row.append(ref,el('td',c.value),el('td',c.mpn||'—'));body.append(row);
    }
    table.append(body);const list=el('div',null,'component-list');list.append(table);container.append(list);
    const c=store.current();
    if(c) {
      const header=el('div',null,'component-head'),clear=el('button','Clear');clear.type='button';clear.addEventListener('click',()=>store.select(''));header.append(el('strong',c.reference),clear);container.append(header);
      const dl=el('dl');for(const [k,v] of [['Value',c.value],['MPN',c.mpn||'Unavailable in saved sources'],['Footprint',c.footprint],['Side',c.layer],['Position',`${c.at[0]}, ${c.at[1]} mm · ${c.at[2]}°`],['UUID',c.id]])dl.append(el('dt',k),el('dd',v));container.append(dl);
      container.append(el('h4','Saved footprint'),footprintPreview(c),el('h4','Source symbol'),symbolPreview(c));
      const padTable=el('table'),ph=el('tr');for(const label of ['Pad','Net','Shape'])ph.append(el('th',label));padTable.append(ph);for(const p of c.pads||[]){const row=el('tr');row.append(el('td',p.number||'—'),el('td',p.net||'—'),el('td',p.shape));padTable.append(row);}container.append(padTable);
      if(c.pads.some(p=>!supported.has(p.shape)))container.append(el('p','Custom or unsupported pad geometry is listed by number and net; its shape is not approximated.','component-limit'));
      container.append(el('h4','3D source'),el('p',c.models.length?c.models.map(m=>m.path).join('\n'):'No model specified by this saved footprint.'));
      container.append(el('p',store.data.assemblyLimit,'component-limit'));
    }else container.append(el('p','Select a listed reference or a saved copper pad in PCB/3D.'));
    const source=el('p',`Saved source: ${store.data.source}`, 'component-source');source.title=`Fingerprint ${store.data.fingerprint}`;container.append(source);
    if(focus){search.focus();search.setSelectionRange(search.value.length,search.value.length);}
  };
  const unsub=store.subscribe(render);render();return()=>{unsub();container.replaceChildren();};
}

// Exact board-plane pad selection, explicitly separate from unidentified
// component model triangles. The renderer owns camera transforms and units.
export function attachSolid(canvas,store,{projectPoint,rayToBoard}) {
  if(!canvas?.parentElement||!projectPoint||!rayToBoard)return()=>{};
  const stage=canvas.parentElement,overlay=svgEl('svg',{'aria-hidden':'true',class:'component-selection-overlay'});
  Object.assign(overlay.style,{position:'absolute',inset:'0',width:'100%',height:'100%',pointerEvents:'none'});stage.append(overlay);
  let queued=false;
  const draw=()=>{
    queued=false;overlay.replaceChildren();const c=store.current();if(!c)return;
    const z=c.layer==='B.Cu'?0:store.data?.thicknessMm;if(!Number.isFinite(z)||z<=0&&c.layer!=='B.Cu')return;
    for(const p of c.pads||[]) {
      const ps=padOutline(c,p,true).map(([x,y])=>projectPoint([x,y,z]));
      if(!ps.length||ps.some(p=>!p||p.visible===false))continue;
      overlay.append(svgEl('polygon',{points:ps.map(p=>`${p.x},${p.y}`).join(' '),fill:'rgba(210,240,90,.2)',stroke:'#d2f05a','stroke-width':1.5}));
    }
    const a=projectPoint([c.at[0],c.at[1],z]);if(a?.visible){const t=svgEl('text',{x:a.x+6,y:a.y-6,fill:'#d2f05a','font-size':12,stroke:'#111','stroke-width':.3});t.textContent=`${c.reference} · footprint`;overlay.append(t);}
  };
  const schedule=()=>{if(!queued){queued=true;requestAnimationFrame(draw);}};
  let press=null;const pointers=new Set();
  const down=e=>{pointers.add(e.pointerId);press=pointers.size===1&&e.button===0?{x:e.clientX,y:e.clientY,id:e.pointerId}:null;};
  const up=e=>{
    pointers.delete(e.pointerId);if(!press||press.id!==e.pointerId)return;
    const a=press;press=null;if(pointers.size||Math.hypot(e.clientX-a.x,e.clientY-a.y)>5)return;
    const r=canvas.getBoundingClientRect(),x=e.clientX-r.left,y=e.clientY-r.top,candidates=[];
    for(const [side,z] of [['F.Cu',store.data?.thicknessMm],['B.Cu',0]]){
      if(!Number.isFinite(z)||z<=0&&side==='F.Cu')continue;
      const p=rayToBoard(x,y,z);if(!p)continue;const c=hitComponent(store.data,p[0],p[1],{side});if(!c)continue;
      const projected=projectPoint([p[0],p[1],z]);if(projected?.visible)candidates.push({c,depth:projected.depth});
    }
    candidates.sort((a,b)=>a.depth-b.depth);store.select(candidates[0]?.c.id||'');
  };
  const cancel=()=>{press=null;pointers.clear();};
  canvas.addEventListener('pointerdown',down);canvas.addEventListener('pointerup',up);canvas.addEventListener('pointercancel',cancel);canvas.addEventListener('solid-camera-change',schedule);
  const resize=typeof ResizeObserver==='function'?new ResizeObserver(schedule):null;resize?.observe(canvas);
  const unsub=store.subscribe(schedule);schedule();
  return()=>{unsub();resize?.disconnect();overlay.remove();canvas.removeEventListener('pointerdown',down);canvas.removeEventListener('pointerup',up);canvas.removeEventListener('pointercancel',cancel);canvas.removeEventListener('solid-camera-change',schedule);};
}
