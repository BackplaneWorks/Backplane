const {test}=require('node:test'),assert=require('node:assert/strict'),vm=require('node:vm'),fs=require('node:fs'),path=require('node:path');
test('private fixture fence rejects external URL, Request, sockets and WebSockets',async()=>{
 const mods={'node:net':{connect:()=>true,createConnection:()=>true},'node:http':{request:()=>true,get:()=>true},'node:https':{request:()=>true,get:()=>true}};
 const context=vm.createContext({URL,fetch:async()=>true,Bun:{connect:()=>true},WebSocket:class{},require:n=>mods[n]});
 vm.runInContext(fs.readFileSync(path.join(__dirname,'real_store_fence.cjs'),'utf8'),context);
 for(const expr of ['fetch("https://example.invalid/")','fetch({url:"https://example.invalid/"})','fetch(new URL("https://example.invalid/"))'])await assert.rejects(vm.runInContext(expr,context));
 for(const expr of ['Bun.connect({hostname:"example.invalid",port:443})','Bun.connect({unix:"/tmp/socket"})','new WebSocket("wss://example.invalid/")','require("node:net").connect({host:"example.invalid",port:443})','require("node:http").request("https://example.invalid/")'])assert.throws(()=>vm.runInContext(expr,context));
 assert.equal(await vm.runInContext('fetch("http://127.0.0.1:9000/")',context),true);
 assert.equal(vm.runInContext('Bun.connect({hostname:"localhost",port:9000})',context),true);
});
