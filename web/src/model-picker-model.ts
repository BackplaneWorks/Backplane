export interface PickerModelChoice {
  value: string;
  label: string;
  note: string;
  on: boolean;
  ok: boolean;
  why: string;
}

export interface PickerEffortChoice {
  value: string;
  label: string;
  on: boolean;
}

export interface PickerProviderGroup {
  provider: string;
  label: string;
  ok: boolean;
  why: string;
  models: PickerModelChoice[];
}

export interface PickerFavoriteItem {
  key: string;
  provider: string;
  model: string;
  label: string;
  note: string;
  on: boolean;
  ok: boolean;
  why: string;
}

export interface PickerSnapshot {
  chip: string;
  provider: string;
  model: string;
  effort: string;
  efforts: PickerEffortChoice[];
  providers: PickerProviderGroup[];
  favoriteKeys: string[];
  favorites: PickerFavoriteItem[];
}

export interface PickerViewModelRow extends PickerModelChoice {
  provider: string;
  starred: boolean;
  starrable: boolean;
}

export interface PickerViewModelGroup {
  provider: string;
  label: string;
  ok: boolean;
  why: string;
  models: PickerViewModelRow[];
}

export interface PickerViewModel {
  favorites: PickerFavoriteItem[];
  groups: PickerViewModelGroup[];
  efforts: PickerEffortChoice[];
}

const PROVIDER_TITLES: Record<string, string> = {
  claude: "Claude Code",
  codex: "Codex CLI",
  grok: "Grok CLI",
  muse: "Muse Code",
  antigravity: "Google Antigravity",
  opencode: "OpenCode",
  pi: "Pi",
  omp: "Oh My Pi",
};

export const MAX_FAVORITES = 64;
export const MAX_FAV_LEN = 200;
const CTRL_RE = /[\x00-\x1f\x7f]/;
const WS_RE = /\s/;

export function normalizeFavProvider(provider: string): string {
  return provider.trim().replace(/^@+/, "").toLowerCase();
}

export function normalizeFavKey(provider: string, model: string): string {
  if (CTRL_RE.test(provider) || CTRL_RE.test(model)) return "";
  const p = normalizeFavProvider(provider);
  const m = model.trim();
  if (!p || !m || p.includes("/") || WS_RE.test(p)) return "";
  if (p.length + 1 + m.length > MAX_FAV_LEN) return "";
  return `${p}/${m}`;
}

export function parseFavKey(key: string): { provider: string; model: string } | null {
  const idx = key.indexOf("/");
  if (idx <= 0) return null;
  const rawProvider = key.slice(0, idx);
  const rawModel = key.slice(idx + 1);
  if (!normalizeFavKey(rawProvider, rawModel)) return null;
  return { provider: normalizeFavProvider(rawProvider), model: rawModel.trim() };
}

export function normalizeFavKeys(keys: unknown): string[] {
  if (!Array.isArray(keys)) return [];
  const out: string[] = [];
  const seen = new Set<string>();
  for (const item of keys) {
    if (typeof item !== "string") continue;
    const parsed = parseFavKey(item);
    if (!parsed) continue;
    const nk = `${parsed.provider}/${parsed.model}`;
    if (!seen.has(nk)) {
      seen.add(nk);
      out.push(nk);
      if (out.length >= MAX_FAVORITES) break;
    }
  }
  return out;
}

export function toggleFavoriteKey(
  keys: string[],
  provider: string,
  model: string,
  on?: boolean,
): { keys: string[]; starred: boolean } {
  const nk = normalizeFavKey(provider, model);
  const current = normalizeFavKeys(keys);
  if (!nk) return { keys: current, starred: false };
  const idx = current.indexOf(nk);
  const want = on !== undefined ? on : idx < 0;
  if (want && idx < 0) {
    if (current.length >= MAX_FAVORITES) {
      return { keys: current, starred: false };
    }
    current.push(nk);
  } else if (!want && idx >= 0) {
    current.splice(idx, 1);
  }
  return { keys: current, starred: want };
}

function asObj(v: unknown): Record<string, unknown> | null {
  return v && typeof v === "object" && !Array.isArray(v) ? (v as Record<string, unknown>) : null;
}

function asArr(v: unknown): unknown[] {
  return Array.isArray(v) ? v : [];
}

function asStr(v: unknown): string {
  return typeof v === "string" ? v : "";
}

