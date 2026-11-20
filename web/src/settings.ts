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
  onLogout: () => Promise<void>;
}

export class SettingsModal {
  private backdrop: HTMLElement;
  private titleEl: HTMLElement;
  private closeBtn: HTMLButtonElement;
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

  public close(): void {
    this.openState = false;
    this.backdrop.classList.add("bp-hidden");
  }

  public refreshLocale(): void {
    this.titleEl.textContent = `${t("palette.settings", "Settings")} · Devices`;
    this.closeBtn.setAttribute("aria-label", t("common.close", "Close"));
    if (this.openState) void this.refresh();
  }

  public async refresh(): Promise<void> {
    const frag = document.createDocumentFragment();

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
      const msg = e instanceof Error && e.message ? e.message : "Not available from a browser";
      err.textContent = `${msg} — manage paired devices in Backplane › Settings › Devices on desktop or a paired phone.`;
      devSec.appendChild(err);
    }

    frag.appendChild(devSec);
    this.bodyEl.replaceChildren(frag);
  }
}
