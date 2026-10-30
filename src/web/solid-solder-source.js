// Bounded traversal over original source objects. No clones, reparenting,
// geometry edits or alternate anchors: the solder builder still qualifies
// actual pad outlines and terminal edges under its unchanged budgets.
export function selectSolderSource({gltf,components,asset,maxNodes=60000,maxMeshes=12000,maxTriangles=300000}={}){
 const fail=reason=>({view:null,reason});
 if(!gltf?.scene||components?.schema!==1||!Array.isArray(components.components)||components.components.length>2000||!components.source||!components.sourceRevision||components.source!==asset?.source||components.sourceRevision!==asset?.revision)return fail('Saved source/revision or metadata mismatch.');
 if(maxNodes>60000||maxMeshes>12000||maxTriangles>300000||[maxNodes,maxMeshes,maxTriangles].some(n=>!Number.isInteger(n)||n<1))return fail('Candidate limits may not exceed existing bounded budgets.');
 const rows=components.components,refs=new Set(rows.map(c=>c.reference)),rc=new Set(rows.filter(c=>/^[RC][1-9][0-9]*$/.test(c.reference)).map(c=>c.reference));
 const definitions=gltf.parser?.json?.meshes,associations=gltf.parser?.associations;if(!Array.isArray(definitions)||!associations)return fail('Original GLTF associations unavailable.');
 const padDefinitions=[];definitions.forEach((m,i)=>{if(/_pad$/.test(m.name||''))padDefinitions.push(i);});if(padDefinitions.length!==1)return fail('Missing or ambiguous original board-pad definition.');
 const chosen=[],meshes=[],stack=[[gltf.scene,0]];let visited=0,triangles=0,padMeshes=0,rcMeshes=0;
 while(stack.length){const [node,depth]=stack.pop();if(++visited>maxNodes||stack.length+node.children.length>maxNodes)return fail('Source header traversal budget exceeded.');if(depth>128)return fail('Source ancestry depth budget exceeded.');for(let i=node.children.length-1;i>=0;i--)stack.push([node.children[i],depth+1]);
  if(refs.has(node.name))chosen.push(node);if(!node.isMesh||!node.geometry?.attributes?.position)continue;
  let reference='';for(let p=node;p;p=p.parent)if(refs.has(p.name)){reference=p.name;break;}
  const originalMesh=associations.get(node)?.meshes,boardPad=!reference&&originalMesh===padDefinitions[0];if(!boardPad&&!rc.has(reference))continue;
  if(boardPad)padMeshes++;else rcMeshes++;meshes.push(node);chosen.push(node);triangles+=Math.floor((node.geometry.index?.count??node.geometry.attributes.position.count)/3);if(meshes.length>maxMeshes||triangles>maxTriangles)return fail('Qualified candidate geometry exceeds existing preview budgets.');
 }
 if(!padMeshes)return fail('No original board-pad mesh associations qualify.');
 const unique=[...new Set(chosen)];return {view:{updateMatrixWorld(force){gltf.scene.updateMatrixWorld(force);},traverse(callback){for(const node of unique)callback(node);}},receipt:{visitedNodes:visited,meshes:meshes.length,triangles,padMeshes,rcMeshes,padDefinition:padDefinitions[0],padOriginalName:definitions[padDefinitions[0]].name,selection:'Original GLTF board-pad association plus actual saved R/C ancestors',source:asset.source,revision:asset.revision,limits:{maxNodes,maxMeshes,maxTriangles}}};
}
