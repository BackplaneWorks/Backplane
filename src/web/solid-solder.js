// Optional, disposable illustration. Source CAD, GLB geometry and materials are
// never changed. These are estimated toe fillets, not manufactured joint shapes.
import {padOutline} from './components.js';

const EPS=.003, MATCH=.025;
const dot=(a,b)=>a.reduce((s,x,i)=>s+x*b[i],0);
const sub=(a,b)=>a.map((x,i)=>x-b[i]);
const dist=(a,b)=>Math.hypot(...sub(a,b));
const key=p=>p.map(x=>Math.round(x/EPS)).join(',');
const edgeKey=(a,b)=>[key(a),key(b)].sort().join('|');
const cross=(a,b)=>[a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]];
const xy=p=>p.slice(0,2);
function segmentDistance(p,a,b) {
  const ab=sub(b,a),den=dot(ab,ab),t=den?Math.max(0,Math.min(1,dot(sub(p,a),ab)/den)):0;
  return dist(p,a.map((x,i)=>x+t*ab[i]));
}
function boundaryDistance(p,edges) {return Math.min(...edges.map(([a,b])=>segmentDistance(p,a,b)));}
function polygonEdges(ps) {return ps.map((p,i)=>[p,ps[(i+1)%ps.length]]);}
function inside(p,ps) {
  let on=false;
  for(let i=0,j=ps.length-1;i<ps.length;j=i++) {
    const a=ps[i],b=ps[j];
    if((a[1]>p[1])!==(b[1]>p[1])&&p[0]<(b[0]-a[0])*(p[1]-a[1])/(b[1]-a[1])+a[0])on=!on;
  }
  return on||boundaryDistance(p,polygonEdges(ps))<=EPS;
}
function boundary(triangles) {
  const edges=new Map();
  for(const t of triangles)for(let i=0;i<3;i++) {
    const a=t[i],b=t[(i+1)%3],k=edgeKey(a,b),prior=edges.get(k);
    if(prior)prior.count++;else edges.set(k,{a,b,count:1});
  }
  // Non-manifold or multiply layered geometry is deliberately unsupported.
  if([...edges.values()].some(e=>e.count>2))return [];
  return [...edges.values()].filter(e=>e.count===1).map(e=>[e.a,e.b]);
}
function normal(t) {
  const n=cross(sub(t[1],t[0]),sub(t[2],t[0])),l=Math.hypot(...n);
  return l>1e-10?n.map(x=>x/l):null;
}
function padFace(triangles,polygon,boardZ,side) {
  const saved=polygonEdges(polygon),planes=new Map();
  // Broad-phase culling only. Qualification below still uses every real face
  // boundary and the saved polygon, never these bounds as a joint anchor.
  const min=[0,1].map(i=>Math.min(...polygon.map(p=>p[i]))-MATCH),max=[0,1].map(i=>Math.max(...polygon.map(p=>p[i]))+MATCH);
  for(const t of triangles) {
    const z=t[0][2];
    if(t.some(p=>p[0]<min[0]||p[0]>max[0]||p[1]<min[1]||p[1]>max[1]))continue;
    if(Math.abs(z-boardZ)>.15||!t.every(p=>inside(xy(p),polygon)||boundaryDistance(xy(p),saved)<MATCH))continue;
    const k=Math.round(z/EPS);if(!planes.has(k))planes.set(k,[]);planes.get(k).push(t);
  }
  const matches=[];
  for(const ts of planes.values()) {
    const edges=boundary(ts).map(e=>e.map(xy));if(!edges.length)continue;
    // A real pad face must follow the saved outline in both directions. A
    // nearby board/mask triangle or a component bounding box cannot qualify.
    if(edges.some(e=>e.some(p=>boundaryDistance(p,saved)>MATCH)))continue;
    if(polygon.some(p=>boundaryDistance(p,edges)>MATCH))continue;
    matches.push({z:ts[0][0][2],edges,triangles:ts.length});
  }
  matches.sort((a,b)=>side*(b.z-a.z));
  return matches[0]||null;
}
function rayBoundary(p,u,edges) {
  let best=Infinity;
  for(const [a,b] of edges) {
    const v=sub(b,a),den=u[0]*v[1]-u[1]*v[0];if(Math.abs(den)<1e-8)continue;
    const q=sub(a,p),t=(q[0]*v[1]-q[1]*v[0])/den,s=(q[0]*u[1]-q[1]*u[0])/den;
    if(t>EPS&&s>=-EPS&&s<=1+EPS)best=Math.min(best,t);
  }
  return best;
}
function intervalsAtHeight(ts,z,v) {
  const intervals=[];
  for(const t of ts) {
    const ps=[];
    for(let i=0;i<3;i++) {
      const a=t[i],b=t[(i+1)%3];
      if(Math.abs(a[2]-z)<EPS)ps.push(dot(xy(a),v));
      if((a[2]<z&&b[2]>z)||(a[2]>z&&b[2]<z)) {
        const f=(z-a[2])/(b[2]-a[2]);ps.push(dot([a[0]+f*(b[0]-a[0]),a[1]+f*(b[1]-a[1])],v));
      }
    }
    if(ps.length>1)intervals.push([Math.min(...ps),Math.max(...ps)]);
  }
  intervals.sort((a,b)=>a[0]-b[0]);const merged=[];
  for(const a of intervals) {const last=merged.at(-1);if(last&&a[0]<=last[1]+EPS)last[1]=Math.max(last[1],a[1]);else merged.push([...a]);}
  return merged;
}
function terminalEdge(triangles,polygon,pad,other,face,side) {
  const du=sub(pad.board,other.board),length=Math.hypot(...du),u=du.map(x=>x/length),v=[-u[1],u[0]],planes=new Map();
  for(const t of triangles) {
    const n=normal(t);if(!n||Math.abs(n[2])>.02||Math.abs(dot(xy(n),u))<.995)continue;
    if(!t.every(p=>inside(xy(p),polygon)))continue;
    const ds=t.map(p=>dot(xy(p),u));if(Math.max(...ds)-Math.min(...ds)>EPS)continue;
    const k=`${Math.round(ds[0]/EPS)}|${t.materialKey}`;if(!planes.has(k))planes.set(k,[]);planes.get(k).push(t);
  }
  const candidates=[];
  for(const ts of planes.values()) {
    const heights=ts.flat().map(p=>side*(p[2]-face.z)),lo=Math.min(...heights),hi=Math.max(...heights);
    if(lo<-.02||lo>.15||hi-lo<.10||hi>5)continue;
    const z=face.z+side*lo,edges=boundary(ts),bottom=edges.filter(([a,b])=>Math.abs(a[2]-z)<EPS&&Math.abs(b[2]-z)<EPS&&dist(a,b)>.05);
    if(!bottom.length)continue;
    // Use actual coplanar boundary segments, not an axis-aligned component box.
    const intervals=bottom.map(e=>e.map(p=>dot(xy(p),v)).sort((a,b)=>a-b)).sort((a,b)=>a[0]-b[0]);
    const joined=[];for(const a of intervals){const last=joined.at(-1);if(last&&a[0]<=last[1]+EPS)last[1]=Math.max(last[1],a[1]);else joined.push([...a]);}
    if(joined.length!==1)continue;
    const [a,b]=joined[0],d=dot(xy(ts[0][0]),u),span=b-a;if(span<.15||span>8)continue;
    const ends=[a+.015,b-.015].map(t=>[u[0]*d+v[0]*t,u[1]*d+v[1]*t]);
    const runs=ends.map(p=>rayBoundary(p,u,face.edges));
    if(runs.some(r=>!Number.isFinite(r)||r<.08||r>2))continue;
    const h=Math.min(.25,lo+(hi-lo)*.3,Math.min(...runs)*.65);
    if(h<lo+.03)continue;
    const zAnchor=face.z+side*h,coverage=intervalsAtHeight(ts,zAnchor,v);
    if(!coverage.some(([l,r])=>l<=a+.015+EPS&&r>=b-.015-EPS))continue;
    candidates.push({ends,runs:runs.map(r=>r-.025),u,zAnchor,zBase:face.z+side*.002,side,plane:d,terminalTriangles:ts.length,padTriangles:face.triangles,bottomEdgeZ:z,materialKey:ts[0].materialKey});
  }
  candidates.sort((a,b)=>b.plane-a.plane);
  if(candidates.length>1&&Math.abs(candidates[0].plane-candidates[1].plane)<MATCH)return null;
  return candidates[0]||null;
}
function filletGeometry(T,a) {
  const points=[],indices=[],rows=6,cols=12;
  for(let s=0;s<=rows;s++)for(let t=0;t<=cols;t++) {
    const f=s/rows,g=t/cols,p=a.ends[0].map((x,i)=>x+(a.ends[1][i]-x)*f),run=a.runs[0]+(a.runs[1]-a.runs[0])*f;
    const z=a.zBase+(a.zAnchor-a.zBase)*(1-g)**3;
    points.push((p[0]+a.u[0]*run*g)*.001,-(p[1]+a.u[1]*run*g)*.001,z*.001);
  }
  // Each patch has an outward winding in world [x,-y,z]. Its normal
  // vertices stay separate at the sharp side/rear/base joins: averaging the
  // hidden rear into the curved top reverses its contact-corner shading.
  const tri=(a,b,c,n)=>{
    const ab=[0,1,2].map(i=>points[b*3+i]-points[a*3+i]);
    const ac=[0,1,2].map(i=>points[c*3+i]-points[a*3+i]);
    if(dot(cross(ab,ac),n)>0)indices.push(a,b,c);else indices.push(a,c,b);
  };
  const point=(s,t)=>points.slice((s*(cols+1)+t)*3,(s*(cols+1)+t)*3+3);
  const add=p=>{const index=points.length/3;points.push(...p);return index;};
  for(let s=0;s<rows;s++)for(let t=0;t<cols;t++){const k=s*(cols+1)+t;tri(k,k+1,k+cols+1,[0,0,a.side]);tri(k+1,k+cols+2,k+cols+1,[0,0,a.side]);}
  const span=sub(point(rows,0),point(0,0));
  for(const s of [0,rows]) {
    const start=points.length/3,p=point(s,0),normal=span.map(x=>s?x:-x);
    add([p[0],p[1],a.zBase*.001]);for(let t=0;t<=cols;t++)add(point(s,t));
    for(let t=0;t<cols;t++)tri(start,start+t+1,start+t+2,normal);
  }
  // Match the top's span subdivisions on rear and base. Long unsplit edges
  // would leave T-junctions; omitting the base would leave an open surface.
  for(const rear of [true,false]) {
    const start=points.length/3,normal=rear?[-a.u[0],a.u[1],0]:[0,0,-a.side];
    for(let s=0;s<=rows;s++){const p=point(s,0);add([p[0],p[1],a.zBase*.001]);add(point(s,rear?0:cols));}
    for(let s=0;s<rows;s++){const k=start+s*2;tri(k,k+1,k+2,normal);tri(k+1,k+3,k+2,normal);}
  }
  const geom=new T.BufferGeometry();geom.setAttribute('position',new T.Float32BufferAttribute(points,3));geom.setIndex(indices);geom.computeVertexNormals();return geom;
}

