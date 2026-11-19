// Settings › Devices modal + Appearance (Dark/Light) + Browser Sign Out.
import { t } from "./i18n";

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
  onSetTheme: (theme: "dark" | "light") => void;
  onLogout: () => Promise<void>;
}

export class SettingsModal {
  private backdrop: HTMLElement;
  private bodyEl: HTMLElement;
  private cb: SettingsCallbacks;
  private openState = false;

  constructor(root: HTMLElement, cb: SettingsCallbacks) {
    this.cb = cb;

    this.backdrop = document.createElement("div");
    this.backdrop.className = "bp-modal-backdrop bp-hidden";
    this.backdrop.setAttribute("data-testid", "settings-modal");

    const panel = document.createElement("div");
    panel.className = "bp-modal";

    const head = document.createElement("div");
    head.className = "bp-modal-head";
    const title = document.createElement("h2");
    title.textContent = "Settings · Devices";
    const closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.className = "icon";
    closeBtn.textContent = "×";
    closeBtn.setAttribute("aria-label", "Close settings");
    closeBtn.addEventListener("click", () => this.close());
    head.append(title, closeBtn);

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

  public close(): void {
    this.openState = false;
    this.backdrop.classList.add("bp-hidden");
  }

  public async refresh(): Promise<void> {
    const frag = document.createDocumentFragment();

    // Appearance section
    const appSec = document.createElement("section");
    appSec.className = "bp-set-section";
    const appHead = document.createElement("div");
    appHead.className = "bp-set-head";
    appHead.textContent = "Appearance";
    const appRow = document.createElement("div");
    appRow.className = "bp-set-row";
    const themeLabel = document.createElement("span");
    themeLabel.textContent = "Theme";
    const themeBtns = document.createElement("div");
    themeBtns.className = "bp-set-btns";
    const isLight = document.documentElement.classList.contains("light");
    const darkBtn = document.createElement("button");
    darkBtn.type = "button";
    darkBtn.className = isLight ? "" : "on";
    darkBtn.textContent = "Dark";
    darkBtn.addEventListener("click", () => { this.cb.onSetTheme("dark"); void this.refresh(); });
    const lightBtn = document.createElement("button");
    lightBtn.type = "button";
    lightBtn.className = isLight ? "on" : "";
    lightBtn.textContent = "Light";
    lightBtn.addEventListener("click", () => { this.cb.onSetTheme("light"); void this.refresh(); });
    themeBtns.append(darkBtn, lightBtn);
    appRow.append(themeLabel, themeBtns);
    appSec.append(appHead, appRow);
    frag.appendChild(appSec);

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
    logoutBtn.textContent = "Sign out";
    logoutBtn.setAttribute("data-testid", "logout-btn");
    logoutBtn.addEventListener("click", () => {
      logoutBtn.disabled = true;
      void this.cb.onLogout();
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
      err.textContent = e instanceof Error ? e.message : "Could not load devices.";
      devSec.appendChild(err);
    }

    frag.appendChild(devSec);
    this.bodyEl.replaceChildren(frag);
  }
}
