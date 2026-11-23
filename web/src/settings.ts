// Grouped, searchable Web Settings modal:
// General (Appearance: Theme & Language), Agents and tools (AI providers),
// Connect (Devices & browser, Home machine).
import { t, trs } from "./i18n";
import type { NoProviderNotice } from "./provider-notice";
import {
  SETTINGS_GROUPS,
  SETTINGS_PAGES,
  filterSearchItems,
  filterSettingsNav,
  resolveTheme,
  type SettingsPageId,
  type SettingsPageMeta,
  type SettingsSearchItem,
  type WebLocalePref,
  type WebThemePref,
} from "./settings-model";
import type { HomeInfo } from "./ws";

export interface SettingsRowButton {
  label: string;
  action: string;
  value: string;
  primary?: boolean;
  danger?: boolean;
}

export interface SettingsRow {
  title?: string;
  head?: string;
  text?: string;
  detail?: string;
  buttons?: SettingsRowButton[];
}

export interface SettingsCallbacks {
  onLoadDevices: () => Promise<SettingsRow[]>;
  onDevicesAct: (action: string, value: string) => Promise<void>;
  onLogout: () => Promise<void>;
  onGetHome?: () => HomeInfo | null;
  onMoveHomeHere?: () => Promise<void>;
  onGetNoProvider?: () => NoProviderNotice | null;
  onGetThemePref?: () => WebThemePref;
  onSetThemePref?: (pref: WebThemePref) => void;
  onGetHubTheme?: () => "dark" | "light";
  onGetLocalePref?: () => WebLocalePref;
  onSetLocalePref?: (pref: WebLocalePref) => void;
}

function makeNavIcon(kind: SettingsPageMeta["icon"]): SVGSVGElement {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", "15");
  svg.setAttribute("height", "15");
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "1.75");
  svg.setAttribute("stroke-linecap", "square");
  svg.setAttribute("stroke-linejoin", "miter");
  svg.setAttribute("aria-hidden", "true");

  const add = (tag: string, attrs: Record<string, string>) => {
    const el = document.createElementNS(ns, tag);
    for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, v);
    svg.appendChild(el);
  };

  switch (kind) {
    case "sun":
      add("circle", { cx: "12", cy: "12", r: "4" });
      add("path", { d: "M12 2v2m0 16v2M4.93 4.93l1.41 1.41m11.32 11.32 1.41 1.41M2 12h2m16 0h2M4.93 19.07l1.41-1.41m11.32-11.32 1.41-1.41" });
      break;
    case "cpu":
      add("rect", { x: "5", y: "5", width: "14", height: "14" });
      add("rect", { x: "9", y: "9", width: "6", height: "6" });
      add("path", { d: "M9 2v3m6-3v3M9 19v3m6-3v3M2 9h3m-3 6h3M19 9h3m-3 6h3" });
      break;
    case "link":
      add("path", { d: "M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" });
      add("path", { d: "M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" });
      break;
    case "globe":
      add("circle", { cx: "12", cy: "12", r: "10" });
      add("path", { d: "M2 12h20M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" });
      break;
  }
  return svg;
}

function makeSearchIcon(): SVGSVGElement {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", "13");
  svg.setAttribute("height", "13");
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "1.75");
  svg.setAttribute("aria-hidden", "true");
  const c = document.createElementNS(ns, "circle");
  c.setAttribute("cx", "11");
  c.setAttribute("cy", "11");
  c.setAttribute("r", "7");
  const p = document.createElementNS(ns, "path");
  p.setAttribute("d", "m20 20-3.5-3.5");
  svg.append(c, p);
  return svg;
}

export class SettingsModal {
  private backdrop: HTMLElement;
  private titleEl: HTMLElement;
  private badgeEl: HTMLElement;
  private closeBtn: HTMLButtonElement;
  private navListEl: HTMLElement;
  private searchInput: HTMLInputElement;
  private searchClearBtn: HTMLButtonElement;
  private paneEl: HTMLElement;
  private cb: SettingsCallbacks;

  private openState = false;
  private activePage: SettingsPageId = "appearance";
  private forceProviders = false;
  private query = "";
  private searchBrowseAll = false;