function providerLabel(id: string): string {
  return PROVIDER_TITLES[id] ?? id;
}

function buildFavoriteItems(
  providers: PickerProviderGroup[],
  favoriteKeys: string[],
  curProvider: string,
  curModel: string,
): PickerFavoriteItem[] {
  const byProvider = new Map<string, PickerProviderGroup>();
  for (const g of providers) byProvider.set(g.provider, g);
  const out: PickerFavoriteItem[] = [];
  for (const key of normalizeFavKeys(favoriteKeys)) {
    const parsed = parseFavKey(key);
    if (!parsed) continue;
    const grp = byProvider.get(parsed.provider);
    const hasCatalog = Boolean(grp && grp.models.some((m) => m.value !== ""));
    const hit = grp?.models.find((m) => m.value === parsed.model);
    let ok = hit ? hit.ok && (grp?.ok ?? true) : (grp?.ok ?? true);
    let why = hit?.why || grp?.why || "";
    if (ok && hasCatalog && !hit) {
      ok = false;
      why = "not available on this machine";
    }
    out.push({
      key,
      provider: parsed.provider,
      model: parsed.model,
      label: hit?.label || parsed.model,
      note: hit?.note || "",
      on: parsed.provider === curProvider && parsed.model === curModel,
      ok,
      why,
    });
  }
  return out;
}

export function parsePickerResponse(raw: unknown, fallbackFavKeys: string[] = []): PickerSnapshot {
  const root = asObj(raw);
  const picker = asObj(root?.picker) ?? root ?? {};
  const sections = asArr(picker.sections).map(asObj).filter((s): s is Record<string, unknown> => Boolean(s));

  const sectionByAction = (action: string) => sections.find((s) => asStr(s.action) === action);
  const provSec = sectionByAction("provider");
  const modelSec = sectionByAction("model");
  const effortSec = sectionByAction("effort");

  const provChoices = asArr(provSec?.choices).map(asObj).filter((c): c is Record<string, unknown> => Boolean(c));
  const modelChoicesRaw = asArr(modelSec?.choices).map(asObj).filter((c): c is Record<string, unknown> => Boolean(c));
  const effortChoicesRaw = asArr(effortSec?.choices).map(asObj).filter((c): c is Record<string, unknown> => Boolean(c));

  const chip = asStr(picker.chip);
  const curProviderRaw =
    asStr(provChoices.find((c) => c.on === true)?.value) ||
    chip.split("/")[0]?.trim() ||
    "claude";
  const curProvider = normalizeFavProvider(curProviderRaw) || "claude";
  const curModel = asStr(modelChoicesRaw.find((c) => c.on === true)?.value);
  const curEffort = asStr(effortChoicesRaw.find((c) => c.on === true)?.value);

  const efforts: PickerEffortChoice[] = effortChoicesRaw.map((e) => ({
    value: asStr(e.value),
    label: asStr(e.label) || "auto",
    on: e.on === true,
  }));

  const agentsObj = asObj(picker.agents);
  const providerIds: string[] = [];
  const seenProv = new Set<string>();
  const addProv = (id: string) => {
    const n = normalizeFavProvider(id);
    if (n && !seenProv.has(n)) {
      seenProv.add(n);
      providerIds.push(n);
    }
  };
  for (const c of provChoices) addProv(asStr(c.value) || asStr(c.label));
  if (agentsObj) {
    for (const k of Object.keys(agentsObj)) addProv(k);
  }
  addProv(curProvider);

  const providers: PickerProviderGroup[] = providerIds.map((pid) => {
    const pChoice = provChoices.find((c) => normalizeFavProvider(asStr(c.value) || asStr(c.label)) === pid);
    const agentInfo = asObj(agentsObj?.[pid]);
    const ok = agentInfo ? agentInfo.ok !== false : (pChoice ? pChoice.ok !== false : true);
    const why = asStr(agentInfo?.why) || asStr(pChoice?.why);
    const agentModels = asArr(agentInfo?.models).map(asObj).filter((m): m is Record<string, unknown> => Boolean(m));

    let models: PickerModelChoice[];
    if (agentModels.length > 0) {
      models = agentModels.map((m) => {
        const value = asStr(m.value);
        const label = asStr(m.label) || (value === "" ? "default" : value);
        const note = asStr(m.note);
        return {
          value,
          label,
          note,
          on: pid === curProvider && value === curModel,
          ok: m.ok !== false && ok,
          why: asStr(m.why) || why,
        };
      });
    } else if (pid === curProvider && modelChoicesRaw.length > 0) {
      models = modelChoicesRaw.map((m) => {
        const value = asStr(m.value);
        const rawLabel = asStr(m.label);
        return {
          value,
          label: value === "" ? "default" : value,
          note: rawLabel && rawLabel !== value ? rawLabel : "",
          on: m.on === true,
          ok: m.ok !== false && ok,
          why: asStr(m.why) || why,
        };
      });
    } else if (ok) {
      models = [
        {
          value: "",
          label: "default",
          note: "",
          on: pid === curProvider && curModel === "",
          ok: true,
          why: "",
        },
      ];
    } else {
      models = [];
    }

    if (pid === curProvider && curModel && !models.some((m) => m.value === curModel)) {
      const fromSec = modelChoicesRaw.find((m) => asStr(m.value) === curModel);
      const rawLabel = asStr(fromSec?.label);
      models.push({
        value: curModel,
        label: curModel,
        note: rawLabel && rawLabel !== curModel ? rawLabel : "",
        on: true,
        ok: true,
        why: "",
      });
    }

    return {
      provider: pid,
      label: providerLabel(pid),
      ok,
      why,
      models,
    };
  });

  const hasHubFavKeys = Array.isArray(picker.favoriteKeys);
  const rawFavRows = asArr(picker.favorites).map(asObj).filter((f): f is Record<string, unknown> => Boolean(f));
  const favoriteKeys = hasHubFavKeys
    ? normalizeFavKeys(picker.favoriteKeys)
    : rawFavRows.length > 0
      ? normalizeFavKeys(rawFavRows.map((f) => asStr(f.key) || normalizeFavKey(asStr(f.provider), asStr(f.model))))
      : normalizeFavKeys(fallbackFavKeys);

  const favorites: PickerFavoriteItem[] =
    rawFavRows.length > 0
      ? rawFavRows
          .map((f) => {
            const p = normalizeFavProvider(asStr(f.provider));
            const m = asStr(f.model).trim();
            const key = normalizeFavKey(p, m);
            if (!key) return null;
            return {
              key,
              provider: p,
              model: m,
              label: asStr(f.label) || m,
              note: asStr(f.note),
              on: f.on === true || (p === curProvider && m === curModel),
              ok: f.ok !== false,
              why: asStr(f.why),
            };
          })
          .filter((x): x is PickerFavoriteItem => Boolean(x))
      : buildFavoriteItems(providers, favoriteKeys, curProvider, curModel);

  return {
    chip,
    provider: curProvider,
    model: curModel,
    effort: curEffort,
    efforts,
    providers,
    favoriteKeys,
    favorites,
  };
}

