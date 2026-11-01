// Fresh run: A-round2-hub.sh node A-drive.mjs run <absolute path to this module>.
// Current desktop/web pixels at 2x; all actions use the real UI.
const R='<capture-root>';
const M=R+'/repo/docs/media/readme';
export default async p=>{
 const cdp=await p.context().newCDPSession(p);
 const metrics=async(w,h)=>{await cdp.send('Emulation.setDeviceMetricsOverride',{width:w,height:h,deviceScaleFactor:2,mobile:false});await p.waitForTimeout(400);};
 const click=async(a,v)=>{await p.locator(`[data-on-click="${a}"][data-value="${v}"]`).filter({visible:true}).first().click();await p.waitForTimeout(300);};
 await p.locator('.gear[data-on-click="flag"][data-value="settings"]').click();
 await click('settings-page','cad');await metrics(1150,800);
 await p.waitForFunction(()=>document.body.innerText.includes('Backplane KiCad 10.99.0-bp.4, API 2, from installed'));
 if(await p.locator('#kicad-path').inputValue())throw Error('KiCad override must be unset');
 const placeholder=await p.locator('#kicad-path').getAttribute('placeholder');
 if(!placeholder.includes('/tmp/readme-demo/home/kicad/'))throw Error('Unexpected discovery path');
 await p.mouse.move(895,480);await p.screenshot({path:M+'/cad/source/A-settings-frame.png'});
 await p.keyboard.press('Escape');await metrics(2000,750);
 await click('select','board-demo');await click('view-kind','split');
 await p.waitForSelector('#plot-pcb');await p.waitForTimeout(7000);
 await p.locator('.viewer-split').focus();await p.keyboard.press('Home');await p.waitForTimeout(1000);
 // J2 pad 1: actual pointer click, resulting in net +BATT / pin J2.1.
 await p.mouse.click(1576,165);await p.waitForTimeout(1200);
 await p.locator('summary').filter({hasText:'Sheets'}).click();
 await p.locator('.combo-sheets [data-on-click="sch-go"]').filter({hasText:'Connectors'}).click();
 await p.waitForFunction(()=>document.querySelector('#plot-sch')?.dataset.key.includes('/sch/Connectors.kicad_sch'));
 await p.waitForTimeout(1200);
 // Two panes keep the README crop wide; no 3D net highlight is claimed.
 await metrics(2200,650);await p.locator('.combo-picks [data-on-click="split-pane"][data-value="3d"]').click();
 await p.locator('.probe-name').click();await p.waitForTimeout(500);
 await metrics(2500,650);
 const b=await p.locator('#plot-pcb').boundingBox();
 await p.mouse.move(b.x+b.width/2,b.y+b.height/2);await p.mouse.wheel(0,-40);await p.waitForTimeout(500);
 if(await p.locator('#composer').inputValue())throw Error('Composer must be empty');
 const probes=await p.locator('#plot-pcb,#plot-sch').evaluateAll(es=>es.map(e=>({id:e.id,probe:e.dataset.probe,key:e.dataset.key})));
 if(probes.length!==2||probes.some(v=>v.probe!=='n\t+BATT\tJ2.1'))throw Error('Unexpected probe');
 console.log(JSON.stringify({kicad:placeholder,probes}));
 await p.mouse.move(300,500);await p.screenshot({path:M+'/cad/source/A-pad-frame.png'});
};