  private devicesLoaded = false;
  private devicesLoading = false;
  private devicesRows: SettingsRow[] = [];
  private devicesError = "";
  private loadGen = 0;

  constructor(root: HTMLElement, cb: SettingsCallbacks) {
    this.cb = cb;

    this.backdrop = document.createElement("div");
    this.backdrop.className = "bp-modal-backdrop bp-hidden";
    this.backdrop.setAttribute("data-testid", "settings-modal");

    const panel = document.createElement("div");
    panel.className = "bp-modal bp-settings-modal";
    panel.setAttribute("role", "dialog");
    panel.setAttribute("aria-modal", "true");
    panel.setAttribute("aria-labelledby", "bp-settings-title");

    const head = document.createElement("div");
    head.className = "bp-modal-head";
    const headTitle = document.createElement("div");
    headTitle.className = "bp-settings-head-title";
    this.titleEl = document.createElement("h2");
    this.titleEl.id = "bp-settings-title";
    this.titleEl.textContent = t("palette.settings", "Settings");
    this.badgeEl = document.createElement("span");
    this.badgeEl.className = "bp-settings-badge";
    this.badgeEl.textContent = "Browser";
    headTitle.append(this.titleEl, this.badgeEl);

    this.closeBtn = document.createElement("button");
    this.closeBtn.type = "button";
    this.closeBtn.className = "icon";
    this.closeBtn.textContent = "×";
    this.closeBtn.setAttribute("aria-label", t("common.close", "Close"));
    this.closeBtn.addEventListener("click", () => this.close());
    head.append(headTitle, this.closeBtn);

    const body = document.createElement("div");
    body.className = "bp-settings-body";

    const nav = document.createElement("aside");
    nav.className = "bp-settings-nav";

    const searchWrap = document.createElement("div");
    searchWrap.className = "bp-settings-search";
    const searchIcon = document.createElement("span");
    searchIcon.className = "bp-settings-search-icon";
    searchIcon.appendChild(makeSearchIcon());

    this.searchInput = document.createElement("input");
    this.searchInput.type = "text";
    this.searchInput.className = "bp-settings-search-input";
    this.searchInput.placeholder = "Search settings…";
    this.searchInput.setAttribute("aria-label", "Search settings");
    this.searchInput.setAttribute("data-testid", "settings-search");

    this.searchClearBtn = document.createElement("button");
    this.searchClearBtn.type = "button";
    this.searchClearBtn.className = "bp-settings-search-clear bp-hidden";
    this.searchClearBtn.textContent = "×";
    this.searchClearBtn.title = "Clear search";
    this.searchClearBtn.setAttribute("aria-label", "Clear search");

    searchWrap.append(searchIcon, this.searchInput, this.searchClearBtn);

    this.navListEl = document.createElement("nav");
    this.navListEl.className = "bp-settings-nav-list";
    this.navListEl.setAttribute("aria-label", "Settings sections");

    nav.append(searchWrap, this.navListEl);

    this.paneEl = document.createElement("div");
    this.paneEl.className = "bp-settings-pane";

    body.append(nav, this.paneEl);
    panel.append(head, body);
    this.backdrop.appendChild(panel);

    this.backdrop.addEventListener("click", (e) => {
      if (e.target === this.backdrop) this.close();
    });

    this.searchInput.addEventListener("input", () => {
      this.query = this.searchInput.value;
      this.searchBrowseAll = this.query.trim().length > 0;
      this.syncSearchClear();
      this.render();
    });

    this.searchInput.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        const matches = filterSettingsNav(SETTINGS_PAGES, this.buildSearchItems(), this.query);
        if (matches.length > 0) {
          e.preventDefault();
          this.activePage = matches[0].page.id;
          this.searchBrowseAll = false;
          this.render();
        }
      } else if (e.key === "Escape" && this.query.trim().length > 0) {
        e.preventDefault();
        e.stopPropagation();
        this.clearSearch();
      }
    });

    this.searchClearBtn.addEventListener("click", () => {
      this.clearSearch();
      this.searchInput.focus();
    });

    root.appendChild(this.backdrop);
  }

  public isOpen(): boolean {
    return this.openState;
  }

  public handleEscape(): void {
    if (!this.openState) return;
    if (this.query.trim().length > 0) {
      this.clearSearch();
      this.searchInput.focus();
    } else {
      this.close();
    }
  }

  public async open(page?: SettingsPageId): Promise<void> {
    this.openState = true;
    if (page) this.activePage = page;
    this.backdrop.classList.remove("bp-hidden");
    this.render();
    void this.loadDevices();
    window.setTimeout(() => {
      if (this.openState) this.searchInput.focus();
    }, 0);
  }

  public async openToAccounts(): Promise<void> {
    this.forceProviders = true;
    this.query = "";
    this.searchInput.value = "";
    this.searchBrowseAll = false;
    this.syncSearchClear();
    await this.open("providers");
  }

  public close(): void {
    this.openState = false;
    this.forceProviders = false;
    this.query = "";
    this.searchInput.value = "";
    this.searchBrowseAll = false;
    this.syncSearchClear();
    this.backdrop.classList.add("bp-hidden");
  }

  public refreshLocale(): void {
    this.titleEl.textContent = t("palette.settings", "Settings");
    this.closeBtn.setAttribute("aria-label", t("common.close", "Close"));
    if (this.openState) this.render();
  }

  public async refresh(): Promise<void> {
    if (!this.openState) return;
    this.render();
    await this.loadDevices();
  }

  private clearSearch(): void {
    this.query = "";
    this.searchInput.value = "";
    this.searchBrowseAll = false;
    this.syncSearchClear();
    this.render();
  }

  private syncSearchClear(): void {
    this.searchClearBtn.classList.toggle("bp-hidden", this.query.length === 0);
  }

  private async loadDevices(): Promise<void> {
    const gen = ++this.loadGen;
    this.devicesLoading = !this.devicesLoaded;
    if (this.devicesLoading && this.openState) this.renderPane();
    try {
      const rows = await this.cb.onLoadDevices();
      if (gen !== this.loadGen) return;
      this.devicesRows = rows;
      this.devicesError = "";
      this.devicesLoaded = true;
    } catch (e) {
      if (gen !== this.loadGen) return;
      const msg = e instanceof Error && e.message ? e.message : "Not available from a browser";
      this.devicesError = `${msg} — manage paired devices in Backplane › Settings › Devices on desktop or a paired phone.`;
      this.devicesRows = [];
      this.devicesLoaded = true;
    } finally {
      if (gen === this.loadGen) {
        this.devicesLoading = false;
        if (this.openState) this.render();
      }
    }
  }

  private buildSearchItems(): SettingsSearchItem[] {
    const home = this.cb.onGetHome?.() ?? null;
    const noProvider = this.cb.onGetNoProvider?.() ?? null;
    const items: SettingsSearchItem[] = [
      {
        pageId: "appearance",
        pageTitle: "Appearance",
        groupLabel: "General",
        rowTitle: "Theme",
        rowDetail: "Auto (follow hub), Dark, or Light in this browser.",
        keywords: ["dark", "light", "auto", "appearance", "colors"],
      },
      {
        pageId: "appearance",
        pageTitle: "Appearance",
        groupLabel: "General",
        rowTitle: t("settings.lang", "Language"),
        rowDetail: "Interface language for this browser.",
        keywords: ["locale", "english", "espanol", "japanese", "language"],
      },
      {
        pageId: "providers",
        pageTitle: "AI providers",
        groupLabel: "Agents and tools",
        rowTitle: noProvider ? trs(noProvider.head) : "Provider connections",
        rowDetail: "Connect Claude, Codex, Antigravity, or other AI providers in Settings → Accounts.",
        keywords: ["accounts", "claude", "codex", "antigravity", "grok", "openai", "signin"],
      },
      {
        pageId: "devices",
        pageTitle: "Devices & browser",
        groupLabel: "Connect",
        rowTitle: "This browser",
        rowDetail: "Sign out and revoke this browser's session cookie.",
        keywords: ["logout", "sign out", "session", "cookie"],
      },
      {
        pageId: "devices",
        pageTitle: "Devices & browser",
        groupLabel: "Connect",
        rowTitle: "Paired devices",
        rowDetail: this.devicesRows.map((r) => `${r.title || r.head || ""} ${r.detail || r.text || ""}`).join(" ") || "Paired phones and browsers on this hub.",
        keywords: ["phone", "pair", "revoke", "browser"],
      },
      {
        pageId: "home",
        pageTitle: "Home machine",
        groupLabel: "Connect",
        rowTitle: home?.name ? `${home.name}` : t("home.machine", "Home machine"),
        rowDetail: "Which machine hosts your always-on hub and bots.",
        keywords: ["boat", "move", "online", "away", "queued", "machine"],
      },
    ];
    for (const r of this.devicesRows) {
      const title = r.title || r.head || "";
      if (title) {
        items.push({
          pageId: "devices",
          pageTitle: "Devices & browser",
          groupLabel: "Connect",
          rowTitle: title,
          rowDetail: r.detail || r.text || "",
          keywords: ["paired", "device"],
        });
      }
    }
    return items;
  }

  private render(): void {
    this.renderNav();
    this.renderPane();
  }

  private renderNav(): void {
    const frag = document.createDocumentFragment();
    const matches = filterSettingsNav(SETTINGS_PAGES, this.buildSearchItems(), this.query);
    const noProvider = this.cb.onGetNoProvider?.() ?? null;
    const home = this.cb.onGetHome?.() ?? null;

    if (matches.length === 0) {
      const empty = document.createElement("div");
      empty.className = "bp-settings-nav-empty";
      empty.setAttribute("data-testid", "settings-nav-empty");
      empty.textContent = "No matching settings";
      frag.appendChild(empty);
      this.navListEl.replaceChildren(frag);
      return;
    }

    let lastGroup = "";
    for (const match of matches) {
      const p = match.page;
      if (p.group !== lastGroup) {
        lastGroup = p.group;
        const g = SETTINGS_GROUPS.find((x) => x.id === p.group);
        if (g) {
          const head = document.createElement("div");
          head.className = "bp-settings-group";
          head.textContent = trs(g.label);
          frag.appendChild(head);
        }
      }

      const btn = document.createElement("button");
      btn.type = "button";
      const isSelected = !this.searchBrowseAll && p.id === this.activePage;
      btn.className = "bp-settings-nav-item" + (isSelected ? " active" : "");
      btn.setAttribute("data-testid", `settings-nav-${p.id}`);

      const rowTop = document.createElement("div");
      rowTop.className = "bp-settings-nav-row";

      const iconWrap = document.createElement("span");
      iconWrap.className = "bp-settings-nav-icon";
      iconWrap.appendChild(makeNavIcon(p.icon));

      const label = document.createElement("span");
      label.className = "bp-settings-nav-label";
      label.textContent = trs(p.title);

      rowTop.append(iconWrap, label);

      if (p.id === "providers" && (noProvider || this.forceProviders)) {
        const dot = document.createElement("span");
        dot.className = "bp-settings-nav-dot warn";
        dot.title = "Action needed";
        rowTop.appendChild(dot);
      } else if (p.id === "home" && home) {
        const dot = document.createElement("span");
        dot.className = "bp-settings-nav-dot " + (home.moving?.state === "failed" ? "bad" : home.online ? "ok" : "warn");
        rowTop.appendChild(dot);
      }

      btn.appendChild(rowTop);

      if (match.hits.length > 0) {
        for (const hit of match.hits) {
          const hitEl = document.createElement("div");
          hitEl.className = "bp-settings-hit";
          hitEl.textContent = hit;
          btn.appendChild(hitEl);
        }
      }

      btn.addEventListener("click", () => {
        this.activePage = p.id;
        this.searchBrowseAll = false;
        this.render();
      });
      frag.appendChild(btn);
    }

    this.navListEl.replaceChildren(frag);
  }

  private renderPane(): void {
    const frag = document.createDocumentFragment();
    const q = this.query.trim();

    if (q && this.searchBrowseAll) {
      this.renderSearchResults(frag, q);
      this.paneEl.replaceChildren(frag);
      return;
    }

    const meta = SETTINGS_PAGES.find((p) => p.id === this.activePage) ?? SETTINGS_PAGES[0];
    const header = document.createElement("div");
    header.className = "bp-settings-pane-head";
    const h3 = document.createElement("h3");
    h3.textContent = trs(meta.title);
    const sub = document.createElement("div");
    sub.className = "bp-settings-pane-sub";
    sub.textContent = meta.subtitle;
    header.append(h3, sub);
    frag.appendChild(header);

    switch (meta.id) {
      case "appearance":
        this.renderAppearancePage(frag);
        break;
      case "providers":
        this.renderProvidersPage(frag);
        break;
      case "devices":
        this.renderDevicesPage(frag);
        break;
      case "home":
        this.renderHomePage(frag);
        break;
    }

    this.paneEl.replaceChildren(frag);
  }

  private renderSearchResults(frag: DocumentFragment, q: string): void {
    const hits = filterSearchItems(this.buildSearchItems(), q);
    const header = document.createElement("div");
    header.className = "bp-settings-pane-head";
    const h3 = document.createElement("h3");
    h3.textContent = "Search results";
    const sub = document.createElement("div");
    sub.className = "bp-settings-pane-sub";
    sub.textContent = hits.length === 0
      ? `No settings match "${q}".`
      : `${hits.length} matching ${hits.length === 1 ? "setting" : "settings"} for "${q}"`;
    header.append(h3, sub);
    frag.appendChild(header);

    if (hits.length === 0) {
      const empty = document.createElement("div");
      empty.className = "bp-settings-empty";
      empty.textContent = "Try searching for theme, language, providers, sign out, devices, or home.";
      frag.appendChild(empty);
      return;
    }

    const pagesMatched = Array.from(new Set(hits.map((h) => h.pageId)));
    for (const pageId of pagesMatched) {
      const meta = SETTINGS_PAGES.find((p) => p.id === pageId);
      if (!meta) continue;
      const secBar = document.createElement("div");
      secBar.className = "bp-settings-search-sec";
      const secHead = document.createElement("div");
      secHead.className = "bp-settings-subhead";
      secHead.textContent = meta.title;
      const openPageBtn = document.createElement("button");
      openPageBtn.type = "button";
      openPageBtn.className = "bp-settings-search-open";
      openPageBtn.textContent = "Open page ›";
      openPageBtn.addEventListener("click", () => {
        this.activePage = pageId;
        this.searchBrowseAll = false;
        this.render();
      });
      secBar.append(secHead, openPageBtn);
      frag.appendChild(secBar);

      const matchedTitles = new Set(hits.filter((h) => h.pageId === pageId).map((h) => h.rowTitle));
      if (pageId === "appearance") {
        this.renderAppearancePage(frag, matchedTitles);
      } else if (pageId === "providers") {
        this.renderProvidersPage(frag);
      } else if (pageId === "devices") {
        this.renderDevicesPage(frag, matchedTitles);
      } else if (pageId === "home") {
        this.renderHomePage(frag);
      }
    }
  }

  private renderAppearancePage(frag: DocumentFragment, onlyRows?: Set<string>): void {
    const themePref = this.cb.onGetThemePref?.() ?? "auto";
    const hubTheme = this.cb.onGetHubTheme?.() ?? "dark";
    const resolvedTheme = resolveTheme(themePref, hubTheme);
    const localePref = this.cb.onGetLocalePref?.() ?? "auto";
    const langTitle = t("settings.lang", "Language");

    if (!onlyRows || onlyRows.has("Theme")) {
      const themeDesc = themePref === "auto"
        ? `Following hub theme (${hubTheme === "light" ? "Light" : "Dark"}). Stored in this browser only.`
        : `Using ${resolvedTheme === "light" ? "Light" : "Dark"} in this browser. Select Auto to follow the hub.`;

      frag.appendChild(this.makeSegmentedRow(
        "Theme",
        themeDesc,
        [
          { value: "auto", label: "Auto" },
          { value: "dark", label: "Dark" },
          { value: "light", label: "Light" },
        ],
        themePref,
        "settings-theme",
        (next) => {
          this.cb.onSetThemePref?.(next as WebThemePref);
          this.render();
        },
      ));
    }

    if (!onlyRows || onlyRows.has(langTitle)) {
      frag.appendChild(this.makeSegmentedRow(
        langTitle,
        "Interface language for this browser. Automatic follows the hub or browser language.",
        [
          { value: "auto", label: t("settings.lang.auto", "Automatic") },
          { value: "en", label: t("settings.lang.en", "English") },
          { value: "es", label: t("settings.lang.es", "Español") },
          { value: "ja", label: t("settings.lang.ja", "日本語") },
        ],
        localePref,
        "settings-locale",
        (next) => {
          this.cb.onSetLocalePref?.(next as WebLocalePref);
          this.render();
        },
      ));
    }
  }

  private makeSegmentedRow(
    titleText: string,
    detailText: string,
    options: Array<{ value: string; label: string }>,
    current: string,
    testIdPrefix: string,
    onPick: (value: string) => void,
  ): HTMLElement {
    const row = document.createElement("div");
    row.className = "bp-settings-row";

    const info = document.createElement("div");
    info.className = "bp-settings-row-info";
    const title = document.createElement("div");
    title.className = "bp-settings-row-title";
    title.textContent = titleText;
    const detail = document.createElement("div");
    detail.className = "bp-settings-row-detail";
    detail.textContent = detailText;
    info.append(title, detail);

    const group = document.createElement("div");
    group.className = "bp-chip-group";
    group.setAttribute("role", "group");
    group.setAttribute("aria-label", titleText);

    for (const opt of options) {
      const btn = document.createElement("button");
      btn.type = "button";
      const active = opt.value === current;
      btn.className = "bp-chip" + (active ? " active" : "");
      btn.textContent = opt.label;
      btn.setAttribute("aria-pressed", active ? "true" : "false");
      btn.setAttribute("data-testid", `${testIdPrefix}-${opt.value}`);
      btn.addEventListener("click", () => onPick(opt.value));
      group.appendChild(btn);
    }

    row.append(info, group);
    return row;
  }

  private renderProvidersPage(frag: DocumentFragment): void {
    const noProvider = this.cb.onGetNoProvider?.() ?? null;
    const sec = document.createElement("section");
    sec.className = "bp-settings-section";
    sec.setAttribute("data-testid", "settings-providers");

    if (noProvider || this.forceProviders) {
      const notice = document.createElement("div");
      notice.className = "bp-settings-notice warn";
      const info = document.createElement("div");
      info.className = "bp-settings-row-info";
      const head = document.createElement("div");
      head.className = "bp-settings-row-title";
      head.textContent = noProvider ? trs(noProvider.head) : t("provider.notice.head", "No AI provider connected");
      const sub = document.createElement("div");
      sub.className = "bp-settings-row-detail";
      sub.textContent = t(
        "web.settings.providers.browser",
        "Connect a provider in the Backplane desktop or mobile app under Settings → Accounts.",
      );
      info.append(head, sub);

      const btns = document.createElement("div");
      btns.className = "bp-set-btns";
      const link = document.createElement("a");
      link.className = "bp-link-btn primary";
      link.href = noProvider?.url || "backplane://settings/accounts";
      link.setAttribute("data-testid", "settings-open-accounts");
      link.textContent = noProvider ? trs(noProvider.button) : t("provider.notice.button", "Open Settings → Accounts");
      btns.appendChild(link);
      notice.append(info, btns);
      sec.appendChild(notice);
    } else {
      const row = document.createElement("div");
      row.className = "bp-settings-row";
      const info = document.createElement("div");
      info.className = "bp-settings-row-info";
      const titleRow = document.createElement("div");
      titleRow.className = "bp-settings-row-title-wrap";
      const title = document.createElement("span");
      title.className = "bp-settings-row-title";
      title.textContent = "Provider connections";
      const pill = document.createElement("span");
      pill.className = "bp-status-pill ok";
      pill.textContent = t("gpui.acct.connected", "Connected");
      titleRow.append(title, pill);

      const detail = document.createElement("div");
      detail.className = "bp-settings-row-detail";
      detail.textContent = "Provider sign-ins, fallback order, API keys, and usage limits are managed in the Backplane desktop or mobile app.";
      info.append(titleRow, detail);

      const btns = document.createElement("div");
      btns.className = "bp-set-btns";
      const link = document.createElement("a");
      link.className = "bp-link-btn";
      link.href = "backplane://settings/accounts";
      link.setAttribute("data-testid", "settings-open-accounts");
      link.textContent = t("provider.notice.button", "Open Settings → Accounts");
      btns.appendChild(link);
      row.append(info, btns);
      sec.appendChild(row);
    }

    frag.appendChild(sec);
  }

  private renderDevicesPage(frag: DocumentFragment, onlyRows?: Set<string>): void {
    const showSelf = !onlyRows || onlyRows.has("This browser");
    const showPaired = !onlyRows || onlyRows.has("Paired devices") || this.devicesRows.some((r) => onlyRows.has(r.title || r.head || ""));

    if (showSelf) {
      // This browser (session logout)
      const selfHead = document.createElement("div");
      selfHead.className = "bp-settings-subhead";
      selfHead.textContent = "This browser";
      frag.appendChild(selfHead);

      const selfRow = document.createElement("div");
      selfRow.className = "bp-settings-row";
      const selfInfo = document.createElement("div");
      selfInfo.className = "bp-settings-row-info";
      const selfTitle = document.createElement("div");
      selfTitle.className = "bp-settings-row-title";
      selfTitle.textContent = "Browser session";
      const selfDetail = document.createElement("div");
      selfDetail.className = "bp-settings-row-detail";
      selfDetail.textContent = "Sign out and revoke this browser's session cookie.";
      selfInfo.append(selfTitle, selfDetail);

      const logoutBtn = document.createElement("button");
      logoutBtn.type = "button";
      logoutBtn.className = "primary stop";
      logoutBtn.textContent = t("agy.signout", "Sign out");
      logoutBtn.setAttribute("data-testid", "logout-btn");
      logoutBtn.addEventListener("click", () => {
        logoutBtn.disabled = true;
        void this.cb.onLogout().catch(() => {
          selfDetail.setAttribute("role", "alert");
          selfDetail.className = "bp-settings-row-detail bp-bad";
          selfDetail.textContent = t("web.logout.failed", "Could not confirm sign-out. Try again.");
          logoutBtn.disabled = false;
        });
      });
      selfRow.append(selfInfo, logoutBtn);
      frag.appendChild(selfRow);
    }

    if (!showPaired) return;

    // Paired devices from hub
    const devHead = document.createElement("div");
    devHead.className = "bp-settings-subhead";
    devHead.textContent = "Paired devices";
    frag.appendChild(devHead);

    if (this.devicesLoading) {
      const loading = document.createElement("div");
      loading.className = "bp-settings-empty";
      loading.textContent = "Loading paired devices…";
      frag.appendChild(loading);
      return;
    }

    if (this.devicesError) {
      const err = document.createElement("div");
      err.className = "bp-settings-notice bad";
      const txt = document.createElement("div");
      txt.className = "bp-settings-row-detail";
      txt.textContent = this.devicesError;
      err.appendChild(txt);
      frag.appendChild(err);
      return;
    }

    const visibleDevices = !onlyRows || onlyRows.has("Paired devices")
      ? this.devicesRows
      : this.devicesRows.filter((r) => onlyRows.has(r.title || r.head || ""));

    if (visibleDevices.length === 0) {
      const empty = document.createElement("div");
      empty.className = "bp-settings-empty";
      empty.textContent = "No paired devices listed.";
      frag.appendChild(empty);
    } else {
      for (const r of visibleDevices) {
        const rowEl = document.createElement("div");
        rowEl.className = "bp-settings-row";

        const info = document.createElement("div");
        info.className = "bp-settings-row-info";
        const titleWrap = document.createElement("div");
        titleWrap.className = "bp-settings-row-title-wrap";
        const lbl = document.createElement("span");
        lbl.className = "bp-settings-row-title";
        lbl.textContent = r.title || r.head || "";
        titleWrap.appendChild(lbl);

        const detailText = r.detail || r.text || "";
        if (detailText) {
          const pill = document.createElement("span");
          const low = detailText.toLowerCase();
          pill.className = "bp-status-pill " + (low.includes("this browser") ? "accent" : "neutral");
          pill.textContent = detailText;
          titleWrap.appendChild(pill);
        }
        info.appendChild(titleWrap);
        rowEl.appendChild(info);

        if (r.buttons?.length) {
          const btns = document.createElement("div");
          btns.className = "bp-set-btns";
          for (const b of r.buttons) {
            const btn = document.createElement("button");
            btn.type = "button";
            if (b.primary) btn.className = "primary";
            if (b.danger) btn.className = "primary stop";
            btn.textContent = b.label || t("common.remove", "Remove");
            btn.addEventListener("click", () => {
              btn.disabled = true;
              void this.cb.onDevicesAct(b.action, b.value).finally(() => { void this.refresh(); });
            });
            btns.appendChild(btn);
          }
          rowEl.appendChild(btns);
        }
        frag.appendChild(rowEl);
      }
    }

    if (!onlyRows) {
      const note = document.createElement("div");
      note.className = "bp-settings-footnote";
      note.textContent = "Pair new browsers or phones, configure Tailscale HTTPS forwarding, or revoke access in desktop or mobile Settings → Devices.";
      frag.appendChild(note);
    }
  }

  private renderHomePage(frag: DocumentFragment): void {
    const home = this.cb.onGetHome?.() ?? null;
    const sec = document.createElement("section");
    sec.className = "bp-settings-section";
    sec.setAttribute("data-testid", "settings-home");

    if (!home) {
      const empty = document.createElement("div");
      empty.className = "bp-settings-empty";
      empty.textContent = "Single-hub mode — this hub is running locally without linked home machines.";
      sec.appendChild(empty);
      frag.appendChild(sec);
      return;
    }

    const row = document.createElement("div");
    row.className = "bp-settings-row";

    const info = document.createElement("div");
    info.className = "bp-settings-row-info";

    const titleWrap = document.createElement("div");
    titleWrap.className = "bp-settings-row-title-wrap";
    const title = document.createElement("span");
    title.className = "bp-settings-row-title";
    title.textContent = home.name || (home.kind === "boat" ? "Boat" : "This machine");
    titleWrap.appendChild(title);

    const kindPill = document.createElement("span");
    kindPill.className = "bp-status-pill neutral";
    kindPill.textContent = home.kind === "local" ? "This machine" : home.kind === "boat" ? "Boat" : "Linked machine";
    titleWrap.appendChild(kindPill);

    const statePill = document.createElement("span");
    let stateText = "Online";
    let stateTone = "ok";
    if (home.moving?.state === "moving") {
      stateText = "Moving";
      stateTone = "accent";
    } else if (home.moving?.state === "failed") {
      stateText = "Move failed";
      stateTone = "bad";
    } else if (!home.online) {
      stateText = "Away";
      stateTone = "warn";
    }
    statePill.className = `bp-status-pill ${stateTone}`;
    statePill.textContent = stateText;
    titleWrap.appendChild(statePill);

    const sub = document.createElement("div");
    sub.className = "bp-settings-row-detail";
    if (home.moving?.state === "moving") {
      sub.textContent = home.moving.step
        ? t("home.moving.step", "Moving home to {to}: {step}").replace("{to}", home.moving.to).replace("{step}", home.moving.step)
        : t("home.moving", "Moving home to {to}…").replace("{to}", home.moving.to);
    } else if (home.moving?.state === "failed") {
      sub.textContent = home.moving.note
        ? t("home.move.failed.note", "Home move failed: {note}").replace("{note}", home.moving.note)
        : t("home.move.failed", "Home move failed");
    } else if (!home.online) {
      sub.textContent = home.queuedCount > 0
        ? t("home.away.queued", "Home ({name}) is away · {n} queued").replace("{name}", home.name || "home").replace("{n}", String(home.queuedCount))
        : t("home.away", "Home ({name}) is away").replace("{name}", home.name || "home");
    } else {
      sub.textContent = "Your active home hub is online and serving threads and bots.";
    }

    info.append(titleWrap, sub);
    row.appendChild(info);

    if (home.canMoveHere && this.cb.onMoveHomeHere) {
      const moveBtn = document.createElement("button");
      moveBtn.type = "button";
      moveBtn.className = "primary";
      moveBtn.textContent = t("home.move.here", "Make this machine the home");
      moveBtn.setAttribute("data-testid", "home-move-here-btn");
      moveBtn.addEventListener("click", () => {
        moveBtn.disabled = true;
        void this.cb.onMoveHomeHere?.().finally(() => { void this.refresh(); });
      });
      row.appendChild(moveBtn);
    }

    sec.appendChild(row);
    frag.appendChild(sec);
  }
}
