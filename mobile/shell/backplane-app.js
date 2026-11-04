// window.backplaneApp, bridge 1 (docs/phone-app-shell.md): what the phone
// apps give the hub's page. Injected by both apps at document start into
// the hub's origin only, after the app has put its facts in place of the
// info placeholder below ({platform, version, build, features, hub,
// textScale}; it occurs once in this file). The page
// posts {id?, op, args} as JSON; the app answers {id, ok, value|error} and
// sends events {event, data}.
// - Android: WebViewCompat.addWebMessageListener's "BackplaneNative" object
//   (postMessage; answers arrive as its onmessage)
// - iOS: webkit.messageHandlers.backplane (answers through
//   evaluateJavaScript("window.backplaneApp._receive(<json>)"))
(function () {
  "use strict";
  if (window.backplaneApp) return;
  var info = /*@info*/null || {};
  var native = window.BackplaneNative;
  var ios = window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.backplane;
  if (!native && !ios) return;
  var seq = 0, waiting = {}, listeners = {}, kept = {};
  function post(msg) {
    var text = JSON.stringify(msg);
    if (native) native.postMessage(text); else ios.postMessage(text);
  }
  function ask(op, args) {
    return new Promise(function (resolve, reject) {
      var id = ++seq;
      waiting[id] = { resolve: resolve, reject: reject };
      try { post({ id: id, op: op, args: args === undefined ? null : args }); }
      catch (e) { delete waiting[id]; reject(e); }
    });
  }
  function tell(op, args) { try { post({ op: op, args: args === undefined ? null : args }); } catch (e) {} }
  function receive(msg) {
    if (typeof msg === "string") { try { msg = JSON.parse(msg); } catch (e) { return; } }
    if (!msg) return;
    if (msg.id && waiting[msg.id]) {
      var w = waiting[msg.id];
      delete waiting[msg.id];
      if (msg.ok) w.resolve(msg.value); else w.reject(new Error(msg.error || "failed"));
      return;
    }
    if (msg.event) {
      var fns = listeners[msg.event];
      if (fns && fns.length) fns.slice().forEach(function (fn) { try { fn(msg.data || {}); } catch (e) { setTimeout(function () { throw e; }); } });
      else (kept[msg.event] = kept[msg.event] || []).push(msg.data || {});
    }
  }
  if (native) native.onmessage = function (e) { receive(e.data); };
  var app = {
    bridge: 1,
    platform: info.platform || (native ? "android" : "ios"),
    version: info.version || "",
    build: info.build || "",
    features: info.features || [],
    hub: info.hub || null,
    textScale: info.textScale || 1,
    requestCamera: function () { return ask("requestCamera").then(Boolean, function () { return false; }); },
    setState: function (s) { tell("setState", s || {}); },
    share: function (s) { return ask("share", s || {}).then(Boolean, function () { return false; }); },
    openExternal: function (url) { return ask("openExternal", { url: String(url) }).then(Boolean, function () { return false; }); },
    haptic: function (kind) { tell("haptic", { kind: kind || "light" }); },
    hubs: function () { return ask("hubs"); },
    switchHub: function (key) { return ask("switchHub", { key: String(key) }).then(Boolean, function () { return false; }); },
    addHub: function () { tell("addHub"); },
    openAppSettings: function () { tell("openAppSettings"); },
    notifications: function () { return ask("notifications"); },
    requestNotifications: function () { return ask("requestNotifications"); },
    notify: function (n) { tell("notify", n || {}); },
    pairingFailed: function (cause) { tell("pairingFailed", { cause: String(cause || "") }); },
    on: function (type, fn) {
      (listeners[type] = listeners[type] || []).push(fn);
      var early = kept[type];
      if (early) { delete kept[type]; early.forEach(function (d) { setTimeout(function () { fn(d); }); }); }
      return function () { var l = listeners[type] || [], i = l.indexOf(fn); if (i >= 0) l.splice(i, 1); };
    },
    _receive: receive,
  };
  Object.defineProperty(window, "backplaneApp", { value: Object.freeze(app), configurable: false, writable: false });
  tell("hello", { url: location.href });
})();
