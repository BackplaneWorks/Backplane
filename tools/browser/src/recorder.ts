// The in-page recorder (see README "Recording"). Installed on every
// document with addInitScript; inert until the helper turns it on. It
// describes each element semantically and masks secrets in the page, so a
// secret's value never reaches the binding. Written as a string: it runs in
// the page, not here, and must not be touched by the compiler.

export const RECORDER_SCRIPT = String.raw`(() => {
  if (window.__bpRecInstalled) return;
  Object.defineProperty(window, "__bpRecInstalled", { value: true });
  let on = false;
  const send = (ev) => { try { const r = window.__bpRec(ev); if (r && r.catch) r.catch(() => {}); return r; } catch (e) { return null; } };
  window.__bpRecSet = (v) => { on = !!v; };
  try { const r = send({ q: "on?" }); if (r && r.then) r.then((v) => { if (v) on = true; }, () => {}); } catch (e) {}

  // words, not substrings: "footprint" or "discard" are not secrets
  const SECRET_RE = /\bcard\b|card.?(num|no\b)|cc.?(num|no\b)|\bcvv|\bcvc|\biban\b|\bssn\b|social.?security|\bpin\b|\botp\b|\b2fa\b|one.?time|security.?code|passcode/i;
  const ACTIONABLE = "a,button,input,select,textarea,label,summary,[role],[onclick],[tabindex]";
  const clip = (s, n) => String(s || "").replace(/\s+/g, " ").trim().slice(0, n);
  const lower = (s) => String(s || "").toLowerCase();

  const labelText = (el) => {
    try {
      if (el.labels && el.labels.length) return clip(Array.from(el.labels).map((l) => l.innerText || l.textContent).join(" "), 80);
    } catch (e) {}
    const by = el.getAttribute && el.getAttribute("aria-labelledby");
    if (by) {
      const t = by.split(/\s+/).map((id) => { const n = document.getElementById(id); return n ? n.innerText || n.textContent : ""; }).join(" ");
      if (clip(t, 80)) return clip(t, 80);
    }
    return "";
  };

  const isSecret = (el) => {
    if (!el || el.nodeType !== 1) return false;
    const tag = lower(el.tagName);
    if (tag !== "input" && tag !== "textarea" && tag !== "select" && !el.isContentEditable) return false;
    if (lower(el.type) === "password") return true;
    const ac = lower(el.getAttribute("autocomplete"));
    if (ac.indexOf("password") >= 0 || ac.indexOf("one-time-code") >= 0 || /(^|\s)cc-/.test(ac)) return true;
    const hay = [el.getAttribute("name"), el.id, labelText(el), el.getAttribute("placeholder"), el.getAttribute("aria-label")].join(" ");
    return SECRET_RE.test(hay);
  };

  const roleOf = (el) => {
    const r = el.getAttribute("role");
    if (r) return r.split(/\s+/)[0];
    const tag = lower(el.tagName), type = lower(el.type);
    switch (tag) {
      case "a": return el.hasAttribute("href") ? "link" : "";
      case "button": case "summary": return "button";
      case "select": return el.multiple || el.size > 1 ? "listbox" : "combobox";
      case "textarea": return "textbox";
      case "option": return "option";
      case "form": return "form";
      case "img": return "img";
      case "nav": return "navigation";
      case "dialog": return "dialog";
      case "li": return "listitem";
      case "h1": case "h2": case "h3": case "h4": case "h5": case "h6": return "heading";
      case "input":
        if (type === "checkbox" || type === "radio") return type;
        if (type === "button" || type === "submit" || type === "reset" || type === "image") return "button";
        if (type === "range") return "slider";
        if (type === "number") return "spinbutton";
        if (type === "hidden") return "";
        return "textbox";
    }
    return "";
  };

  const nameOf = (el, secret) => {
    const al = el.getAttribute("aria-label");
    if (al && clip(al, 80)) return clip(al, 80);
    const lt = labelText(el);
    if (lt) return lt;
    const tag = lower(el.tagName), type = lower(el.type);
    const alt = el.getAttribute("alt");
    if (alt) return clip(alt, 80);
    const title = el.getAttribute("title");
    if (title) return clip(title, 80);
    if (tag === "input" && (type === "button" || type === "submit" || type === "reset")) return secret ? "" : clip(el.value, 80);
    if (tag === "input" || tag === "textarea" || tag === "select") return clip(el.getAttribute("placeholder"), 80);
    if (secret) return "";
    return clip(el.innerText || el.textContent, 80);
  };

  const idStable = (id) => id && !/\d{4,}/.test(id) && !(/[0-9a-f]{8,}/i.test(id) && /\d/.test(id)) && !/^(:r|ember|react|radix|headlessui|__)/i.test(id);
  const classStable = (c) => c && c.length <= 40
    && !/^(css-|sc-|jsx-|svelte-|emotion-|styled-|_)/i.test(c)
    && !/\d{3,}/.test(c)
    && !(/^[a-z0-9]{6,}$/i.test(c) && /\d/.test(c) && /[a-z]/i.test(c) && !/-/.test(c))
    && !(/-[a-z0-9]{5,8}$/i.test(c) && /\d/.test(c.split("-").pop()) && /[a-z]/i.test(c.split("-").pop()));
  const cssOf = (el) => {
    const parts = [];
    let cur = el;
    for (let i = 0; cur && cur.nodeType === 1 && i < 4; i++, cur = cur.parentElement) {
      if (idStable(cur.id)) { parts.unshift("#" + CSS.escape(cur.id)); break; }
      let s = lower(cur.tagName);
      const cls = Array.from(cur.classList || []).filter(classStable).slice(0, 2);
      if (cls.length) s += "." + cls.map((c) => CSS.escape(c)).join(".");
      const p = cur.parentElement;
      if (p) {
        const same = Array.from(p.children).filter((c) => c.tagName === cur.tagName);
        if (same.length > 1) s += ":nth-of-type(" + (same.indexOf(cur) + 1) + ")";
      }
      parts.unshift(s);
      if (lower(cur.tagName) === "body") break;
    }
    return parts.join(" > ");
  };

  const nearOf = (el) => {
    let best = "";
    let all = [];
    try { all = document.querySelectorAll("h1,h2,h3,h4,h5,h6,[role=heading],[role=region][aria-label],[role=navigation][aria-label],[role=main][aria-label],[role=form][aria-label],[role=dialog][aria-label],nav[aria-label],section[aria-label],form[aria-label],dialog[aria-label]"); } catch (e) {}
    for (let i = 0; i < all.length && i < 3000; i++) {
      const h = all[i];
      if (h === el) continue;
      const pos = el.compareDocumentPosition(h);
      if (!(pos & 2 || pos & 8)) continue; // h precedes el, or contains it
      const isHead = /^h[1-6]$/i.test(h.tagName) || h.getAttribute("role") === "heading";
      const t = isHead ? clip(h.innerText || h.textContent, 60) : clip(h.getAttribute("aria-label"), 60);
      if (t) best = t;
    }
    return best;
  };

  const describe = (el) => {
    const secret = isSecret(el);
    const role = roleOf(el);
    const name = nameOf(el, secret);
    let nth = 0;
    try {
      const cands = document.querySelectorAll(ACTIONABLE + ",h1,h2,h3,h4,h5,h6");
      let n = 0;
      for (let i = 0; i < cands.length && i < 2000; i++) {
        const c = cands[i];
        if (c === el) { nth = n; break; }
        if (roleOf(c) === role && nameOf(c, isSecret(c)) === name) n++;
      }
    } catch (e) {}
    const tag = lower(el.tagName);
    return {
      role: role,
      name: name,
      label: labelText(el),
      placeholder: secret ? "" : clip(el.getAttribute("placeholder"), 80),
      text: secret ? "" : tag === "input" || tag === "textarea" || tag === "select" ? "" : clip(el.innerText || el.textContent, 80),
      testid: el.getAttribute("data-testid") || el.getAttribute("data-test") || el.getAttribute("data-qa") || "",
      css: cssOf(el),
      nth: nth,
      near: nearOf(el),
      tag: tag,
      type: tag === "input" ? lower(el.type) : "",
    };
  };

  const secretLabel = (el) => labelText(el) || clip(el.getAttribute("aria-label"), 80) || clip(el.getAttribute("placeholder"), 80) || clip(el.getAttribute("name") || el.id, 80) || "secret";

  const emit = (step) => { send({ step: step }); };

  const actionable = (t) => {
    if (!t || t.nodeType !== 1) t = t && t.parentElement;
    if (!t) return null;
    return t.closest(ACTIONABLE) || t;
  };

  // fields: the value last reported (or present at focus), so a field edit is one step
  const seen = new WeakMap();
  const valueOf = (el) => el.isContentEditable && lower(el.tagName) !== "input" && lower(el.tagName) !== "textarea" ? String(el.innerText || "") : String(el.value == null ? "" : el.value);
  const typable = (el) => {
    if (!el || el.nodeType !== 1) return false;
    const tag = lower(el.tagName);
    if (tag === "textarea") return true;
    if (tag === "input") return !/^(checkbox|radio|file|button|submit|reset|image|range|color|hidden)$/.test(lower(el.type));
    return !!el.isContentEditable;
  };
  const flush = (el) => {
    if (!typable(el)) return;
    const v = valueOf(el);
    if (seen.has(el) && seen.get(el) === v) return;
    if (!seen.has(el) && v === "") return;
    seen.set(el, v);
    if (isSecret(el)) emit({ do: "type", target: describe(el), secret: true, label: secretLabel(el) });
    else emit({ do: "type", target: describe(el), value: v });
  };

  let lastLabelClick = 0;
  const hook = (type, fn) => window.addEventListener(type, (e) => {
    // Playwright's selectOption fires an untrusted change; everything else must be real input
    if (!on || !(e.isTrusted || e.type === "change")) return;
    try { fn(e); } catch (err) {}
  }, true);

  hook("focusin", (e) => { const t = e.target; if (typable(t) && !seen.has(t)) seen.set(t, valueOf(t)); });
  hook("click", (e) => {
    const el = actionable(e.target);
    if (!el) return;
    if (lower(el.tagName) === "label") lastLabelClick = Date.now();
    else if (e.detail === 0 && lower(el.tagName) === "input" && Date.now() - lastLabelClick < 200) return;
    emit({ do: "click", target: describe(el) });
  });
  hook("change", (e) => {
    const t = e.target;
    if (!t || t.nodeType !== 1) return;
    if (lower(t.tagName) === "select") {
      const o = t.options && t.options[t.selectedIndex];
      if (isSecret(t)) emit({ do: "select", target: describe(t), secret: true, label: secretLabel(t) });
      else emit({ do: "select", target: describe(t), value: o ? o.value : t.value, text: o ? clip(o.text, 80) : "" });
    } else flush(t);
  });
  hook("focusout", (e) => { if (e.target && e.target.isContentEditable) flush(e.target); });
  hook("keydown", (e) => {
    const k = e.key;
    if (k !== "Enter" && k !== "Tab" && k !== "Escape") return;
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    const el = e.target && e.target.nodeType === 1 ? e.target : null;
    if (!el) return;
    if (k !== "Escape") flush(el);
    emit({ do: "press", key: k, target: describe(el) });
  });
  hook("submit", (e) => { if (e.target && e.target.nodeType === 1) emit({ do: "submit", target: describe(e.target) }); });
})()`;
