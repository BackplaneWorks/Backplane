// Native X11 CEF children controlled over stdin. See README.md for the protocol.
#include <X11/Xlib.h>
#include <X11/extensions/XI2.h>
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <atomic>
#include <iostream>
#include <map>
#include <set>
#include <sstream>
#include <string>
#include <thread>

#include "include/cef_app.h"
#include "include/cef_browser.h"
#include "include/cef_client.h"
#include "include/cef_command_line.h"
#include "include/cef_parser.h"
#include "include/cef_devtools_message_observer.h"
#include "include/wrapper/cef_closure_task.h"
#include "include/wrapper/cef_helpers.h"
#include "include/base/cef_callback.h"

namespace {
double now_ms() {
  timespec ts; clock_gettime(CLOCK_REALTIME, &ts);
  return ts.tv_sec * 1000.0 + ts.tv_nsec / 1e6;
}
std::string js(const std::string& s) {
  std::string o = "\"";
  for (unsigned char c : s) {
    if (c == '"' || c == '\\') { o += '\\'; o += c; }
    else if (c < 32) { char b[8]; snprintf(b, sizeof b, "\\u%04x", c); o += b; }
    else o += c;
  }
  return o + "\"";
}
void emit(const std::string& body) {
  printf("{%s,\"t\":%.1f}\n", body.c_str(), now_ms()); fflush(stdout);
}
struct Opts {
  unsigned long parent = 0;
  std::string url, cache;
  int w = 1280, h = 800, x = 0, y = 0, port = 0;
} opts;
Display* dpy = nullptr;
Window parent = 0, shield = 0;
int xi_opcode = 0;
bool shield_on = false;
std::string shown;
std::atomic<bool> quitting{false};
struct Entry { CefRefPtr<CefBrowser> browser; int x = 0, y = 0, w = 1280, h = 800; };
std::map<std::string, Entry> pages;
std::set<std::string> reserved;
unsigned long popup_n = 0;
int pending_browsers = 0;
std::map<std::pair<std::string, int>, std::string> pending_popups;

std::string fields(const std::string& key) { return ",\"key\":" + js(key); }
void shield_place() {
  auto it = pages.find(shown);
  if (!shield_on || it == pages.end()) XUnmapWindow(dpy, shield);
  else {
    auto& e = it->second;
    XMoveResizeWindow(dpy, shield, e.x, e.y, e.w, e.h);
    XMapRaised(dpy, shield);
  }
  XFlush(dpy);
}
void hide_all() {
  for (auto& [k, e] : pages) XUnmapWindow(dpy, e.browser->GetHost()->GetWindowHandle());
  shown.clear(); shield_place();
}
void focus(CefRefPtr<CefBrowser> b, const std::string& key) {
  if (shown != key) return; // XSetInputFocus requires a viewable window.
  XWindowAttributes a;
  if (!XGetWindowAttributes(dpy, b->GetHost()->GetWindowHandle(), &a) || a.map_state != IsViewable) return;
  XSetInputFocus(dpy, b->GetHost()->GetWindowHandle(), RevertToParent, CurrentTime);
  XFlush(dpy);
}

class Client : public CefClient, public CefLifeSpanHandler, public CefLoadHandler,
               public CefDisplayHandler, public CefFocusHandler, public CefRequestHandler,
               public CefDevToolsMessageObserver {
 public:
  std::string key, opener, popup_url, why = "page", command;
  CefRefPtr<CefRegistration> observer;
  int initial_x, initial_y;
  Client(std::string k, std::string o = "", std::string u = "", std::string c = "", int x = 0, int y = 0)
      : key(k), opener(o), popup_url(u), command(c), initial_x(x), initial_y(y) {}
  CefRefPtr<CefLifeSpanHandler> GetLifeSpanHandler() override { return this; }
  CefRefPtr<CefLoadHandler> GetLoadHandler() override { return this; }
  CefRefPtr<CefDisplayHandler> GetDisplayHandler() override { return this; }
  CefRefPtr<CefFocusHandler> GetFocusHandler() override { return this; }
  CefRefPtr<CefRequestHandler> GetRequestHandler() override { return this; }
  bool OnBeforePopup(CefRefPtr<CefBrowser>, CefRefPtr<CefFrame>, int popup_id,
      const CefString& url, const CefString&, WindowOpenDisposition, bool,
      const CefPopupFeatures&, CefWindowInfo& wi, CefRefPtr<CefClient>& client,
      CefBrowserSettings&, CefRefPtr<CefDictionaryValue>&, bool*) override {
    if (quitting) return true;
    std::string k = key + "~" + std::to_string(++popup_n);
    reserved.insert(k); ++pending_browsers;
    pending_popups[{key, popup_id}] = k;
    wi.SetAsChild(parent, CefRect(0, 0, opts.w, opts.h));
    wi.runtime_style = CEF_RUNTIME_STYLE_ALLOY;
    client = new Client(k, key, url.ToString());
    return false;
  }
  void OnBeforePopupAborted(CefRefPtr<CefBrowser>, int id) override {
    auto it = pending_popups.find({key, id});
    if (it != pending_popups.end()) {
      reserved.erase(it->second); pending_popups.erase(it); --pending_browsers;
      if (quitting && pages.empty() && pending_browsers == 0) CefQuitMessageLoop();
    }
  }
  void OnAfterCreated(CefRefPtr<CefBrowser> b) override {
    CEF_REQUIRE_UI_THREAD();
    --pending_browsers;
    for (auto it = pending_popups.begin(); it != pending_popups.end();) {
      if (it->second == key) it = pending_popups.erase(it); else ++it;
    }
    pages[key] = {b, initial_x, initial_y, opts.w, opts.h};
    XUnmapWindow(dpy, b->GetHost()->GetWindowHandle()); XFlush(dpy);
    observer = b->GetHost()->AddDevToolsMessageObserver(this);
    b->GetHost()->ExecuteDevToolsMethod(1, "Target.getTargetInfo", nullptr);
    if (quitting) b->GetHost()->CloseBrowser(true);
  }
  void OnDevToolsMethodResult(CefRefPtr<CefBrowser> b, int id, bool success,
                             const void* data, size_t len) override {
    if (id != 1) return;
    auto v = CefParseJSON(std::string(static_cast<const char*>(data), len), JSON_PARSER_RFC);
    if (!success || !v || v->GetType() != VTYPE_DICTIONARY) {
      emit("\"event\":\"error\",\"cmd\":\"create\"" + fields(key) + ",\"error\":\"no CDP target id\"");
      return;
    }
    auto ti = v->GetDictionary()->GetDictionary("targetInfo");
    if (!ti) return;
    std::string extra = ",\"xid\":" + std::to_string(b->GetHost()->GetWindowHandle()) +
                        ",\"target\":" + js(ti->GetString("targetId").ToString());
    if (opener.empty()) {
      emit("\"event\":\"created\",\"cmd\":" + js(command) + fields(key) + extra);
    } else {
      emit("\"event\":\"popup\"" + fields(key) + ",\"opener\":" + js(opener) + ",\"url\":" + js(popup_url) + extra);
    }
  }
  void OnBeforeClose(CefRefPtr<CefBrowser>) override {
    observer = nullptr;
    pages.erase(key); reserved.erase(key);
    if (shown == key) { shown.clear(); shield_place(); }
    emit("\"event\":\"closed\"" + fields(key) + ",\"why\":" + js(why) +
         (why == "command" ? ",\"cmd\":\"close\"" : ""));
    if (quitting && pages.empty() && pending_browsers == 0) CefQuitMessageLoop();
  }
  void OnLoadEnd(CefRefPtr<CefBrowser>, CefRefPtr<CefFrame> f, int code) override {
    if (f->IsMain()) emit("\"event\":\"load\"" + fields(key) + ",\"url\":" + js(f->GetURL().ToString()) + ",\"code\":" + std::to_string(code));
  }
  void OnAddressChange(CefRefPtr<CefBrowser>, CefRefPtr<CefFrame> f, const CefString& url) override {
    if (f->IsMain()) emit("\"event\":\"navigated\"" + fields(key) + ",\"url\":" + js(url.ToString()));
  }
  void OnTitleChange(CefRefPtr<CefBrowser>, const CefString& title) override {
    emit("\"event\":\"title\"" + fields(key) + ",\"title\":" + js(title.ToString()));
  }
  bool OnSetFocus(CefRefPtr<CefBrowser> b, FocusSource) override {
    focus(b, key); return false;
  }
  void OnGotFocus(CefRefPtr<CefBrowser> b) override {
    focus(b, key); emit("\"event\":\"focus\"" + fields(key));
  }
  void OnRenderProcessTerminated(CefRefPtr<CefBrowser>, TerminationStatus status, int, const CefString&) override {
    why = "other";
    emit("\"event\":\"crashed\"" + fields(key) + ",\"status\":" + std::to_string(status));
  }
 private:
  IMPLEMENT_REFCOUNTING(Client);
};

void create(const std::string& key, const std::string& url, const std::string& cmd, int x = 0, int y = 0) {
  reserved.insert(key); ++pending_browsers;
  CefWindowInfo wi;
  wi.SetAsChild(parent, CefRect(x, y, opts.w, opts.h));
  wi.runtime_style = CEF_RUNTIME_STYLE_ALLOY;
  CefBrowserSettings bs;
  if (!CefBrowserHost::CreateBrowser(wi, new Client(key, "", "", cmd, x, y), url, bs, nullptr, nullptr)) {
    reserved.erase(key); --pending_browsers;
    emit("\"event\":\"error\",\"cmd\":" + js(cmd) + fields(key) + ",\"error\":\"create failed\"");
  }
}

bool valid_key(const std::string& k) {
  return !k.empty() && k.size() <= 64 && k.find_first_not_of("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-") == std::string::npos;
}
void run_command(std::string line) {
  CEF_REQUIRE_UI_THREAD();
  std::istringstream s(line);
  std::string cmd, key, tail; s >> cmd;
  auto error = [&]() { emit("\"event\":\"error\",\"cmd\":" + js(cmd) + fields(key) + ",\"error\":\"unknown or malformed command\""); };
  auto end = [&]() { s >> std::ws; return s.eof(); };
  if (cmd == "quit" && end()) {
    if (quitting.exchange(true)) return;
    for (auto& [k, e] : pages) {
      static_cast<Client*>(e.browser->GetHost()->GetClient().get())->why = "other";
      e.browser->GetHost()->CloseBrowser(true);
    }
    if (pages.empty() && pending_browsers == 0) CefQuitMessageLoop();
    return;
  }
  if (quitting) { error(); return; }
  if (cmd == "create") {
    s >> key >> std::ws; std::getline(s, tail);
    if (!valid_key(key) || tail.empty() || reserved.count(key)) { error(); return; }
    create(key, tail, cmd); return;
  }
  if (cmd == "hide" && end()) hide_all();
  else if (cmd == "shield") {
    int on = -1;
    if (!(s >> on) || (on != 0 && on != 1) || !end()) { error(); return; }
    shield_on = on; shield_place();
  } else {
    s >> key;
    auto it = pages.find(key);
    if (it == pages.end()) { error(); return; }
    auto& e = it->second;
    Window win = e.browser->GetHost()->GetWindowHandle();
    if (cmd == "show" || cmd == "size") {
      int x = e.x, y = e.y, w, h;
      if (cmd == "show" && !(s >> x >> y)) { error(); return; }
      if (!(s >> w >> h) || w <= 0 || h <= 0 || !end()) { error(); return; }
      if (cmd == "show") hide_all();
      e.x = x; e.y = y; e.w = w; e.h = h;
      XMoveResizeWindow(dpy, win, x, y, w, h);
      e.browser->GetHost()->WasResized();
      if (cmd == "show") { shown = key; XMapRaised(dpy, win); }
      shield_place();
    } else if (cmd == "focus" && end()) {
      e.browser->GetHost()->SetFocus(true); focus(e.browser, key); shield_place();
    } else if (cmd == "close" && end()) {
      auto c = static_cast<Client*>(e.browser->GetHost()->GetClient().get());
      c->why = "command"; e.browser->GetHost()->CloseBrowser(true); return;
    } else { error(); return; }
  }
  XSync(dpy, False);
  emit("\"event\":\"ok\",\"cmd\":" + js(cmd) + fields(key));
}
void stdin_loop() {
  std::string line;
  while (std::getline(std::cin, line)) CefPostTask(TID_UI, base::BindOnce(&run_command, line));
  CefPostTask(TID_UI, base::BindOnce(&run_command, std::string("quit")));
}
void shield_event(int x, int y) {
  if (!shown.empty() && shield_on) emit("\"event\":\"shield-click\"" + fields(shown) + ",\"x\":" + std::to_string(x) + ",\"y\":" + std::to_string(y));
}
// Observe raw button presses without selecting/stealing the browser's core
// events. CDP focus emulation can suppress CEF's normal focus callbacks even
// after a WM returns X focus to the embedding top level.
void native_click(Window child) {
  auto it = pages.find(shown);
  if (it == pages.end() || child != it->second.browser->GetHost()->GetWindowHandle()) return;
  it->second.browser->GetHost()->SetFocus(true);
  focus(it->second.browser, shown);
  shield_place();
  emit("\"event\":\"focus\"" + fields(shown));
}
bool watch_clicks() {
  struct EventMask { int deviceid, mask_len; unsigned char* mask; };
  using Query = int (*)(Display*, int*, int*);
  using Select = int (*)(Display*, Window, EventMask*, int);
  void* xi = dlopen("libXi.so.6", RTLD_NOW);
  int event, error, major = 2, minor = 0;
  if (!xi || !XQueryExtension(dpy, "XInputExtension", &xi_opcode, &event, &error)) return false;
  auto query = reinterpret_cast<Query>(dlsym(xi, "XIQueryVersion"));
  auto select = reinterpret_cast<Select>(dlsym(xi, "XISelectEvents"));
  if (!query || !select || query(dpy, &major, &minor) != Success) return false;
  unsigned char bits[2] = {0, 1 << (XI_RawButtonPress - 8)};
  EventMask mask{XIAllMasterDevices, 2, bits};
  return select(dpy, DefaultRootWindow(dpy), &mask, 1) == Success;
}
void x_loop() {
  while (!quitting) {
    XEvent ev; XNextEvent(dpy, &ev);
    if (ev.type == ButtonPress && ev.xbutton.window == shield)
      CefPostTask(TID_UI, base::BindOnce(&shield_event, ev.xbutton.x, ev.xbutton.y));
    if (ev.type == GenericEvent && ev.xcookie.extension == xi_opcode && ev.xcookie.evtype == XI_RawButtonPress) {
      Window root, child; int rx, ry, x, y; unsigned mask;
      if (XQueryPointer(dpy, parent, &root, &child, &rx, &ry, &x, &y, &mask))
        CefPostTask(TID_UI, base::BindOnce(&native_click, child));
    }
  }
}
class App : public CefApp, public CefBrowserProcessHandler {
 public:
  CefRefPtr<CefBrowserProcessHandler> GetBrowserProcessHandler() override { return this; }
  void OnBeforeCommandLineProcessing(const CefString&, CefRefPtr<CefCommandLine> cl) override {
    cl->AppendSwitchWithValue("password-store", "basic"); cl->AppendSwitch("use-mock-keychain");
    if (getenv("BACKPLANE_CEF_NO_GPU")) cl->AppendSwitch("disable-gpu");
  }
  void OnContextInitialized() override {
    if (!opts.url.empty()) create("main", opts.url, "create", opts.x, opts.y);
  }
 private:
  IMPLEMENT_REFCOUNTING(App);
};
int x_error(Display*, XErrorEvent* e) {
  fprintf(stderr, "backplane-cef: X error %d (request %d)\n", e->error_code, e->request_code); return 0;
}
void parse(int argc, char** argv) {
  for (int i = 1; i + 1 < argc; i++) {
    std::string a = argv[i]; const char* v = argv[i+1];
    if (a == "--parent") opts.parent = strtoul(v, nullptr, 0), i++;
    else if (a == "--url") opts.url = v, i++;
    else if (a == "--size") sscanf(v, "%dx%d", &opts.w, &opts.h), i++;
    else if (a == "--at") sscanf(v, "%dx%d", &opts.x, &opts.y), i++;
    else if (a == "--cache") opts.cache = v, i++;
    else if (a == "--port") opts.port = atoi(v), i++;
  }
}
} // namespace

