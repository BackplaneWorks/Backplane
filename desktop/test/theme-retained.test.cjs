'use strict';
// Light/dark is one fact for the page: a picture retained for a thread (the
// view-transition cache, the focused cache) shows the theme
// of the newest authoritative frame, never the one it was drawn with.
const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const el=(tag,key,attrs=[],kids=[])=>({type:'element',tag,key,attrs,kids});
const cls=v=>({name:'class',value:v});
const composer=el('div','composer',[cls('composer')],[el('textarea','',[{name:'value',value:'draft 日本語'}])]);
const page=(theme,thread,body)=>el('div','app',[{name:'style',value:'--text-scale:125'},cls('app panel picked'+(theme==='light'?' light':'')+' tour-on tour-head')],[
 el('aside','sidebar',[cls('sidebar')],[{type:'text',text:'projects'}]),
 el('main','thread',[cls('thread-view'),{name:'data-thread',value:thread}],[el('div','timeline',[cls('timeline')],[{type:'text',text:body}]),composer])]);
const classOf=n=>n.attrs.find(a=>a.name==='class').value;
const strip=n=>n?.type!=='element'?n:{...n,attrs:n.key==='app'?n.attrs.filter(a=>a.name!=='class'):n.attrs,kids:n.kids.map(strip)};

test('themeOf reads the app root, and a tree without one says nothing',async()=>{
 const {themeOf}=await import('../../src/web/view-transition.js');
 assert.equal(themeOf(page('light','A','x')),'light');assert.equal(themeOf(page('dark','A','x')),'dark');
 assert.equal(themeOf(el('div','',[cls('light')])),'');assert.equal(themeOf({type:'text',text:'light'}),'');
 // a "light" word elsewhere (a message, a chip) is not the theme
 assert.equal(themeOf(el('div','app',[cls('app')],[el('span','',[cls('light')])])),'dark');
});

test('both directions change only the root theme word, keep its order, and keep every other byte',async()=>{
 const {withTheme}=await import('../../src/web/view-transition.js');
 const dark=page('dark','A','retained 日本語'),light=page('light','A','retained 日本語');
 assert.equal(classOf(withTheme(dark,'light')),'app panel picked light tour-on tour-head');
 assert.deepEqual(withTheme(dark,'light'),light);assert.deepEqual(withTheme(light,'dark'),dark);
 assert.deepEqual(strip(withTheme(dark,'light')),strip(dark));
 assert.equal(withTheme(light,'light'),light);assert.equal(withTheme(dark,''),dark);assert.equal(withTheme(dark,'system'),dark);
 assert.equal(classOf(withTheme(el('div','app',[cls('app light')]),'light')),'app light');
 assert.equal(classOf(withTheme(el('div','app',[]),'light')),'light');
 // the composer, its draft and the timeline are untouched
 assert.deepEqual(withTheme(dark,'light').kids[1].kids[1],composer);
});

test('retained thread pictures come back in the current theme, light to dark and dark to light',async()=>{
 const {ViewTransition,withTheme,themeOf}=await import('../../src/web/view-transition.js');
 const cache=new ViewTransition();const scope=sel=>({origin:'o',identity:'i',language:'en',epoch:'e',target:{kind:'thread',id:sel},selected:sel,settings:false,route:null});
 assert(cache.put(scope('A'),page('dark','A','A body')));assert(cache.put(scope('B'),page('light','B','B body')));
 // after a toggle to light, A (seen dark) and B (seen light) both show light; back to dark, both dark
 for(const theme of ['light','dark']) for(const t of ['A','B']){
  const shown=withTheme(cache.get(scope(t)),theme);assert.equal(themeOf(shown),theme,t+' '+theme);
  assert.equal(shown.kids[1].kids[0].kids[0].text,t+' body');
 }
 // the cache itself is not rewritten (a picture stays exactly what the hub sent)
 assert.equal(themeOf(cache.get(scope('A'))),'dark');
});

test('a cold focused picture takes the current theme',async()=>{
 const {withTheme,themeOf}=await import('../../src/web/view-transition.js');
 const {mergeFocused}=await import('../../src/web/focused-cache.js');
 // the local Ui waiting for the hub has no ui.theme: it renders dark
 const waiting=page('dark','N','');
 // a focused record saved while dark, shown after the theme went light (and the reverse)
 const record=t=>({shellAttrs:[cls('app picked'+(t==='light'?' light':'')),{name:'style',value:'--text-scale:110'}],main:el('main','thread',[],[el('div','timeline',[],[{type:'text',text:'saved'}])])});
 for(const [saved,now] of [['dark','light'],['light','dark']]){
  const shown=withTheme(mergeFocused(waiting,record(saved)),now);assert.equal(themeOf(shown),now);
  assert(shown.attrs.some(a=>a.name==='style'&&a.value==='--text-scale:110'));
  assert.deepEqual(shown.kids[1].kids.at(-1),composer);
 }
});

