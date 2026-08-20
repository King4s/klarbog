/** Shared Klarbog UI helpers + mutable session state (soft-split wave 51). */

export const STORAGE_KEY = "klarbog.ui.settings.v1";

export const defaultSettings = () => ({
  apiBase: "",
  company: "",
  actorKind: "user",
  actorId: "ui-dev",
  apiToken: "",
});

export function loadSettings() {
  try {
    return { ...defaultSettings(), ...JSON.parse(localStorage.getItem(STORAGE_KEY) || "{}") };
  } catch {
    return defaultSettings();
  }
}

export function saveSettings(s) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(s));
}

export function apiBase(settings) {
  return (settings.apiBase || "").replace(/\/$/, "") || window.location.origin;
}

export function formatDkk(minor) {
  if (typeof minor !== "number" || !Number.isFinite(minor)) return "—";
  const sign = minor < 0 ? "-" : "";
  const abs = Math.abs(Math.trunc(minor));
  const whole = Math.floor(abs / 100);
  const ore = String(abs % 100).padStart(2, "0");
  return `${sign}${whole.toLocaleString("da-DK")},${ore} kr`;
}

export async function api(settings, path, options = {}) {
  const headers = new Headers(options.headers || {});
  if (!headers.has("Content-Type") && options.body) {
    headers.set("Content-Type", "application/json");
  }
  headers.set("x-klarbog-actor-kind", settings.actorKind || "user");
  headers.set("x-klarbog-actor-id", settings.actorId || "ui-dev");
  const token = (settings.apiToken || "").trim();
  if (token) {
    headers.set("Authorization", `Bearer ${token}`);
  }
  const res = await fetch(`${apiBase(settings)}${path}`, { ...options, headers });
  const text = await res.text();
  let json = null;
  try {
    json = text ? JSON.parse(text) : null;
  } catch {
    throw new Error(`Ugyldigt JSON (${res.status})`);
  }
  if (!res.ok || (json && json.ok === false)) {
    const errs = json?.errors?.join?.("; ") || res.statusText || "Fejl";
    throw new Error(errs);
  }
  return json;
}

/** Mutable UI session — views import this bag (never rebind the export). */
export const ui = {
  app: null,
  footStatus: null,
  settings: loadSettings(),
  view: "home",
  flash: null,
  journalPending: null,
  journalMomsLast: null,
  journalDraft: null,
  gdprEraseLast: null,
  bankLastResult: null,
  bankReconcileDraft: null,
  bankSuggestLast: null,
  retentionPurgeLast: null,
};

export function parseMinor(raw, label) {
  const s = String(raw ?? "").trim();
  if (!/^-?\d+$/.test(s)) throw new Error(`${label}: angiv heltal i øre (i64)`);
  const n = Number(s);
  if (!Number.isSafeInteger(n)) throw new Error(`${label}: beløb uden for sikkert heltal`);
  return n;
}

export function buildJournalEntry(fd) {
  const memo = String(fd.get("memo") || "").trim();
  if (!memo) throw new Error("Memo kræves");
  const amount1 = parseMinor(fd.get("amount1"), "Ben 1 beløb");
  const amount2 = parseMinor(fd.get("amount2"), "Ben 2 beløb");
  const account1 = String(fd.get("account1") || "").trim();
  const account2 = String(fd.get("account2") || "").trim();
  if (!account1 || !account2) throw new Error("Begge konti kræves");
  const direction1 = String(fd.get("direction1") || "debit");
  const direction2 = String(fd.get("direction2") || "credit");
  return {
    as_of: new Date().toISOString(),
    memo,
    actor: {
      kind: ui.settings.actorKind || "user",
      id: ui.settings.actorId || "ui-dev",
    },
    legs: [
      {
        account: account1,
        direction: direction1,
        amount: { units: amount1 },
        currency: "DKK",
      },
      {
        account: account2,
        direction: direction2,
        amount: { units: amount2 },
        currency: "DKK",
      },
    ],
  };
}

export function setFlash(kind, message) {
  ui.flash = { kind, message };
}

export function renderFlash() {
  if (!ui.flash) return "";
  return `<p class="flash ${ui.flash.kind === "err" ? "err" : "ok"}" role="status">${escapeHtml(
    ui.flash.message,
  )}</p>`;
}

export function escapeHtml(s) {
  return String(s)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

export function setActiveNav() {
  document.querySelectorAll(".nav-btn").forEach((btn) => {
    btn.classList.toggle("is-active", btn.dataset.view === ui.view);
  });
}

export async function refreshFoot() {
  try {
    const health = await fetch(`${apiBase(ui.settings)}/health`).then((r) => r.json());
    ui.footStatus.textContent = health.ok
      ? `${health.service} ${health.version}`
      : "API svarer ikke";
  } catch {
    ui.footStatus.textContent = "API offline";
  }
}

export function idStr(v) {
  if (v == null) return "";
  if (typeof v === "string") return v;
  if (typeof v === "object" && v.id != null) return String(v.id);
  return String(v);
}
