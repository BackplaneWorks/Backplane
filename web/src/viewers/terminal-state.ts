// Pure session-selection and input-gating state machine for the web terminal pane.
// Keeps monotonic selection generations so delayed `term.new` or `term.watch` replies
// can never override a newer explicit shell selection or route input before snapshot ready.

export class TerminalSwitchState {
  gen = 0;
  activeKey = "";
  inputReady = false;
  knownKeys: string[] = [];

  beginSelect(targetKey?: string): number {
    this.gen += 1;
    this.inputReady = false;
    if (targetKey !== undefined) {
      this.activeKey = targetKey;
      this.rememberKey(targetKey);
    }
    return this.gen;
  }

  rememberKey(key: string): void {
    if (key && !this.knownKeys.includes(key)) {
      this.knownKeys.push(key);
    }
  }

  isCurrent(gen: number): boolean {
    return gen === this.gen;
  }

  /** Complete an async `term.new` reply. Returns true only if this generation is still current. */
  resolveNew(gen: number, key: string): boolean {
    this.rememberKey(key);
    if (gen !== this.gen) return false;
    this.activeKey = key;
    return true;
  }

  /** Complete an async `term.watch` snapshot. Returns true only if still current for `key`. */
  resolveWatch(gen: number, key: string): boolean {
    if (gen !== this.gen || this.activeKey !== key) return false;
    this.inputReady = true;
    return true;
  }

  /** Whether user typing/paste or resize should be forwarded to `activeKey`. */
  canSendInput(): boolean {
    return this.inputReady && this.activeKey.length > 0;
  }
}