/** model is the original scene after the renderer's coordinate normalization.
 * World: Z-up metres. Saved board [x,y,z] mm -> world [x,-y,z]/1000.
 * The returned identity group is OFF and belongs beside model, never inside it.
 */
export function buildSolderPreview({THREE:T,model,components,asset,maxJoints=128}={}) {
  const group=new T.Group();group.name='Illustrative solder';group.visible=false;group.userData.illustrative=true;
  const coverage={label:'Illustrative solder',components:0,joints:0,eligible:0,anchors:[],limits:'Estimated toe fillets only. No measured joint shape, paste volume, wetting, alloy, thermal history or manufacturing acceptance.'},skipped=[];
  const skip=(reference,reason)=>skipped.push({reference,reason});let disposed=false,material=null;
  const result={group,coverage,skipped,dispose(){if(disposed)return;disposed=true;group.removeFromParent();for(const m of [...group.children]){m.geometry?.dispose();group.remove(m);}material?.dispose();}};
  if(!model||components?.schema!==1||!Array.isArray(components.components)||!components.source||!components.sourceRevision||components.source!==asset?.source||components.sourceRevision!==asset?.revision) {skip('','Saved board and asset revision do not match.');return result;}
  if(!Number.isFinite(components.thicknessMm)||components.thicknessMm<=0||components.thicknessMm>10||components.components.length>2000) {skip('','Unsupported board dimensions or component count.');return result;}
  if(!Number.isInteger(maxJoints)||maxJoints<2||maxJoints>256) {skip('','Joint budget must be 2–256.');return result;}
  const ids=new Set(),refs=new Set(),duplicates=new Set();
  for(const c of components.components){if(!c.id||ids.has(c.id)){skip('','Missing or duplicate saved component UUID.');return result;}ids.add(c.id);if(refs.has(c.reference))duplicates.add(c.reference);refs.add(c.reference);}
  const nodes=new Map();model.updateMatrixWorld(true);
  model.traverse(o=>{if(refs.has(o.name)){if(!nodes.has(o.name))nodes.set(o.name,[]);nodes.get(o.name).push(o);}});
  const sourceMeshes=[];let total=0;
  model.traverse(o=>{if(o.isMesh&&o.geometry?.attributes?.position){const count=o.geometry.index?.count??o.geometry.attributes.position.count;total+=Math.floor(count/3);sourceMeshes.push(o);}});
  if(total>300000||sourceMeshes.length>12000) {skip('','Source geometry exceeds preview budget.');return result;}
  const byRef=new Map(),horizontal=[];const p=new T.Vector3();
  for(const mesh of sourceMeshes) {
    let parent=mesh,reference='';while(parent){if(refs.has(parent.name)){reference=parent.name;break;}parent=parent.parent;}
    const geometry=mesh.geometry,pos=geometry.attributes.position,index=geometry.index;
    const ts=reference?(byRef.get(reference)||[]):null;if(reference)byRef.set(reference,ts);
    const count=index?.count??pos.count;
    for(let i=0;i+2<count;i+=3) {
      const t=[];for(let j=0;j<3;j++){p.fromBufferAttribute(pos,index?index.getX(i+j):i+j).applyMatrix4(mesh.matrixWorld);t.push([p.x*1000,-p.y*1000,p.z*1000]);}
      const slot=geometry.groups?.find(g=>i>=g.start&&i<g.start+g.count)?.materialIndex||0;
      t.materialKey=(Array.isArray(mesh.material)?mesh.material[slot]:mesh.material)?.uuid;
      if(t.some(p=>p.some(x=>!Number.isFinite(x))))continue;
      if(ts)ts.push(t);else if(Math.max(...t.map(p=>p[2]))-Math.min(...t.map(p=>p[2]))<.0001&&normal(t))horizontal.push(t);
    }
  }
  for(const c of components.components) {
    const ref=c.reference;
    if(!/^[RC][1-9][0-9]*$/.test(ref))continue;
    coverage.eligible++;
    if(duplicates.has(ref)||nodes.get(ref)?.length!==1) {skip(ref,'Missing or ambiguous original component ancestor.');continue;}
    if(!['F.Cu','B.Cu'].includes(c.layer)||c.pads?.length!==2||new Set(c.pads.map(p=>p.number)).size!==2||c.pads.some(p=>!p.number||p.type!=='smd'||!['rect','roundrect','oval'].includes(p.shape)||!Array.isArray(p.board)||!p.board.every(Number.isFinite)||!Number.isFinite(p.angle)||p.size?.length!==2||p.size.some(x=>!Number.isFinite(x)||x<.1||x>10))) {skip(ref,'Requires two unique supported SMD pads.');continue;}
    if(dist(c.pads[0].board,c.pads[1].board)<.3||dist(c.pads[0].board,c.pads[1].board)>12) {skip(ref,'Unsupported terminal spacing.');continue;}
    const triangles=byRef.get(ref)||[];
    if(!triangles.length||triangles.length>20000) {skip(ref,'Missing or excessive terminal geometry.');continue;}
    const side=c.layer==='F.Cu'?1:-1,boardZ=side===1?components.thicknessMm:0,anchors=[];
    for(let i=0;i<2;i++) {
      const pad=c.pads[i],polygon=padOutline(c,pad),face=polygon.length?padFace(horizontal,polygon,boardZ,side):null;
      const anchor=face?terminalEdge(triangles,polygon,pad,c.pads[1-i],face,side):null;
      if(!anchor)break;anchors.push(anchor);
    }
    // Conservatively require a vendor model with two equal-material end caps
    // and a differently partitioned central body. Material identity is only an
    // additional topology guard: no RGB value or material name is interpreted
    // as proof of a conductive terminal. Monolithic boxes are unsupported.
    const midpoint=c.pads[0].board.map((x,i)=>(x+c.pads[1].board[i])/2),spacing=dist(c.pads[0].board,c.pads[1].board);
    const capMaterial=anchors[0]?.materialKey;
    const distinctBody=capMaterial&&triangles.some(t=>t.materialKey!==capMaterial&&t.every(p=>dist(xy(p),midpoint)<spacing*.6));
    if(anchors.length!==2||anchors[1].materialKey!==capMaterial||!distinctBody) {skip(ref,'Actual matching pad face and unique partitioned terminal boundary edge unavailable.');continue;}
    if(coverage.joints+2>maxJoints) {skip(ref,'Joint budget reached.');continue;}
    material ||=new T.MeshStandardMaterial({color:0xaeb6bb,metalness:1,roughness:.28,side:T.DoubleSide});material.name='Illustrative solder material (estimated)';
    anchors.forEach((a,i)=>{const mesh=new T.Mesh(filletGeometry(T,a),material);mesh.name=`Illustrative solder ${ref} pad ${c.pads[i].number}`;mesh.userData.illustrative=true;group.add(mesh);coverage.anchors.push({reference:ref,pad:c.pads[i].number,sourceUuid:c.id,terminalTriangles:a.terminalTriangles,padTriangles:a.padTriangles,terminalEdgeBoardMm:a.ends.map(p=>[...p,a.bottomEdgeZ]),contactHeightMm:a.zAnchor,padSurfaceZMm:a.zBase-.002*side});});
    coverage.components++;coverage.joints+=2;
  }
  return result;
}
