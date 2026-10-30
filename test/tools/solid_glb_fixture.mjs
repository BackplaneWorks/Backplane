// Read-only evidence adapter: existing GLB scene -> the production Solid wire.
// No design, export or cache file is changed. Coordinates use SE.v's axis swap
// and 10um quantization; baseColorFactor stays linear as in glb.bend.
import {readFileSync} from 'node:fs';
export function glbFixture(path, select) {
  const bytes=readFileSync(path), len=bytes.readUInt32LE(12);
  const json=JSON.parse(bytes.subarray(20,20+len).toString());
  const bin=bytes.subarray(28+len);
  const accessor=index=>{
    const a=json.accessors[index],v=json.bufferViews[a.bufferView];
    const types={5121:[1,'readUInt8'],5123:[2,'readUInt16LE'],5125:[4,'readUInt32LE'],5126:[4,'readFloatLE']};
    const [size,read]=types[a.componentType],width={SCALAR:1,VEC3:3}[a.type];
    if(!width)throw Error(`unsupported accessor ${a.type}`);
    return Array.from({length:a.count},(_,i)=>Array.from({length:width},(_,k)=>bin[read]((v.byteOffset||0)+(a.byteOffset||0)+i*(v.byteStride||size*width)+k*size)));
  };
  const id=[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1];
  const mul=(a,b)=>Array.from({length:16},(_,i)=>{const row=i%4,col=Math.floor(i/4);return [0,1,2,3].reduce((s,k)=>s+a[k*4+row]*b[col*4+k],0)});
  const matrix=node=>{
    if(node.matrix)return node.matrix;
    const [x,y,z,w]=node.rotation||[0,0,0,1],s=node.scale||[1,1,1],t=node.translation||[0,0,0];
    return [(1-2*y*y-2*z*z)*s[0],(2*x*y+2*w*z)*s[0],(2*x*z-2*w*y)*s[0],0,
      (2*x*y-2*w*z)*s[1],(1-2*x*x-2*z*z)*s[1],(2*y*z+2*w*x)*s[1],0,
      (2*x*z+2*w*y)*s[2],(2*y*z-2*w*x)*s[2],(1-2*x*x-2*y*y)*s[2],0,...t,1];
  };
  const triangles=[];
  const walk=(index,parent,chosen)=>{
    const node=json.nodes[index],m=mul(parent,matrix(node)),take=chosen||!select||node.name===select;
    if(take&&node.mesh!==undefined)for(const p of json.meshes[node.mesh].primitives){
      if(p.mode!==undefined&&p.mode!==4)continue;
      const pos=accessor(p.attributes.POSITION),indices=p.indices===undefined?pos.map((_,i)=>[i]):accessor(p.indices);
      const c=json.materials?.[p.material]?.pbrMetallicRoughness?.baseColorFactor||[1,1,1,1];
      const color=c.slice(0,3).reduce((a,x)=>a*256+Math.round(Math.max(0,Math.min(x,1))*255),0);
      for(let k=0;k<indices.length;k+=3){
        const points=indices.slice(k,k+3).map(([i])=>{
          const [x,y,z]=pos[i],p=[0,1,2].map(a=>m[a]*x+m[4+a]*y+m[8+a]*z+m[12+a]);
          return [p[0],p[2],p[1]].map(x=>Math.round(x*100000));
        });
        triangles.push({color,points});
      }
    }
    for(const child of node.children||[])walk(child,m,take);
  };
  for(const i of json.scenes[json.scene||0].nodes)walk(i,id,false);
  if(!triangles.length)throw Error('No triangles selected');
  const wire=[],box=[Infinity,Infinity,Infinity,-Infinity,-Infinity,-Infinity];
  const put=v=>{do{const b=v%128;v=Math.floor(v/128);wire.push(b|(v?128:0))}while(v)};
  let color=0,previous=[0,0,0];
  for(const t of triangles){put((color^t.color)>>>0);color=t.color;
    for(const p of t.points)for(let i=0;i<3;i++){const d=p[i]-previous[i];put(d<0?-d*2-1:d*2);previous[i]=p[i];box[i]=Math.min(box[i],p[i]);box[i+3]=Math.max(box[i+3],p[i]);}
  }
  return {n:triangles.length,mesh:new Uint8Array(wire),box3:box};
}
