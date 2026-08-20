const STORAGE_KEY = "klarbog.ui.settings.v1";

const defaultSettings = () => ({
  apiBase: "",
  company: "",
  actorKind: "user",
  actorId: "ui-dev",
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
    };
    saveSettings(settings);
    setFlash("ok", "Indstillinger gemt");
    renderSettings();
    refreshFoot();
  });
}

async function render() {
  setActiveNav();
  flash = view === "settings" ? flash : flash;
  if (view === "home") await renderHome();
  else if (view === "parties") await renderParties();
  else if (view === "invoices") await renderInvoices();
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
