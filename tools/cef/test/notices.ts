// Read the pinned runtime's built-in third-party notices on a private page.
import {join} from 'node:path';
import {writeFileSync} from 'node:fs';
import {CefEngine} from '../../browser/src/cef.ts';
import {check,privateDisplay,until} from './harness.ts';
const d=await privateDisplay();
const host=new CefEngine({parent:d.parent,display:d.env.DISPLAY!,cache:join(d.root,'cef'),cef:process.env.BACKPLANE_CEF});
try{
  await host.start();
  const page=await host.create('credits');
  await page.goto('chrome://credits/',{waitUntil:'load'});
  await until(async()=>await page.evaluate(()=>document.body.textContent!.length>10000),'runtime third-party credits');
  const html=await page.content();
  check('pinned runtime exposes third-party license notices',/license/i.test(html)&&html.length>10000);
  if(process.env.CEF_CREDITS_OUT)writeFileSync(process.env.CEF_CREDITS_OUT,html);
  console.log(JSON.stringify({creditsHtmlBytes:Buffer.byteLength(html)}));
}finally{await host.stop();await d.cleanup();}
