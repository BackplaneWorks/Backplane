import * as fs from 'node:fs';
import {createHash} from 'node:crypto';
import {checked_refs,term_refs,save_checked_refs} from './compiler/bend2/references.ts';
import * as assert from 'node:assert/strict';
import * as B from './compiler/bend2/bend.ts';
import {TermStore, stored_body} from './compiler/bend2/offload.ts';
const book=B.book_nil();
const file:any={str:'def value() -> U32:\n  1\n',ns:'',al:Object.create(null),book};
const span:B.Span={file,beg:0,end:3};
book.tlds.value={$:'Def',n:0,x:0,T:B.Ref('U32',span),v:B.Lit('U32',1,span),e:B.Lit('U32',1,span)};
book.tlds.Flag={$:'ADT',n:0,g:0,T:B.Typ(B.Qua(B.Lone())),c:[{k:'Yes',n:0,T:B.Ref('Flag',span)}]};
book.ctrs.Yes=(book.tlds.Flag as B.ADT).c[0];book.order.push('value','Flag');
const key=(t:B.HTerm)=>B.term_key(B.term_lower(t));
const original=book.tlds.value as B.Def;
const expected=[key(original.T),key(original.v!),B.term_key(original.e!)];
const store=new TermStore(1);
store.replace(book,['value','Flag']);store.clear();
const value=book.tlds.value as B.Def;
const priorReads=store.reads;
assert.notEqual(value.v,null);
assert.equal(store.reads,priorReads,'presence check must not decode body');
assert.equal(value.v,original.v,'retain exact source body graph');
assert.equal(value.v!.s!.file.str,file.str);