// The authority itself, from host.js's own source: seeded before any frame,
// moved only by an authoritative frame, never by a retained picture.
function authority(stored,desktop){
 const source=fs.readFileSync(path.resolve(__dirname,'../../src/web/host.js'),'utf8');
 const a=source.indexOf('const THEME = '),b=source.indexOf('const themed = ',a);assert(a>0&&b>a);
 const store=new Map(stored?[['backplane-theme',stored]]:[]),root=new Set(),meta={content:'#111410',setAttribute(k,v){this[k]=v;}};
 const sent=[],ctx={window:desktop?{backplaneDesktop:{theme:desktop,themeSeen:t=>sent.push(t)}}:{},localStorage:{getItem:k=>store.get(k)??null,setItem:(k,v)=>store.set(k,v)},
  document:{documentElement:{classList:{toggle:(c,on)=>on?root.add(c):root.delete(c)}},querySelector:()=>meta}};
 vm.createContext(ctx);vm.runInContext(source.slice(a,source.indexOf('\n',b))+'\nthis.seen=themeSeen;this.now=()=>themeNow;',ctx);
 const app=t=>({classList:{contains:c=>c==='light'&&t==='light'}});
 return{ctx,store,root,meta,app,sent};
}
test('the first paint has the last authoritative theme, and a frame order moves it both ways',()=>{
 const cold=authority('light');assert(cold.root.has('light'));assert.equal(cold.meta.content,'#fafaf8');assert.equal(cold.ctx.now(),'light');
 const fresh=authority('');assert(!fresh.root.has('light'));assert.equal(fresh.ctx.now(),'');
 // an authoritative dark frame, then the toggle's light frame, then dark again
 for(const t of ['dark','light','dark','light']){fresh.ctx.seen(fresh.app(t));assert.equal(fresh.ctx.now(),t);assert.equal(fresh.root.has('light'),t==='light');assert.equal(fresh.store.get('backplane-theme'),t);}
 // no app root (a lightbox-only frame, an error page) leaves it alone
 fresh.ctx.seen(null);assert.equal(fresh.ctx.now(),'light');
});
test('host.js shows every retained or waiting picture through the theme, and learns it only from authoritative frames',()=>{
 const source=fs.readFileSync(path.resolve(__dirname,'../../src/web/host.js'),'utf8');
 assert.match(source,/transitionNode = shown \? App\.presentation_node\(presentationJson\(themed\(shown\)\)\)/);
 assert.match(source,/presentationJson\(themed\(mergeFocused\(shell, record\)\)\)/);
 assert.match(source,/presentationJson\(themed\(Wire\.plain\(App\.presentation_view\(App\.presentation_wait\(ui\)\)\)\)\)/);
 assert.match(source,/if \(frontendNode \|\| canonicalBoot && App\.synced\(ui\)\) themeSeen\(root\.querySelector\("\.app"\)\);/);
 // the seed runs before the first render
 assert(source.indexOf('themeDocument();')<source.indexOf('function render()'));
});
test('in the desktop window the page starts from the theme main kept and tells main only what authoritative frames painted',()=>{
 // the page's own word wins; with none, the window's
 const kept=authority('',"light");assert.equal(kept.ctx.now(),'light');assert(kept.root.has('light'));assert.deepEqual(kept.sent,[]);
 const own=authority('dark','light');assert.equal(own.ctx.now(),'dark');assert(!own.root.has('light'));
 // a frame equal to what main has sends nothing; a change is sent once; no app root sends nothing
 kept.ctx.seen(kept.app('light'));assert.deepEqual(kept.sent,[]);
 kept.ctx.seen(kept.app('dark'));kept.ctx.seen(kept.app('dark'));kept.ctx.seen(null);assert.deepEqual(kept.sent,['dark']);
 kept.ctx.seen(kept.app('light'));assert.deepEqual(kept.sent,['dark','light']);
 // main's word older than the page's (an upgrade, another profile's page): the first authoritative frame corrects it
 own.ctx.seen(own.app('dark'));assert.deepEqual(own.sent,['dark']);
 // a plain browser has no desktop bridge and sends nothing
 const web=authority('light');web.ctx.seen(web.app('dark'));assert.deepEqual(web.sent,[]);
});
