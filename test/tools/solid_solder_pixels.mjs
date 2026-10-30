// Isolated native-size browser pixels of real saved-board geometry. Requires
// explicit read-only input paths; this is not installed-app acceptance.
import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
const glb=process.env.SOLDER_GLB,metadata=process.env.SOLDER_COMPONENTS,three=process.env.THREE_MODULE;
assert(glb&&metadata&&three,'Set SOLDER_GLB, SOLDER_COMPONENTS and THREE_MODULE.');
const out=resolve(process.env.SOLDER_PIXEL_OUT||'/tmp/backplane-solder-evidence/pixels');mkdirSync(out,{recursive:true});
const sha=b=>createHash('sha256').update(b).digest('hex'),input=readFileSync(glb),data=readFileSync(metadata);
const loader=resolve(dirname(three),'../examples/jsm/loaders/GLTFLoader.js'),source=resolve('src/web/solid-solder.js');
const entry=`import * as T from ${JSON.stringify(three)};
import {GLTFLoader} from ${JSON.stringify(loader)};
import {buildSolderPreview} from ${JSON.stringify(source)};
const [gltf,components]=await Promise.all([new GLTFLoader().loadAsync('/board.glb'),fetch('/components.json').then(r=>r.json())]);
const model=gltf.scene;model.rotation.x=Math.PI/2;model.updateMatrixWorld(true);
const scene=new T.Scene();scene.background=new T.Color(0x252a30);scene.add(model);
const camera=new T.PerspectiveCamera(35,900/650,.0001,.2);camera.position.set(.1225,-.057,.0048);camera.up.set(0,0,1);camera.lookAt(.119,-.053,.0011);
const renderer=new T.WebGLRenderer({canvas:document.querySelector('canvas'),antialias:true,preserveDrawingBuffer:true});renderer.setPixelRatio(1);renderer.setSize(900,650);renderer.outputColorSpace=T.SRGBColorSpace;renderer.toneMapping=T.ACESFilmicToneMapping;renderer.toneMappingExposure=1;
scene.add(new T.HemisphereLight(0xffffff,0x637080,2));const light=new T.DirectionalLight(0xffffff,4);light.position.set(.11,-.06,.025);scene.add(light);const fill=new T.DirectionalLight(0xcbdfff,2);fill.position.set(.13,-.045,.015);scene.add(fill);
const preview=buildSolderPreview({THREE:T,model,components,asset:{source:components.source,revision:components.sourceRevision}});scene.add(preview.group);
window.solderTest={coverage:preview.coverage,skipped:preview.skipped,show(on){preview.group.visible=on;document.querySelector('span').textContent=on?'Illustrative solder · estimated toe fillets':'Original exported geometry';renderer.render(scene,camera);},dispose(){preview.dispose();renderer.dispose();}};solderTest.show(false);window.ready=true;`;
writeFileSync(`${out}/entry.js`,entry);
const built=await Bun.build({entrypoints:[`${out}/entry.js`],target:'browser',format:'esm',minify:false});assert(built.success,String(built.logs));const bundle=await built.outputs[0].text();
const server=Bun.serve({hostname:'127.0.0.1',port:0,fetch(req){const p=new URL(req.url).pathname;if(p==='/entry.js')return new Response(bundle,{headers:{'Content-Type':'application/javascript'}});if(p==='/board.glb')return new Response(input);if(p==='/components.json')return new Response(data);return new Response('<style>body{margin:0;background:#252a30;color:white;font:14px monospace}span{position:absolute;left:14px;top:12px;background:#15191ddd;padding:8px}canvas{width:900px;height:650px;display:block}</style><span></span><canvas></canvas><script type="module" src="/entry.js"></script>',{headers:{'Content-Type':'text/html'}});}});
const {chromium}=await import(pathToFileURL(process.env.PLAYWRIGHT_CORE_PATH||'/home/h/Documents/i2cjak/backplane-bend/tools/browser/node_modules/playwright-core/index.mjs'));
const browser=await chromium.launch({executablePath:'/usr/bin/google-chrome-stable',headless:true,args:['--use-angle=swiftshader','--enable-unsafe-swiftshader']});
try {
  const page=await browser.newPage({viewport:{width:900,height:650},deviceScaleFactor:1}),errors=[];page.on('pageerror',e=>errors.push(String(e)));
  await page.goto(`http://127.0.0.1:${server.port}`);await page.waitForFunction(()=>window.ready,{},{timeout:30000});
  await page.screenshot({path:`${out}/off.png`});await page.evaluate(()=>solderTest.show(true));await page.screenshot({path:`${out}/on.png`});
  const evidence=await page.evaluate(()=>({coverage:solderTest.coverage,skipped:solderTest.skipped,width:document.querySelector('canvas').width,height:document.querySelector('canvas').height}));
  assert.equal(evidence.width,900);assert.equal(evidence.height,650);assert.equal(evidence.coverage.joints,116);assert(evidence.coverage.anchors.some(a=>a.reference==='C5'));assert.deepEqual(errors,[]);
  assert.equal(sha(readFileSync(glb)),sha(input));assert.notEqual(sha(readFileSync(`${out}/off.png`)),sha(readFileSync(`${out}/on.png`)));
  const receipt={glb,glbSha256:sha(input),metadata,metadataSha256:sha(data),moduleSha256:sha(readFileSync(source)),browser:await browser.version(),width:900,height:650,scale:1,scope:'Standalone WebGL test of fresh saved-board export and real component metadata; not installed-app acceptance. Lighting is a neutral test rig, not renderer quality qualification.',...evidence,errors,offSha256:sha(readFileSync(`${out}/off.png`)),onSha256:sha(readFileSync(`${out}/on.png`))};
  writeFileSync(`${out}/receipt.json`,JSON.stringify(receipt,null,2));console.log(JSON.stringify({out,joints:evidence.coverage.joints,errors}));await page.evaluate(()=>solderTest.dispose());
}finally{await browser.close();server.stop(true);}
