import fs from 'node:fs';
import crypto from 'node:crypto';
export default async function(page,{R,source}) {
 const raw=source+'/raw'; fs.mkdirSync(raw,{recursive:true});
 const skip=page.locator('[data-on-click="ob-skip"]');
 if(await skip.count()) {await skip.click(); await page.waitForTimeout(600);}
 await page.locator('[data-on-click="select"][data-value="demo-main"]').click();
 await page.waitForTimeout(900);
 await page.screenshot({path:raw+'/selected.png'});
 const panels={};
 const rect=async locator=>locator.evaluate(e=>{const r=e.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height};});
 const crop=async(name,r)=>{
   const clip={x:Math.floor(r.x),y:Math.floor(r.y),width:Math.ceil(r.width),height:Math.ceil(r.height)};
   await page.screenshot({path:raw+'/'+name+'.png'});
   await page.screenshot({path:source+'/'+name+'.png',clip});
   panels[name]={raw:'raw/'+name+'.png',cropCSS:clip,deviceScaleFactor:2};
 };
 // Preserve the actual current rendering; never replace labels with invented chips.
 const linkRect=await page.locator('a.ref').first().evaluate(e=>{const ps=e.closest('.body').querySelectorAll('p');const a=ps[0].getBoundingClientRect(),z=ps[3].getBoundingClientRect();return {x:a.x-6,y:a.y-6,width:500,height:z.bottom-a.y+12};});
 await crop('link-cards',linkRect);
 const navigation=[];
 for(const [kind,id] of [['thread','demo-layout'],['bot','demo-miso'],['project','demo-board']]) {
   const target=page.locator(`a[data-ref="backplane://${kind}/${id}"]`);
   await target.click(); await page.waitForTimeout(650);
   await page.screenshot({path:raw+'/navigation-'+kind+'.png'});
   navigation.push({kind,uri:`backplane://${kind}/${id}`,text:await page.locator('body').innerText()});
   await page.locator('[data-on-click="select"][data-value="demo-main"]').click(); await page.waitForTimeout(350);
 }
 fs.writeFileSync(source+'/navigation.json',JSON.stringify(navigation,null,2)+'\n');
 const composer=page.locator('#composer');
 for(const [name,text] of [['thread','>'],['room','#'],['project','%'],['bot','@']]) {
   await composer.fill(text); await page.waitForTimeout(550);
   await page.locator('.composer .skills').waitFor();
   const menu=await rect(page.locator('.composer .skills'));
   const field=await rect(composer);
   await crop('reference-'+name,{x:menu.x-5,y:menu.y-5,width:menu.width+10,height:field.y+38-menu.y+10});
   console.log('reference',name,await page.locator('.composer .skills').innerText());
 }
 await composer.fill('/bt'); await page.waitForTimeout(450);
 const menu=await rect(page.locator('.composer .skills'));
 const box=await rect(page.locator('.composer'));
 await crop('btw-autocomplete',{x:menu.x-5,y:menu.y-5,width:menu.width+10,height:box.y+box.height-menu.y+5});
 console.log('btw',await page.locator('.composer .skills').innerText());
 await composer.fill(''); await page.waitForTimeout(350);
 const token=fs.readFileSync(R+'/run/C-features/home/token','utf8').trim();
 const key=crypto.createHmac('sha256',token).update('mcp\ndemo-main').digest('hex');
 const mcp=async(name,args)=>{
   const answer=await fetch('http://127.0.0.1:47830/mcp/demo-main?k='+key,{method:'POST',headers:{'Content-Type':'application/json','Accept':'application/json, text/event-stream'},body:JSON.stringify({jsonrpc:'2.0',id:1,method:'tools/call',params:{name,arguments:args}})});
   const result=await answer.json();
   fs.writeFileSync(source+'/'+name+(args.text?.includes(':q')?'-quit':'')+'.json',JSON.stringify({request:{name,arguments:args},response:result},null,2)+'\n');
   if(result.error||result.result?.isError) throw Error(JSON.stringify(result));
   return result;
 };
 const opened=await mcp('terminal_open',{command:"exec /usr/bin/nvim -u NONE -i NONE -n -R --cmd 'set title titlestring=Review\\ checklist' review.txt",title:'Review checklist · vi (Neovim)',cols:88,rows:16});
 console.log('terminal_open',opened);
 const id=JSON.parse(opened.result.content[0].text).id;
 await page.waitForTimeout(700);
 await mcp('terminal_send',{id,text:'\rjj'});
 await page.waitForTimeout(600);
 const read=await mcp('terminal_read',{id});
 const terminalText=read.result.content[0].text;
 if(!terminalText.includes('SENSOR BOARD / DESIGN REVIEW')||!terminalText.includes('state="running"')||!terminalText.includes('[RO]')) throw Error('Running read-only checklist did not render in terminal');
 const term=page.locator('.emb').last();
 await term.scrollIntoViewIfNeeded();
 const terminal=await rect(term);
 await crop('inline-terminal',{x:terminal.x-6,y:terminal.y-6,width:terminal.width+12,height:terminal.height+12});
 await mcp('terminal_send',{id,text:'\u001b:q\r'});
 await mcp('terminal_close',{id});
 fs.writeFileSync(source+'/panels.json',JSON.stringify(panels,null,2)+'\n');
 const events=fs.readFileSync(R+'/run/C-features/home/events.jsonl','utf8').trim().split('\n').map(s=>JSON.parse(s));
 const seeded=fs.readFileSync(source+'/demo-events.jsonl','utf8').trim().split('\n').map(s=>JSON.parse(s));
 const eventKinds={}; for(const e of events) eventKinds[e.$]=(eventKinds[e.$]||0)+1;
 const messages=events.filter(e=>e.$==='MessagePosted');
 const audit={eventKinds,messageCount:messages.length,messagesMatchSeed:JSON.stringify(messages)===JSON.stringify(seeded.filter(e=>e.$==='MessagePosted')),agentTurnEvents:events.filter(e=>['TurnChanged','TurnQueued','TurnUsage'].includes(e.$)).length,btwSubmitted:false};
 fs.writeFileSync(source+'/agent-audit.json',JSON.stringify(audit,null,2)+'\n');
 if(!audit.messagesMatchSeed||audit.agentTurnEvents!==0) throw Error('Unexpected messages or agent turns');
}