export function applyFavoriteKeysToSnapshot(snap: PickerSnapshot, keys: string[]): PickerSnapshot {
  const favoriteKeys = normalizeFavKeys(keys);
  return {
    ...snap,
    favoriteKeys,
    favorites: buildFavoriteItems(snap.providers, favoriteKeys, snap.provider, snap.model),
  };
}

export function buildPickerViewModel(snap: PickerSnapshot, query: string): PickerViewModel {
  const words = query
    .trim()
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean);
  const matches = (hay: string) => {
    if (words.length === 0) return true;
    const low = hay.toLowerCase();
    return words.every((w) => low.includes(w));
  };

  const favSet = new Set(snap.favoriteKeys);
  const favorites = snap.favorites.filter((f) => matches(`${f.provider} ${f.model} ${f.label} ${f.note}`));

  const groups: PickerViewModelGroup[] = [];
  for (const g of snap.providers) {
    const models: PickerViewModelRow[] = g.models
      .filter((m) => matches(`${g.provider} ${g.label} ${m.value} ${m.label} ${m.note}`))
      .map((m) => {
        const key = m.value ? normalizeFavKey(g.provider, m.value) : "";
        return {
          ...m,
          provider: g.provider,
          starrable: m.value !== "",
          starred: Boolean(key && favSet.has(key)),
        };
      });
    if (models.length > 0 || (words.length === 0 && !g.ok && g.why)) {
      groups.push({
        provider: g.provider,
        label: g.label,
        ok: g.ok,
        why: g.why,
        models,
      });
    }
  }

  return {
    favorites,
    groups,
    efforts: snap.efforts,
  };
}