const liveType=value.T;store.clear();assert.equal(value.T,liveType,'LRU eviction must preserve still-live graph identity');
assert.deepEqual([key(value.T),key(value.v!),B.term_key(value.e!)],expected);
assert.equal(Object.getOwnPropertyDescriptor(value,'T')!.enumerable,false);
assert.equal(stored_body(value),true);
assert.equal(value.T.s!.file.str,file.str);assert.equal(value.T.s!.beg,span.beg);
assert.equal(Object.hasOwn(value.T.s!.file,'book'),false);
assert.equal(key(book.ctrs.Yes.T),key(B.Ref('Flag',span)));
value.v=null;assert.equal(stored_body(value),false);value.v=B.Lit('U32',2);assert.equal(stored_body(value),true);
assert.equal(key(value.v!),key(B.Lit('U32',2)));
// Infer/Check embed a higher-order open type in a lower annotation's share cell.
const open=B.book_nil();
const annotation=B.All(B.Lone(),'y',1,B.Ref('U32'),(y:B.HTerm)=>B.Eql(B.Var('x',0),y,B.Ref('U32')));
const erased=B.Lam('x',0,B.Ann(B.Var('x',0),B.Var('_',-1,undefined,annotation))) as B.LTerm;
open.tlds.open={$:'Def',n:1,x:0,T:B.Ref('U32'),v:B.Lit('U32',0),e:erased};
const mixed=new TermStore();mixed.replace(open,['open']);mixed.clear();
const restored:any=B.term_force(((open.tlds.open as B.Def).e as any).f.T.v);
assert.equal(B.term_key(B.term_lower(restored,1)),B.term_key(B.term_lower(annotation,1)));
assert.equal((mixed as any).interfaces.every((v:any)=>typeof v==='string'),true,'only blob keys remain resident');
assert.equal((mixed as any).openVariables.every((v:any)=>v.$==='Var' && v.v===undefined),true,'only free variable leaves may stay resident');
assert.throws(()=>(mixed as any).decode({'@open':'99999999'}),/missing open variable handle/);
const given=B.Var('given',77);
const applied:any=restored.B(given);
assert.equal(applied.b,given,'dependent binder must preserve caller variable identity');
assert.equal(applied.a.$,'Var');assert.equal(applied.a.i,0);assert.equal(applied.a.k,'x');
assert.equal(applied.b.$,'Var');assert.equal(applied.b.i,77);
// Kernel head memoization stays exact after eviction, without retaining ASTs.
const memo=B.book_nil();memo.tlds.memo={$:'Def',n:0,x:0,T:B.Ref('U32'),v:B.Lit('U32',7)};
const exactBody=(memo.tlds.memo as B.Def).v;
const memoStore=new TermStore(1);memoStore.replace(memo,['memo']);memoStore.clear();
const cell=(memo.tlds.memo as B.Def).v!;
assert.equal(memoStore.reads,0);
const first=B.term_wnf(memo,B.Ref('memo'));
assert.equal(B.u32_from_term(first),7);
assert.equal(cell,exactBody);
memoStore.clear();assert.equal(B.u32_from_term(B.term_wnf(memo,B.Ref('memo'))),7);
// Snapshot interning preserves repeated types without retaining mutable types
// across later snapshots. Existing checked state remains the old exact bytes.
const shared=B.Ref('Before');
const repeated=B.Ctr('Fixture',Array.from({length:1000},()=>B.Ann(B.Lit('U32',1),B.Var('_',-1,undefined,shared)))) as B.LTerm;
const sharing=B.book_nil();sharing.tlds.one={$:'Def',n:0,x:0,T:B.Ref('U32'),v:B.Lit('U32',1),e:repeated};
const sharingStore=new TermStore();sharingStore.replace(sharing,['one']);sharingStore.clear();
const priorBytes=sharingStore.bytes;
(shared as any).k='After';
sharing.tlds.two={$:'Def',n:0,x:0,T:B.Ref('U32'),v:B.Lit('U32',1),e:repeated};
sharingStore.replace(sharing,['two']);sharingStore.clear();
assert.equal((B.term_force((sharing.tlds.one as any).e.x[0].T.v) as any).k,'Before');
assert.equal((B.term_force((sharing.tlds.two as any).e.x[0].T.v) as any).k,'After');
assert.ok(priorBytes<800000,'repeated annotation snapshot must stay bounded');
// A repeated lower subtree remains one node through encode + binary storage +
// decode. JSON tree expansion would destroy this invariant and blow the cap.
const graph=B.book_nil();const node=B.Lit('U32',9);
graph.tlds.graph={$:'Def',n:0,x:0,T:B.Ref('U32'),v:B.Lit('U32',1),e:B.Ctr('Fixture',Array(1000).fill(node)) as B.LTerm};
const graphStore=new TermStore();graphStore.replace(graph,['graph']);graphStore.clear();
const result:any=(graph.tlds.graph as B.Def).e;
assert.equal(result.x[0],result.x[999]);
assert.ok(graphStore.bytes<60000,'binary graph must not expand repeated subtrees');
assert.throws(()=>(graphStore as any).decode({'@interface':'99999999'}),/missing checked interface handle/);
assert.throws(()=>(graphStore as any).decode({'@interface':'invalid'}),/invalid checked interface handle/);
// Streaming UTF8 decoding must survive a codepoint crossing the 64KiB read.
const unicode=B.book_nil();const text='a'.repeat(65400)+'🛠'.repeat(1000);
unicode.tlds.unicode={$:'Def',n:0,x:0,T:B.Ref('String'),v:B.Lit('String',text),e:B.Lit('String',text)};
const unicodeStore=new TermStore();unicodeStore.replace(unicode,['unicode']);unicodeStore.clear();
assert.equal((unicode.tlds.unicode as any).e.v,text);
const graphFiles=fs.readdirSync(graphStore.directory);assert.equal(graphFiles.length,1);
const graphText=fs.readFileSync(graphStore.directory+'/'+graphFiles[0],'utf8');
const records=graphText.trimEnd().split('\n');const header=JSON.parse(records[0]);
assert.equal(JSON.parse(records[records.length-1]).count,3,'one repeated lower node');
const incomplete=records.slice(0,-1).join('\n')+'\n';
const brokenKey=createHash('sha256').update(incomplete).digest('hex');fs.writeFileSync(graphStore.directory+'/'+brokenKey,incomplete);
assert.throws(()=>(graphStore as any).getTerm(brokenKey),/incomplete or changed checked graph/);
const different=new TermStore();fs.writeFileSync(different.directory+'/'+graphFiles[0],graphText);
assert.throws(()=>(different as any).getTerm(graphFiles[0]),/another process store/);
const actualRefs=new Set<string>();term_refs(repeated,actualRefs);
// Capture the current source snapshot, then compare the exact traversal's set.
const refBook=B.book_nil();refBook.tlds.refs={$:'Def',n:0,x:0,T:B.Ref('U32'),v:B.Lit('U32',1),e:repeated};
const refStore=new TermStore();refStore.replace(refBook,['refs']);
assert.deepEqual([...checked_refs(refBook.tlds.refs)!].sort(),[...actualRefs].sort());
(refBook.tlds.refs as B.Def).e=B.Ref('updated') as B.LTerm;
assert.deepEqual([...checked_refs(refBook.tlds.refs)!],['updated'],'result assignment invalidates dependency set');
for (let i=0;i<32;i++) {
  const common=B.Ref('common'+i);
  const ty=B.All(B.Lone(),'x',0,B.Ref('domain'+i),x=>B.Ref('unopened'+i));
  const tree=B.Ctr('Fixture',[common,common,B.Ann(B.Ref('used'+i),B.Var('_',-1,undefined,ty)),B.ADT('family'+i,[B.Ref('arg'+i)])]) as B.LTerm;
  const exact=new Set<string>();term_refs(tree,exact);
  const own=B.book_nil();own.tlds.edge={$:'Def',n:0,x:0,T:B.Ref('U32'),v:B.Lit('U32',1),e:tree};
  const edgeStore=new TermStore();edgeStore.replace(own,['edge']);
  assert.deepEqual([...checked_refs(own.tlds.edge)!].sort(),[...exact].sort());
  assert.equal(checked_refs(own.tlds.edge)!.has('unopened'+i),false,'functions stay opaque exactly as original traversal');
}
const checking=B.book_nil();const rawBody=B.Lit('U32',1);
checking.tlds.checking={$:'Def',n:0,x:0,T:B.Ref('U32'),v:rawBody,e:B.Ref('unsafe-dependency') as B.LTerm};
const noEmit=new TermStore(1,true);noEmit.replace(checking,['checking']);
assert.equal((checking.tlds.checking as B.Def).v,rawBody);
assert.equal(noEmit.bytes,0);assert.deepEqual([...checked_refs(checking.tlds.checking)!],['unsafe-dependency']);
assert.throws(()=>(checking.tlds.checking as B.Def).e,/not retained for check-only/);
save_checked_refs(checking.tlds.checking,undefined);
assert.equal(checked_refs(checking.tlds.checking),undefined);
assert.throws(()=>(checking.tlds.checking as B.Def).e,/not retained for check-only/,'missing dependency cache must never silently omit bodies');
// Corrupt a content-addressed term after write, before hydration: fail closed.
const other=B.book_nil();other.tlds.x={$:'Def',n:0,x:0,T:B.Ref('OnlyT'),v:B.Lit('U32',2),e:B.Lit('U32',1)};
const corrupt=new TermStore();corrupt.replace(other,['x']);
const targets=fs.readdirSync(corrupt.directory);assert.equal(targets.length,1);const target=targets[0];
assert.ok(target);fs.writeFileSync(corrupt.directory+'/'+target,'{}');
assert.throws(()=>(other.tlds.x as B.Def).e,/checked-term storage changed/);
// Distinct but byte-identical accepted type roots share one blob handle.
const clones=B.book_nil();const cloneOriginal:any={$:'Def',n:0,x:0,T:B.Ref('U32'),v:B.Lit('U32',1),
 e:B.Ctr('Fixture',Array.from({length:2000},()=>B.Ann(B.Lit('U32',1),B.Var('_',-1,undefined,B.Ref('Same')))))};
