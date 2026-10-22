// The composer popup in thin mode: a read (view.update) carries the selected
// draft only while the popup may show or must shut, and the hub renders the
// popup from it. bun test/tools/thin_draft_update_test.ts
import { requestSession, ThinHost } from "../../src/web/thin-host.js";
import { popupDraft, popupReadDraft } from "../../src/web/thin.js";
import { readFileSync } from "node:fs";
function check(name: string, ok: boolean) { if (!ok) throw new Error(name); console.log(`ok ${name}`); }
const session = { sel: "t1", rid: 1, drafts: { t1: "%al", t2: "other", "@lang": "en" }, open: {}, info: {} };
const target = { kind: "thread", id: "t1" };
check("a popup read carries the selected draft", requestSession(session, false, target, true).drafts.t1 === "%al");
check("a plain read carries no draft", !("t1" in requestSession(session, false, target).drafts));
check("an action carries it whole", requestSession(session, true, target).drafts.t1 === "%al");
check("no other thread's draft", !("t2" in requestSession(session, false, target, true).drafts));
check("a bot route's @ key is never sent as a draft", !("@bot" in requestSession({ ...session, sel: "@bot", drafts: { "@bot": "%x" } }, false, target, true).drafts));
const long = "x ".repeat(3000) + "%be";
check("a long draft's read keeps only its last word", popupReadDraft(long) === " %be");
check("a long read stays small", JSON.stringify(requestSession({ ...session, drafts: { t1: long } }, false, target, true)).length < 4096);
for (const t of ["@", ">", "%", "#", "$", "/", "hello %pr", "line\n>th", "/compact"]) check(`popupDraft(${JSON.stringify(t)})`, popupDraft(t));
for (const t of ["", "hello", "a/b", "hello /x", "50%", "%a done "]) check(`not popupDraft(${JSON.stringify(t)})`, !popupDraft(t));
// the web host: reads carry the draft while it may pop up, and once more after
const h = Object.create(ThinHost.prototype);
h.session = { sel: "t1", drafts: { t1: "hel" } };
check("plain text: no draft on the read", h.popupRead() === false);
h.session.drafts.t1 = "%a"; check("typing a reference: the draft goes", h.popupRead() === true);
h.session.drafts.t1 = "%a "; check("the word ended: once more, to shut the popup", h.popupRead() === true);
check("then no more", h.popupRead() === false);
const phone = readFileSync("src/mobile/thin-native-host-307265.js", "utf8");
check("phone host asks again while the popup may open or shut", phone.includes("if (popupDraft(value) || s.popupSent) this.update(s);"));
check("phone reads carry the popup draft", phone.includes("this.requestSession(s.session, 'view.update', s.target, this.popupRead(s))"));
