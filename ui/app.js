const STORAGE_KEY = "klarbog.ui.settings.v1";

const defaultSettings = () => ({
  apiBase: "",
  company: "",
  actorKind: "user",
  actorId: "ui-dev",
  apiToken: "",
});

function loadSettings() {
  try {
    return { ...defaultSettings(), ...JSON.parse(localStorage.getItem(STORAGE_KEY) || "{}") };
  } catch {
    return defaultSettings();
  }
}

function saveSettings(s) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(s));
}

function apiBase(settings) {
  return (settings.apiBase || "").replace(/\/$/, "") || window.location.origin;
}

function formatDkk(minor) {
  if (typeof minor !== "number" || !Number.isFinite(minor)) return "—";
  const sign = minor < 0 ? "-" : "";
  const abs = Math.abs(Math.trunc(minor));
  const whole = Math.floor(abs / 100);
  const ore = String(abs % 100).padStart(2, "0");
  return `${sign}${whole.toLocaleString("da-DK")},${ore} kr`;
}

async function api(settings, path, options = {}) {
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

const app = document.getElementById("app");
const footStatus = document.getElementById("foot-status");
let settings = loadSettings();
let view = "home";
let flash = null;
/** @type {{ entry: object, confirm_token: string, expires_unix_ms?: number, payload_digest?: string } | null} */
let journalPending = null;

function parseMinor(raw, label) {
  const s = String(raw ?? "").trim();
  if (!/^-?\d+$/.test(s)) throw new Error(`${label}: angiv heltal i øre (i64)`);
  const n = Number(s);
  if (!Number.isSafeInteger(n)) throw new Error(`${label}: beløb uden for sikkert heltal`);
  return n;
}

function buildJournalEntry(fd) {
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
      kind: settings.actorKind || "user",
      id: settings.actorId || "ui-dev",
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

function setFlash(kind, message) {
  flash = { kind, message };
}

function renderFlash() {
  if (!flash) return "";
  return `<p class="flash ${flash.kind === "err" ? "err" : "ok"}" role="status">${escapeHtml(
    flash.message,
  )}</p>`;
}

function escapeHtml(s) {
  return String(s)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function setActiveNav() {
  document.querySelectorAll(".nav-btn").forEach((btn) => {
    btn.classList.toggle("is-active", btn.dataset.view === view);
  });
}

async function refreshFoot() {
  try {
    const health = await fetch(`${apiBase(settings)}/health`).then((r) => r.json());
    footStatus.textContent = health.ok
      ? `${health.service} ${health.version}`
      : "API svarer ikke";
  } catch {
    footStatus.textContent = "API offline";
  }
}

async function renderHome() {
  let statusHtml = `<p class="muted">Indlæser status…</p>`;
  try {
    const env = await api(settings, "/api/v1/status");
    const data = env.data || {};
    const plugins = (data.plugins || [])
      .map((p) => `<li><strong>${escapeHtml(p.id)}</strong> · ${escapeHtml(p.version)}</li>`)
      .join("");
    statusHtml = `
      <div class="grid two">
        <div>
          <div class="muted">Mode</div>
          <div class="stat">${escapeHtml(data.mode || "—")}</div>
        </div>
        <div>
          <div class="muted">Bind</div>
          <div class="stat">${escapeHtml(data.bind || "—")}</div>
        </div>
      </div>
      <p class="muted" style="margin-top:1rem">Allowlist</p>
      <p class="money">${escapeHtml(String(data.allowlist_root || "—"))}</p>
      <p class="muted" style="margin-top:1rem">Plugins</p>
      <ul>${plugins || "<li class='muted'>Ingen</li>"}</ul>
    `;
  } catch (e) {
    statusHtml = `<p class="flash err">${escapeHtml(e.message)}</p>`;
  }

  app.innerHTML = `
    ${renderFlash()}
    <section class="panel">
      <h1>Oversigt</h1>
      <p class="lede">Klarbog kører lokalt. API og UI deler samme loopback-origin.</p>
      ${statusHtml}
    </section>
  `;
}

async function renderParties() {
  const company = settings.company.trim();
  let body = `<p class="muted">Sæt firmasti under Indstillinger.</p>`;
  if (company) {
    try {
      const q = new URLSearchParams({ company });
      const env = await api(settings, `/api/v1/crm/parties?${q}`);
      const parties = Array.isArray(env.data) ? env.data : env.data?.parties || [];
      const rows = parties
        .map(
          (p) => `<tr>
            <td class="money">${escapeHtml(p.party_id || p.id || "")}</td>
            <td>${escapeHtml(p.display_name || "")}</td>
          </tr>`,
        )
        .join("");
      body = `
        <table class="table">
          <thead><tr><th>Id</th><th>Navn</th></tr></thead>
          <tbody>${rows || `<tr><td colspan="2" class="muted">Ingen parter</td></tr>`}</tbody>
        </table>
        <form id="party-form" class="grid" style="margin-top:1.25rem">
          <label>Nyt partenavn
            <input name="display_name" required placeholder="Kunde ApS" />
          </label>
          <div class="actions">
            <button class="primary" type="submit">Opret part</button>
          </div>
        </form>
      `;
    } catch (e) {
      body = `<p class="flash err">${escapeHtml(e.message)}</p>`;
    }
  }

  app.innerHTML = `
    ${renderFlash()}
    <section class="panel">
      <h1>Parter</h1>
      <p class="lede">CRM uden journal-skrivning.</p>
      ${body}
    </section>
  `;

  const form = document.getElementById("party-form");
  form?.addEventListener("submit", async (ev) => {
    ev.preventDefault();
    const fd = new FormData(form);
    try {
      await api(settings, "/api/v1/crm/parties", {
        method: "POST",
        body: JSON.stringify({
          company: settings.company.trim(),
          display_name: String(fd.get("display_name") || "").trim(),
        }),
      });
      setFlash("ok", "Part oprettet");
      await renderParties();
    } catch (e) {
      setFlash("err", e.message);
      await renderParties();
    }
  });
}

async function renderInvoices() {
  const company = settings.company.trim();
  let body = `<p class="muted">Sæt firmasti under Indstillinger.</p>`;
  if (company) {
    try {
      const q = new URLSearchParams({ company });
      const env = await api(settings, `/api/v1/invoices/drafts?${q}`);
      const invoices = Array.isArray(env.data) ? env.data : env.data?.invoices || [];
      const rows = invoices
        .map((inv) => {
          const total = inv.total_minor ?? inv.amount_minor ?? null;
          return `<tr>
            <td class="money">${escapeHtml(inv.invoice_id || inv.id || "")}</td>
            <td>${escapeHtml(inv.status || "")}</td>
            <td class="money">${escapeHtml(formatDkk(total))}</td>
            <td class="money">${escapeHtml(inv.party_id || "")}</td>
          </tr>`;
        })
        .join("");
      body = `
        <table class="table">
          <thead><tr><th>Id</th><th>Status</th><th>Beløb</th><th>Part</th></tr></thead>
          <tbody>${rows || `<tr><td colspan="4" class="muted">Ingen fakturaer</td></tr>`}</tbody>
        </table>
      `;
    } catch (e) {
      body = `<p class="flash err">${escapeHtml(e.message)}</p>`;
    }
  }

  app.innerHTML = `
    ${renderFlash()}
    <section class="panel">
      <h1>Fakturaer</h1>
      <p class="lede">Beløb vises i kroner; API’et bruger stadig øre (i64).</p>
      ${body}
    </section>
  `;
}

async function renderJournal() {
  const company = settings.company.trim();
  const pending = journalPending;
  let companyHint = company
    ? ""
    : `<p class="muted">Sæt firmasti under Indstillinger før preview/commit.</p>`;

  const tokenBlock = pending
    ? `<div class="panel nested">
        <h2>Bekræftelse klar</h2>
        <p class="muted">Gemt entry + token — commit bruger samme payload.</p>
        <p class="muted">confirm_token</p>
        <p class="token money" id="confirm-token">${escapeHtml(pending.confirm_token)}</p>
        <p class="muted">Udløber (unix ms): ${escapeHtml(String(pending.expires_unix_ms ?? "—"))}</p>
        <p class="muted">Digest: <span class="money">${escapeHtml(pending.payload_digest || "—")}</span></p>
        <div class="actions">
          <button class="primary" type="button" id="journal-commit" ${company ? "" : "disabled"}>Commit journal</button>
          <button class="ghost" type="button" id="journal-clear">Ryd token</button>
        </div>
      </div>`
    : `<p class="muted">Kør preview for at få et confirm_token.</p>`;

  app.innerHTML = `
    ${renderFlash()}
    <section class="panel">
      <h1>Journal</h1>
      <p class="lede">To-fase bogføring: preview → confirm_token → commit. Beløb i øre (i64).</p>
      ${companyHint}
      <form id="journal-preview-form" class="grid">
        <label>Memo
          <input name="memo" required placeholder="udgift #receipt" value="${escapeHtml(
            pending?.entry?.memo || "",
          )}" />
        </label>
        <div class="grid two">
          <fieldset class="leg">
            <legend>Ben 1 (debit)</legend>
            <label>Konto
              <input name="account1" required value="${escapeHtml(
                pending?.entry?.legs?.[0]?.account || "6000",
              )}" />
            </label>
            <label>Retning
              <select name="direction1">
                <option value="debit" selected>debit</option>
                <option value="credit">credit</option>
              </select>
            </label>
            <label>Beløb (øre)
              <input name="amount1" required inputmode="numeric" pattern="-?[0-9]+" placeholder="12500" value="${escapeHtml(
                pending?.entry?.legs?.[0]?.amount?.units != null
                  ? String(pending.entry.legs[0].amount.units)
                  : "12500",
              )}" />
            </label>
          </fieldset>
          <fieldset class="leg">
            <legend>Ben 2 (credit)</legend>
            <label>Konto
              <input name="account2" required value="${escapeHtml(
                pending?.entry?.legs?.[1]?.account || "5800",
              )}" />
            </label>
            <label>Retning
              <select name="direction2">
                <option value="debit">debit</option>
                <option value="credit" selected>credit</option>
              </select>
            </label>
            <label>Beløb (øre)
              <input name="amount2" required inputmode="numeric" pattern="-?[0-9]+" placeholder="12500" value="${escapeHtml(
                pending?.entry?.legs?.[1]?.amount?.units != null
                  ? String(pending.entry.legs[1].amount.units)
                  : "12500",
              )}" />
            </label>
          </fieldset>
        </div>
        <p class="muted">Samme beløb på begge ben giver balance. Preview poster ikke.</p>
        <div class="actions">
          <button class="primary" type="submit" ${company ? "" : "disabled"}>Preview</button>
        </div>
      </form>
      ${tokenBlock}
    </section>
  `;

  if (pending?.entry?.legs?.[0]?.direction) {
    const d1 = document.querySelector('select[name="direction1"]');
    if (d1) d1.value = pending.entry.legs[0].direction;
  }
  if (pending?.entry?.legs?.[1]?.direction) {
    const d2 = document.querySelector('select[name="direction2"]');
    if (d2) d2.value = pending.entry.legs[1].direction;
  }

  document.getElementById("journal-preview-form")?.addEventListener("submit", async (ev) => {
    ev.preventDefault();
    if (!company) {
      setFlash("err", "Firmasti mangler");
      await renderJournal();
      return;
    }
    try {
      const entry = buildJournalEntry(new FormData(ev.target));
      const env = await api(settings, "/api/v1/journal/preview", {
        method: "POST",
        body: JSON.stringify({ company, entry }),
      });
      const data = env.data || {};
      if (!data.confirm_token) throw new Error("Intet confirm_token i svar");
      journalPending = {
        entry,
        confirm_token: String(data.confirm_token),
        expires_unix_ms: data.expires_unix_ms,
        payload_digest: data.payload_digest,
      };
      setFlash("ok", "Preview OK — token klar til commit");
      await renderJournal();
    } catch (e) {
      setFlash("err", e.message);
      await renderJournal();
    }
  });

  document.getElementById("journal-clear")?.addEventListener("click", async () => {
    journalPending = null;
    setFlash("ok", "Token ryddet");
    await renderJournal();
  });

  document.getElementById("journal-commit")?.addEventListener("click", async () => {
    if (!company || !journalPending) return;
    try {
      const env = await api(settings, "/api/v1/journal/commit", {
        method: "POST",
        body: JSON.stringify({
          company,
          entry: journalPending.entry,
          confirm_token: journalPending.confirm_token,
        }),
      });
      const data = env.data || {};
      journalPending = null;
      setFlash(
        "ok",
        `Posted ${data.id || "ok"} · digest ${String(data.digest || "").slice(0, 16)}…`,
      );
      await renderJournal();
    } catch (e) {
      setFlash("err", e.message);
      await renderJournal();
    }
  });
}

async function renderChart() {
  const company = settings.company.trim();
  let body = `<p class="muted">Sæt firmasti under Indstillinger.</p>`;
  if (company) {
    try {
      const q = new URLSearchParams({ company });
      const env = await api(settings, `/api/v1/rules/chart?${q}`);
      const data = env.data || {};
      const accounts = Array.isArray(data.accounts) ? data.accounts : [];
      const rows = accounts
        .map((a) => {
          const range =
            a.min != null && a.max != null
              ? `${a.min}–${a.max}`
              : a.code || "";
          return `<tr>
            <td class="money">${escapeHtml(a.code || "")}</td>
            <td>${escapeHtml(a.label || "")}</td>
            <td class="money">${escapeHtml(range)}</td>
          </tr>`;
        })
        .join("");
      body = `
        <p class="muted">Stub-kontoplan (DEV). Regel-hint: <span class="money">${escapeHtml(
          data.rule_known_account || "—",
        )}</span>${data.stub ? " · stub" : ""}</p>
        <table class="table">
          <thead><tr><th>Kode</th><th>Label</th><th>Range</th></tr></thead>
          <tbody>${rows || `<tr><td colspan="3" class="muted">Tom</td></tr>`}</tbody>
        </table>
      `;
    } catch (e) {
      body = `<p class="flash err">${escapeHtml(e.message)}</p>`;
    }
  }

  app.innerHTML = `
    ${renderFlash()}
    <section class="panel">
      <h1>Kontoplan</h1>
      <p class="lede">GET /api/v1/rules/chart — kun læsning, ingen journal-skrivning.</p>
      ${body}
    </section>
  `;
}

function renderSettings() {
  app.innerHTML = `
    ${renderFlash()}
    <section class="panel">
      <h1>Indstillinger</h1>
      <p class="lede">Gemmes i browseren. Firmasti skal ligge under allowlisten.</p>
      <form id="settings-form" class="grid">
        <label>API-base (tom = samme origin)
          <input name="apiBase" value="${escapeHtml(settings.apiBase)}" placeholder="http://127.0.0.1:3195" />
        </label>
        <label>Firma-sti (company)
          <input name="company" value="${escapeHtml(settings.company)}" placeholder="companies/min-aps" required />
        </label>
        <div class="grid two">
          <label>Actor kind
            <select name="actorKind">
              <option value="user" ${settings.actorKind === "user" ? "selected" : ""}>user</option>
              <option value="agent" ${settings.actorKind === "agent" ? "selected" : ""}>agent</option>
              <option value="system" ${settings.actorKind === "system" ? "selected" : ""}>system</option>
            </select>
          </label>
          <label>Actor id
            <input name="actorId" value="${escapeHtml(settings.actorId)}" required />
          </label>
        </div>
        <label>API-token (kun hvis serveren har KLARBOG_API_TOKEN)
          <input name="apiToken" type="password" value="${escapeHtml(settings.apiToken || "")}" autocomplete="off" placeholder="tom = ikke påkrævet" />
        </label>
        <div class="actions">
          <button class="primary" type="submit">Gem</button>
        </div>
      </form>
    </section>
  `;

  document.getElementById("settings-form")?.addEventListener("submit", (ev) => {
    ev.preventDefault();
    const fd = new FormData(ev.target);
    settings = {
      apiBase: String(fd.get("apiBase") || "").trim(),
      company: String(fd.get("company") || "").trim(),
      actorKind: String(fd.get("actorKind") || "user"),
      actorId: String(fd.get("actorId") || "ui-dev").trim(),
      apiToken: String(fd.get("apiToken") || "").trim(),
    };
    saveSettings(settings);
    setFlash("ok", "Indstillinger gemt");
    renderSettings();
    refreshFoot();
  });
}

async function render() {
  setActiveNav();
  if (view === "home") await renderHome();
  else if (view === "parties") await renderParties();
  else if (view === "invoices") await renderInvoices();
  else if (view === "journal") await renderJournal();
  else if (view === "chart") await renderChart();
  else renderSettings();
}

document.querySelectorAll(".nav-btn").forEach((btn) => {
  btn.addEventListener("click", async () => {
    view = btn.dataset.view;
    flash = null;
    await render();
  });
});

refreshFoot();
render();
