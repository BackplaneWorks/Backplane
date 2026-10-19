// Browser frames are pixels. A real editable bridge supplies committed text,
// IME and software-keyboard events; navigation keys use the remote keyboard.
export function installBrowserInput(doc, dispatch, currentOwner) {
  const input = doc.createElement("textarea");
  input.setAttribute("aria-label", "Type into the remote browser");
  input.tabIndex = -1;
  input.autocomplete = "off";
  input.autocapitalize = "off";
  input.spellcheck = false;
  input.style.cssText = "position:fixed;width:1px;height:1px;opacity:0;bottom:0;left:0;font-size:16px;pointer-events:none";
  doc.body.append(input);
  let frame = null, owner = "", composing = false;
  const reset = () => { input.value = "\u200b"; input.setSelectionRange(1, 1); };
  const allowed = () => frame?.isConnected && frame.hasAttribute("data-keyto") && frame.hasAttribute("data-type") && frame.getAttribute("data-owner") === owner && currentOwner() === owner;
  const send = (attr, value) => { if (allowed()) dispatch(frame.getAttribute(attr), value); };
  const clear = () => { composing = false; reset(); };
  const editKeys = { deleteContentBackward: "Backspace", deleteContentForward: "Delete",
    deleteWordBackward: "ControlOrMeta+Backspace", insertLineBreak: "Enter", insertParagraph: "Enter",
    historyUndo: "ControlOrMeta+z", historyRedo: "ControlOrMeta+Shift+z" };
  const commit = (event) => {
    if (composing) return;
    // Some software keyboards emit non-cancelable beforeinput events.
    if (editKeys[event?.inputType]) { send("data-keyto", editKeys[event.inputType]); reset(); return; }
    // Remove only our leading sentinel; preserve characters the user supplied.
    const text = input.value.startsWith("\u200b") ? input.value.slice(1) : input.value;
    if (text) send("data-type", text);
    reset();
  };
  const focus = (el) => {
    const next = el.getAttribute("data-owner");
    if (!next || currentOwner() !== next) return false;
    if (frame !== el || owner !== next) clear();
    frame = el; owner = next;
    input.focus({ preventScroll: true });
    return true;
  };
  reset();
  doc.addEventListener("click", (e) => {
    const el = e.target.closest?.("[data-pt]");
    if (!el || e.button !== 0 || currentOwner() !== el.getAttribute("data-owner")) return;
    e.preventDefault();
    // Commit to the old field before moving the remote caret.
    if (!composing) commit();
    const r = el.getBoundingClientRect();
    const th = (v, size) => Math.max(0, Math.min(1000, Math.round(v / Math.max(size, 1) * 1000)));
    frame = el; owner = el.getAttribute("data-owner");
    send("data-pt", `${th(e.clientX - r.left, r.width)} ${th(e.clientY - r.top, r.height)}`);
    clear();
    focus(el); // synchronous with the tap, required by iOS's keyboard
  }, true);
  doc.addEventListener("focusin", (e) => {
    if (e.target.matches?.("[data-keyto]")) focus(e.target);
  });
  doc.addEventListener("wheel", (e) => {
    const el = e.target.closest?.("[data-wheel]");
    if (!el || currentOwner() !== el.getAttribute("data-owner")) return;
    e.preventDefault();
    dispatch(el.getAttribute("data-wheel"), String(Math.round(Math.max(-600, Math.min(600, e.deltaY)))));
  }, { passive: false });
  input.addEventListener("compositionstart", () => { composing = true; });
  input.addEventListener("compositionend", () => { composing = false; commit(); });
  input.addEventListener("input", commit);
  input.addEventListener("blur", clear);
  input.addEventListener("paste", (e) => {
    if (!allowed()) return;
    e.preventDefault();
    send("data-type", e.clipboardData.getData("text/plain"));
    clear();
  });
  input.addEventListener("beforeinput", (e) => {
    if (!allowed()) { e.preventDefault(); clear(); return; }
    if (composing || e.isComposing) return;
    const key = editKeys[e.inputType];
    if (key && e.cancelable) { e.preventDefault(); send("data-keyto", key); reset(); }
  });
  const named = new Set(["Enter", "Backspace", "Delete", "Tab", "Escape", "ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"]);
  doc.addEventListener("keydown", (e) => {
    if (e.target !== input || !allowed()) return;
    // Keep Backplane's shortcuts from stealing keys while this bridge owns focus.
    e.stopImmediatePropagation();
    if (composing || e.isComposing || e.key === "Dead" || e.key === "Process" || e.keyCode === 229) return;
    const altGraph = e.getModifierState?.("AltGraph") || (e.ctrlKey && e.altKey && !e.metaKey);
    const primary = (e.metaKey || e.ctrlKey) && !altGraph;
    if (primary && e.key.toLowerCase() === "v") return; // local paste supplies the text
    // Physical ASCII keys also deliver keydown/keyup to sites with key handlers.
    // Non-ASCII, dead keys and AltGraph are committed by the editable bridge.
    if (!primary && !e.altKey && e.key.length === 1 && e.key.charCodeAt(0) >= 32 && e.key.charCodeAt(0) < 127) {
      e.preventDefault(); send("data-keyto", (e.shiftKey ? "Shift+" : "") + (e.key === " " ? "Space" : e.key)); reset(); return;
    }
    if (!named.has(e.key) && !(primary && e.key.length === 1)) return;
    e.preventDefault();
    const mods = (primary ? "ControlOrMeta+" : "") + (e.altKey && !altGraph ? "Alt+" : "") + (e.shiftKey ? "Shift+" : "");
    send("data-keyto", mods + (e.key === " " ? "Space" : e.key));
    reset();
  }, true);
  // A frame refresh keeps focus; changing owner, hiding the viewer or giving
  // control back invalidates it. Never replay input after a reconnect/switch.
  const observer = new MutationObserver(() => {
    if (frame && !allowed()) { input.blur(); frame = null; owner = ""; clear(); }
  });
  observer.observe(doc.getElementById("root") || doc.body, { childList: true, subtree: true, attributes: true, attributeFilter: ["data-owner", "data-keyto", "data-type"] });
  return { input };
}
