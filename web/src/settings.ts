// Settings › Devices modal + Appearance (Dark/Light) + Browser Sign Out + Home machine.
import { t, trs } from "./i18n";
import type { NoProviderNotice } from "./provider-notice";
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
}

export class SettingsModal {
  private backdrop: HTMLElement;
  private titleEl: HTMLElement;
  private closeBtn: HTMLButtonElement;
  private bodyEl: HTMLElement;
  private cb: SettingsCallbacks;
  private openState = false;
  private forceProviders = false;
  private providersEl: HTMLElement | null = null;

  constructor(root: HTMLElement, cb: SettingsCallbacks) {
    this.cb = cb;

    this.backdrop = document.createElement("div");
    this.backdrop.className = "bp-modal-backdrop bp-hidden";
    this.backdrop.setAttribute("data-testid", "settings-modal");

    const panel = document.createElement("div");
    panel.className = "bp-modal";

    const head = document.createElement("div");
    head.className = "bp-modal-head";
    this.titleEl = document.createElement("h2");
    this.titleEl.textContent = `${t("palette.settings", "Settings")} · Devices`;
    this.closeBtn = document.createElement("button");
    this.closeBtn.type = "button";
    this.closeBtn.className = "icon";
    this.closeBtn.textContent = "×";
    this.closeBtn.setAttribute("aria-label", t("common.close", "Close"));
    this.closeBtn.addEventListener("click", () => this.close());
    head.append(this.titleEl, this.closeBtn);

    this.bodyEl = document.createElement("div");
    this.bodyEl.className = "bp-modal-body";

    panel.append(head, this.bodyEl);
    this.backdrop.appendChild(panel);
    this.backdrop.addEventListener("click", (e) => {
      if (e.target === this.backdrop) this.close();
    });
    root.appendChild(this.backdrop);
  }

  public isOpen(): boolean {
    return this.openState;
  }

  public async open(): Promise<void> {
    this.openState = true;
    this.backdrop.classList.remove("bp-hidden");
    await this.refresh();
  }

  public async openToAccounts(): Promise<void> {
    this.forceProviders = true;
    await this.open();
    this.providersEl?.scrollIntoView({ block: "nearest" });
  }

  public close(): void {
    this.openState = false;
    this.forceProviders = false;
    this.backdrop.classList.add("bp-hidden");
  }

  public refreshLocale(): void {
    this.titleEl.textContent = `${t("palette.settings", "Settings")} · Devices`;
    this.closeBtn.setAttribute("aria-label", t("common.close", "Close"));
    if (this.openState) void this.refresh();
  }