// The zygote changes the stack canary after fork: no protected frame across it.
__attribute__((no_stack_protector)) int main(int argc, char** argv) {
  double t0 = now_ms();
  CefMainArgs args(argc, argv);
  CefRefPtr<App> app = new App;
  int code = CefExecuteProcess(args, app, nullptr);
  if (code >= 0) return code;
  parse(argc, argv);
  XInitThreads(); dpy = XOpenDisplay(nullptr);
  if (!dpy) { fprintf(stderr, "backplane-cef: no X display\n"); return 1; }
  XSetErrorHandler(x_error);
  parent = opts.parent;
  if (!parent) {
    parent = XCreateSimpleWindow(dpy, DefaultRootWindow(dpy), 0, 0, opts.w, opts.h, 0, 0, 0xffffff);
    XStoreName(dpy, parent, "backplane-cef test"); XMapWindow(dpy, parent);
  }
  XSetWindowAttributes attr{};
  attr.event_mask = ButtonPressMask | ButtonReleaseMask | PointerMotionMask;
  shield = XCreateWindow(dpy, parent, 0, 0, 1, 1, 0, 0, InputOnly, CopyFromParent, CWEventMask, &attr);
  if (!watch_clicks()) { fprintf(stderr, "backplane-cef: XInput2 unavailable\n"); return 1; }
  XSync(dpy, False);
  CefSettings st;
  // UI strings: en-US (the only locale packaged); pages get the person's
  // languages from $LANG (ja_JP.UTF-8 -> ja-JP,ja,en-US,en)
  CefString(&st.locale) = "en-US";
  if (const char* lang = getenv("LANG")) {
    std::string l = lang;
    l = l.substr(0, l.find_first_of(".@"));
    if (l.size() >= 2 && l != "C" && l != "POSIX") {
      for (auto& c : l) if (c == '_') c = '-';
      std::string base = l.substr(0, l.find('-'));
      std::string list = l + (base != l ? "," + base : "") + (base != "en" ? ",en-US,en" : "");
      CefString(&st.accept_language_list) = list;
    }
  }
  st.no_sandbox = true; st.remote_debugging_port = opts.port; st.log_severity = LOGSEVERITY_WARNING;
  if (!opts.cache.empty()) {
    CefString(&st.root_cache_path) = opts.cache;
    CefString(&st.cache_path) = opts.cache + "/Default";
  }
  if (!CefInitialize(args, st, app, nullptr)) return 1;
  emit("\"event\":\"init\",\"ms\":" + std::to_string(now_ms() - t0));
  std::thread(stdin_loop).detach(); std::thread(x_loop).detach();
  CefRunMessageLoop();
  quitting = true;
  CefShutdown();
  // x_loop is parked on this connection until process exit; do not close it underneath XNextEvent.
  return 0;
}