clones.tlds.clones=cloneOriginal;const cloneStore=new TermStore();cloneStore.replace(clones,['clones']);
assert.equal(cloneOriginal.e,undefined,'obsolete accepted emitter IR must be released');
assert.equal((cloneStore as any).interfaces.length,1,'content-identical type handles must intern');
assert.ok((cloneStore as any).interfaceIds.size<=256,'identity cache is bounded independently of input size');
assert.equal((B.term_force((clones.tlds.clones as any).e.x[1999].T.v) as any).k,'Same');
// Lazy higher blobs retain their own hash and per-process identity checks.
const badType=B.book_nil();badType.tlds.badType={$:'Def',n:0,x:0,T:B.Ref('U32'),v:B.Lit('U32',1),
 e:B.Ann(B.Lit('U32',1),B.Var('_',-1,undefined,B.Ref('CorruptType')))};
const badTypeStore=new TermStore();badTypeStore.replace(badType,['badType']);
const unopened=(badType.tlds.badType as any).e.T.v;
const higherKey=(badTypeStore as any).interfaces[0];
const anotherStore=new TermStore();fs.copyFileSync(badTypeStore.directory+'/'+higherKey,anotherStore.directory+'/'+higherKey);
assert.throws(()=>(anotherStore as any).higherInterface(higherKey),/another process store/);
fs.writeFileSync(badTypeStore.directory+'/'+higherKey,'{}');
assert.throws(()=>B.term_force(unopened),/checked-term storage changed/);
console.log(JSON.stringify({codec_roundtrip:true,span_text_preserved:true,no_parser_book_retained:true,constructor_interface:true,open_dependent_annotation:true,kernel_assignment_semantics:true,corruption_rejected:true,body_presence_no_decode:true,exact_source_body_retained:true,shared_annotations:true,mutable_snapshot_isolation:true,live_graph_identity_after_eviction:true,exact_interface_graph_retained:true,lower_graph_sharing_preserved:true,process_private_interface_handles:true,paged_higher_annotation_dag:true,content_interned_higher_handles:true,bounded_identity_cache:true,obsolete_ir_released:true,lazy_higher_corruption_rejected:true,higher_cross_store_rejected:true,only_free_variables_resident:true,missing_interface_rejected:true,bound_variable_identity:true,streamed_graph:true,unicode_boundary:true,incomplete_graph_rejected:true,cross_store_rejected:true,exact_dependency_sets:true,dependency_assignment_invalidation:true,stream_reference_sets_exact_on_32_graphs:true,check_only_storage_separate:true,missing_dependency_fails_loudly:true}));
