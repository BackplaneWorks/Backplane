// Run through A-hub.sh node A-drive.mjs run <absolute path to this module>.
// Real desktop/web pixels only; synthetic transcript is in A-demo-events.jsonl.
import fs from 'node:fs';
const R='<capture-root>';
const M=R+'/repo/docs/media/readme';
export default async p=>{
 const cdp=await p.context().newCDPSession(p);
 const metrics=async(w,h)=>{await cdp.send('Emulation.setDeviceMetricsOverride',{width:w,height:h,deviceScaleFactor:2,mobile:false});await p.waitForTimeout(300);};
 const click=async(a,v)=>{let q=p.locator(`[data-on-click="${a}"]${v===undefined?'':`[data-value="${v}"]`}`);await q.filter({visible:true}).first().click();await p.waitForTimeout(250);};
 const shot=async(path)=>{await p.mouse.move(500,650);await p.screenshot({path:M+'/'+path});};
 const fit=async()=>{await p.getByRole('button',{name:'Fit',exact:true}).click();await p.waitForTimeout(300);};
 const ready=async()=>{await p.waitForFunction(()=>document.body.innerText.includes('Studio · original model'),null,{timeout:45000});await p.waitForTimeout(300);};
 await metrics(1600,1000);await click('select','board-demo');await click('flag','models');await shot('source/A-picker-frame.png');await p.keyboard.press('Escape');
 await click('view-kind','3d');await metrics(1600,600);await ready();await fit();await shot('source/A-3d-frame.png');
 await click('view-kind','board');await p.waitForSelector('#plot');await p.waitForTimeout(1800);await fit();await shot('source/A-board-frame.png');
 await click('view-kind','schematic');await metrics(2200,850);await p.locator('.viewer-split').focus();await p.keyboard.press('End');
 await p.locator('[data-on-click="sch-go"]').filter({hasText:'nRF54L15'}).click();await p.waitForTimeout(1300);await p.getByRole('button',{name:'Sheets',exact:true}).click();await fit();
 await p.mouse.move(1350,392);await p.mouse.wheel(0,-500);await p.waitForTimeout(350);await shot('source/A-schematic-frame.png');
 await metrics(1600,600);await click('view-kind','board');await p.locator('.viewer-split').focus();await p.keyboard.press('Home');await p.waitForTimeout(300);await fit();
 await p.mouse.click(1120,180);await p.waitForTimeout(250);await metrics(1600,1000);
 // The current web client has net probing, but no native pad inspector or
 // Mention in chat action. This is an explicitly manual, unsent fixture.
 await p.locator('#composer').fill('Review J2 pad 1 on +BATT. Check the battery supply connection.');await shot('cad/source/A-pad-frame.png');await p.locator('#composer').fill('');
 await click('select','case-demo');await click('mech');await ready();await shot('cad/source/A-mech-frame.png');
 await p.locator('.gear[data-on-click="flag"][data-value="settings"]').click();await click('settings-page','cad');await metrics(1150,800);
 // An actual /proc alias resolves to our own test hub's cwd. It avoids a
 // personal username in the picture without modifying anything outside R.
 const pid=fs.readFileSync(R+'/run/A-capture/hub.pid','utf8').trim();
 await p.locator('#kicad-path').fill(`/proc/${pid}/cwd/home/kicad/10.99.0-bp.4/bin/kicad-cli`);await p.locator('#kicad-path').press('Enter');
 await p.waitForFunction(()=>document.body.innerText.includes('Backplane KiCad 10.99.0-bp.4, API 2, from setting'));await p.mouse.click(895,480);await shot('cad/source/A-settings-frame.png');
};