  public async refresh(): Promise<void> {
    const frag = document.createDocumentFragment();
    this.providersEl = null;

    const noProvider = this.cb.onGetNoProvider?.() ?? null;
    if (noProvider || this.forceProviders) {
      const provSec = document.createElement("section");
      provSec.className = "bp-set-section";
      provSec.setAttribute("data-testid", "settings-providers");
      const provHead = document.createElement("div");
      provHead.className = "bp-set-head";
      provHead.textContent = t("web.settings.providers", "AI providers");
      const provRow = document.createElement("div");
      provRow.className = "bp-set-row";
      const info = document.createElement("div");
      info.className = "bp-set-info";
      const title = document.createElement("div");
      title.textContent = noProvider ? trs(noProvider.head) : t("provider.notice.head", "No AI provider connected");
      const sub = document.createElement("div");
      sub.className = "bp-dim";
      sub.textContent = t(
        "web.settings.providers.browser",
        "Connect a provider in the Backplane desktop or mobile app under Settings → Accounts.",
      );
      info.append(title, sub);
      const btns = document.createElement("div");
      btns.className = "bp-set-btns";
      const link = document.createElement("a");
      link.className = "bp-link-btn primary";
      link.href = noProvider?.url || "backplane://settings/accounts";
      link.setAttribute("data-testid", "settings-open-accounts");
      link.textContent = noProvider ? trs(noProvider.button) : t("provider.notice.button", "Open Settings → Accounts");
      btns.appendChild(link);
      provRow.append(info, btns);
      provSec.append(provHead, provRow);
      this.providersEl = provSec;
      frag.appendChild(provSec);
    }

    // Home machine section (docs/PHONE-DATA.md, The home)
    const home = this.cb.onGetHome?.() ?? null;
    if (home) {
      const homeSec = document.createElement("section");
      homeSec.className = "bp-set-section";
      homeSec.setAttribute("data-testid", "settings-home");
      const homeHead = document.createElement("div");
      homeHead.className = "bp-set-head";
      homeHead.textContent = t("home.machine", "Home machine");
      const homeRow = document.createElement("div");
      homeRow.className = "bp-set-row";
      const info = document.createElement("div");
      info.className = "bp-set-info";
      const title = document.createElement("div");
      const where =
        home.kind === "local"
          ? `${home.name || "This machine"} (this machine)`
          : home.kind === "boat"
            ? `${home.name || "Boat"} (Boat)`
            : home.name || "Linked machine";
      title.textContent = where;
      info.appendChild(title);
      const sub = document.createElement("div");
      sub.className = "bp-dim";
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
        sub.textContent = "Online";
      }
      info.appendChild(sub);
      homeRow.appendChild(info);
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
        homeRow.appendChild(moveBtn);
      }
      homeSec.append(homeHead, homeRow);
      frag.appendChild(homeSec);
    }

    // This Browser section (self-logout)
    const selfSec = document.createElement("section");
    selfSec.className = "bp-set-section";
    const selfHead = document.createElement("div");
    selfHead.className = "bp-set-head";
    selfHead.textContent = "This browser";
    const selfRow = document.createElement("div");
    selfRow.className = "bp-set-row";
    const selfText = document.createElement("span");
    selfText.className = "bp-dim";
    selfText.textContent = "Sign out and revoke this browser's session cookie.";
    const logoutBtn = document.createElement("button");
    logoutBtn.type = "button";
    logoutBtn.className = "primary stop";
    logoutBtn.textContent = t("agy.signout", "Sign out");
    logoutBtn.setAttribute("data-testid", "logout-btn");
    logoutBtn.addEventListener("click", () => {
      logoutBtn.disabled = true;
      void this.cb.onLogout().catch(() => {
        selfText.setAttribute("role", "alert");
        selfText.textContent = t("web.logout.failed", "Could not confirm sign-out. Try again.");
        logoutBtn.disabled = false;
      });
    });
    selfRow.append(selfText, logoutBtn);
    selfSec.append(selfHead, selfRow);
    frag.appendChild(selfSec);

    // Paired devices from hub (devices.page)
    const devSec = document.createElement("section");
    devSec.className = "bp-set-section";
    const devHead = document.createElement("div");
    devHead.className = "bp-set-head";
    devHead.textContent = "Paired devices";
    devSec.appendChild(devHead);

    try {
      const rows = await this.cb.onLoadDevices();
      if (rows.length === 0) {
        const empty = document.createElement("div");
        empty.className = "bp-dim";
        empty.textContent = "No devices listed.";
        devSec.appendChild(empty);
      } else {
        for (const r of rows) {
          const rowEl = document.createElement("div");
          rowEl.className = "bp-set-row";
          const info = document.createElement("div");
          info.className = "bp-set-info";
          const lbl = document.createElement("div");
          lbl.textContent = r.title || r.head || "";
          info.appendChild(lbl);
          if (r.text || r.detail) {
            const sub = document.createElement("div");
            sub.className = "bp-dim";
            sub.textContent = r.text || r.detail || "";
            info.appendChild(sub);
          }
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
          devSec.appendChild(rowEl);
        }
      }
    } catch (e) {
      const err = document.createElement("div");
      err.className = "bp-dim";
      const msg = e instanceof Error && e.message ? e.message : "Not available from a browser";
      err.textContent = `${msg} — manage paired devices in Backplane › Settings › Devices on desktop or a paired phone.`;
      devSec.appendChild(err);
    }

    frag.appendChild(devSec);
    this.bodyEl.replaceChildren(frag);
  }
}
