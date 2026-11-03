const assert = require('node:assert/strict'), fs = require('node:fs'), os = require('node:os'), path = require('node:path'), test = require('node:test');
const source = fs.readFileSync('src/server/effects/message_image.js', 'utf8').split('io_eff(CID(MessageImage.text)')[0];
const api = new Function('require', 'process', source + ';return {snapshot:mi_snapshot,type:mi_type,text:mi_text};');
const png = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aWQkAAAAASUVORK5CYII=', 'base64');
function fixture(t) {
 const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-message-images-'));
 t.after(()=>fs.rmSync(home,{recursive:true,force:true}));
 return {home, api:api(require,{env:{...process.env,BACKPLANE_HOME:home},platform:process.platform,getuid:()=>process.getuid()})};
}
test('retained bytes survive source deletion, restart and mutation',t=>{
 const f=fixture(t), image=path.join(f.home,'render.png');fs.writeFileSync(image,png);
 const saved=f.api.snapshot('message',image);assert(saved);assert.deepEqual(fs.readFileSync(saved),png);
 fs.unlinkSync(image);assert.equal(f.api.snapshot('message',image),saved);
 const reopened=api(require,{env:{...process.env,BACKPLANE_HOME:f.home},platform:process.platform,getuid:()=>process.getuid()});assert.equal(reopened.snapshot('message',image),saved);
 assert.equal(f.api.snapshot('other-message',image),'');
 fs.writeFileSync(saved,'changed');assert.equal(f.api.snapshot('message',image),'');
});
test('reject traversal, symlinks, directories, wrong content, and encoded/decoded oversize',t=>{
 const f=fixture(t), image=path.join(f.home,'ok.png');fs.writeFileSync(image,png);
 const linked=path.join(f.home,'link.png');fs.symlinkSync(image,linked);
 const parent=path.join(f.home,'alias');fs.symlinkSync(f.home,parent);
 const text=path.join(f.home,'fake.png');fs.writeFileSync(text,'<svg xmlns="http://www.w3.org/2000/svg"><script>bad()</script></svg>');
 const large=path.join(f.home,'large.png');const fd=fs.openSync(large,'w');fs.ftruncateSync(fd,24*1024*1024+1);fs.closeSync(fd);
 const huge=path.join(f.home,'huge.png'), bytes=Buffer.from(png);bytes.writeUInt32BE(100000,16);bytes.writeUInt32BE(100000,20);fs.writeFileSync(huge,bytes);
 for(const file of [linked,path.join(parent,'ok.png'),f.home, text,large,huge, f.home+'/x/../ok.png', 'relative.png',image+'\0', '/etc/passwd']) assert.equal(f.api.snapshot('message',file),'',file);
 assert.equal(f.api.text('unknown'),'');
});
test('private manifest cannot redirect into arbitrary file reads',t=>{
 const f=fixture(t), image=path.join(f.home,'ok.png');fs.writeFileSync(image,png);assert(f.api.snapshot('m',image));
 const dir=path.join(f.home,'message-images'), ref=fs.readdirSync(dir).find(x=>x.endsWith('.json'));
 fs.writeFileSync(path.join(dir,ref),JSON.stringify({image:'../../etc/passwd'}));assert.equal(f.api.snapshot('m',image),'');
});
