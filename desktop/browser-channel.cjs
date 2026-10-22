'use strict';

const WebSocket=require('ws');
const {decodeBrowserMessage,encodeBrowserMessage,MAX_BYTES}=require('./browser-wire.cjs');
const MAX_BACKLOG=4*1024*1024;

// Main-only transport. The controller validates envelope identity and grants;
// opening this socket confers no guest permission. There is no reconnect/replay
// queue, URL credential, renderer-visible socket or cookie authentication.
function openBrowserChannel({origin,token,onMessage,onClose,WebSocketClass=WebSocket,openTimeoutMs=5000}){
  const url=new URL(origin);
  if(url.origin!==origin||url.protocol!=='http:'||!['127.0.0.1','[::1]'].includes(url.hostname)||!url.port||url.username||url.password)throw new TypeError('Expected selected loopback Browser service origin');
  if(!/^[a-f0-9]{32}$/.test(token)||typeof onMessage!=='function'||typeof onClose!=='function')throw new TypeError('Invalid private Browser transport');
  const endpoint=new URL('/desktop/browser/v1',origin);endpoint.protocol='ws:';
  const socket=new WebSocketClass(endpoint.href,{
    headers:{Authorization:'Bearer '+token},followRedirects:false,
    handshakeTimeout:3000,maxPayload:MAX_BYTES,perMessageDeflate:false,
    skipUTF8Validation:false,
  });
  let opened=false,closed=false,resolveOpen,rejectOpen;
  // A hub that accepts the connection and never answers the upgrade must not
  // hold anyone: the open has its own deadline beside ws's handshakeTimeout.
  const openTimer=setTimeout(()=>finish('OPEN_TIMEOUT'),openTimeoutMs);
  const ready=new Promise((resolve,reject)=>{resolveOpen=resolve;rejectOpen=reject;});
  function finish(code){
    if(closed)return;closed=true;clearTimeout(openTimer);
    // Never forward library errors/URLs/response headers: these callbacks may
    // become user-facing. Grant revocation is synchronous before termination.
    const error=Object.assign(new Error('Private Browser connection unavailable'),{code});
    if(!opened)rejectOpen(error);
    try{onClose(error);}finally{socket.terminate();}
  }
  const channel=Object.freeze({
    send(value){
      if(closed||!opened||socket.readyState!==WebSocketClass.OPEN)throw Object.assign(new Error('Private Browser channel closed'),{dispatched:false});
      const source=encodeBrowserMessage(value),size=Buffer.byteLength(source);
      if(socket.bufferedAmount+size>MAX_BACKLOG){finish('BACKLOG_LIMIT');throw Object.assign(new Error('Private Browser backlog limit'),{dispatched:false});}
      // An accepted write is potentially dispatched. Its later failure must
      // retire authority and be reported UNKNOWN by the controller, not retried.
      socket.send(source,{binary:false,compress:false},error=>{if(error)finish('WRITE_FAILED');});
    },
    close(){finish('CLOSED');},
  });
  socket.once('open',()=>{if(closed)return;clearTimeout(openTimer);opened=true;resolveOpen(channel);});
  socket.on('message',(data,binary)=>{
    if(closed)return;
    if(binary){finish('BINARY_FRAME');return;}
    try{
      const result=onMessage(decodeBrowserMessage(data));
      // Synchronous lifecycle invalidation happens in onMessage before any
      // asynchronous response/dispatch work. Rejection still fails closed.
      if(result?.then)void Promise.resolve(result).catch(()=>finish('MESSAGE_REJECTED'));
    }catch{finish('MESSAGE_REJECTED');}
  });
  socket.once('unexpected-response',(_request,response)=>{response.resume();finish('UPGRADE_REFUSED');});
  socket.on('error',()=>finish('SOCKET_ERROR'));
  socket.once('close',()=>finish('DISCONNECTED'));
  return ready;
}
module.exports={openBrowserChannel,MAX_BACKLOG};
