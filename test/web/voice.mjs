// The temporary hub launches this helper for local recognition. Its input file
// lets the test compare captured PCM with the bytes delivered through ingress.
export async function installVoiceStandin(temp) {
  const fs = await import('node:fs/promises');
  const path = await import('node:path');
  const helper = path.join(temp, 'backplane-voice');
  await fs.writeFile(helper, `#!/bin/sh
case "$1" in
probe) exit 0 ;;
local-status) echo '{"event":"local","platform":"linux-x64","supported":true,"runtime":true,"vad":true,"models":[{"id":"parakeet-v2","installed":true}]}' ;;
live) echo '{"event":"ready"}'; cat > "$BP_WEB_FIXTURE/fixture-audio.pcm" ;;
esac
echo '{"event":"end"}'
`, {mode:0o700});
  return helper;
}

export async function checkVoice({page, context, origin, rpc, thread, temp}) {
  const { default: assert } = await import('node:assert/strict');
  const fs = await import('node:fs/promises');
  await fs.mkdir(temp + "/hub/voice/local", {recursive:true});
  await fs.writeFile(temp + "/hub/voice/local/ready-parakeet-v2", "ok");
  const other = await context.browser().newContext();
  try {
    const peer = await other.newPage();
    await peer.goto(origin);
    await peer.getByTestId('pair-name').fill('Second voice browser');
    await peer.getByTestId('pair-request-btn').click();
    await peer.getByTestId('pair-code').waitFor();
    const {rows} = await rpc('devices.page', {});
    const row = rows.find(r => r.label === 'Second voice browser');
    await rpc('devices.page.act', row.buttons.find(b => b.action === 'device-web-approve'));
    await peer.getByTestId('sidebar-list').waitFor();
    const session = await page.evaluate(thread => new Promise((resolve, reject) => {
      const ws = window.fixtureVoiceSocket = new WebSocket('/ws?data=1&enc=json');
      ws.onmessage = e => {
        const f = JSON.parse(e.data);
        if (f.t === 'hello') ws.send(JSON.stringify({id:1,m:'voice.start',p:{thread}}));
        if (f.t === 'reply' && f.id === 1) {
          if (f.result?.session) resolve(f.result.session);
          else reject(Error('fixture voice session unavailable'));
        }
      };
    }), thread);
    const post = ({session, samples, end}) => fetch('/voice/in', {method:'POST', headers:{
      'X-Voice-Session':session,'X-Voice-End':end?'1':'0'
    }, body:new Uint8Array(samples)}).then(r=>r.status);
    assert.equal(await peer.evaluate(post,{session,samples:[99]}),403);
    const samples = await page.evaluate(async () => {
      const ctx = new OfflineAudioContext(1,1600,16000);
      const oscillator = ctx.createOscillator();
      oscillator.connect(ctx.destination);
      oscillator.start();
      const audio = await ctx.startRendering();
      return Array.from(new Uint8Array(Int16Array.from(audio.getChannelData(0), x=>Math.round(x*8192)).buffer));
    });
    assert.equal(await page.evaluate(post,{session,samples}),200);
    assert.equal(await page.evaluate(post,{session,samples:[],end:true}),200);
    await assert.doesNotReject(async () => {
      for (let n=0;n<100;n++) {
        const audio = await fs.readFile(temp+'/fixture-audio.pcm').catch(()=>null);
        if (audio?.equals(Buffer.from(samples))) return;
        await Bun.sleep(25);
      }
      throw Error('voice helper did not receive expected PCM');
    });
    await page.evaluate(()=>window.fixtureVoiceSocket.close());
  } finally { await other.close(); }
}
