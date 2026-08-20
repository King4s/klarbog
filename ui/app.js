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
/** Last moms-suggest payload (preview only; never posts). @type {object | null} */
let journalMomsLast = null;
/** Draft fields shared across moms apply ↔ journal form re-renders. @type {object | null} */
let journalDraft = null;

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

function journalFormDefaults() {
  const fromPending = journalPending?.entry;
  const d = journalDraft || {};
  const leg0 = fromPending?.legs?.[0];
  const leg1 = fromPending?.legs?.[1];
  return {
    memo: d.memo ?? fromPending?.memo ?? "udgift #vat25 #receipt",
    account1: d.account1 ?? leg0?.account ?? "6000",
    direction1: d.direction1 ?? leg0?.direction ?? "debit",
    amount1:
      d.amount1 ??
      (leg0?.amount?.units != null ? String(leg0.amount.units) : "12500"),
    account2: d.account2 ?? leg1?.account ?? "5800",
    direction2: d.direction2 ?? leg1?.direction ?? "credit",
    amount2:
      d.amount2 ??
      (leg1?.amount?.units != null ? String(leg1.amount.units) : "12500"),
    momsGross: d.momsGross ?? "12500",
    momsMemo: d.momsMemo ?? "udgift #vat25 #receipt",
  };
}

async function renderJournal() {
  const company = settings.company.trim();
  const pending = journalPending;
  const defs = journalFormDefaults();
  let companyHint = company
    ? ""
    : `<p class="muted">Sæt firmasti under Indstillinger før preview/commit.</p>`;

  const moms = journalMomsLast;
  let momsResult = `<p class="muted">Intet moms-forslag endnu. Kræver memo-tag <span class="money">#vat25</span> (eller moms:25).</p>`;
  if (moms && moms.suggested === true) {
    momsResult = `
      <div class="panel nested">
        <h2>Moms-forslag (poster ikke)</h2>
        <p class="muted">auto_post=${escapeHtml(String(moms.auto_post))} · rate_bps=${escapeHtml(String(moms.rate_bps ?? "—"))}</p>
        <ul class="plain">
          <li>Brutto: <span class="money">${escapeHtml(String(moms.gross_minor))} øre</span> (${formatDkk(Number(moms.gross_minor))})</li>
          <li>Netto: <span class="money">${escapeHtml(String(moms.net_minor))} øre</span> (${formatDkk(Number(moms.net_minor))})</li>
          <li>Moms: <span class="money">${escapeHtml(String(moms.vat_minor))} øre</span> (${formatDkk(Number(moms.vat_minor))})</li>
        </ul>
        <div class="actions">
          <button class="primary" type="button" id="moms-apply-gross" ${company ? "" : "disabled"}>Anvend brutto på begge ben</button>
          <button class="ghost" type="button" id="moms-clear">Ryd forslag</button>
        </div>
        <p class="muted">Anvend sætter memo + beløb (øre) til brutto på begge ben — stadig preview→commit.</p>
      </div>`;
  } else if (moms && moms.suggested === false) {
    momsResult = `<p class="muted">Ingen forslag (${escapeHtml(moms.reason || "ingen #vat25-tag")}). auto_post=false.</p>`;
  }

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

      <div class="panel nested">
        <h2>Moms-forslag</h2>
        <p class="muted">POST /api/v1/journal/moms-suggest — kun forslag, aldrig post (ADR-011).</p>
        <form id="moms-suggest-form" class="grid">
          <label>Brutto moms-inkl. (øre)
            <input name="gross_minor" required inputmode="numeric" pattern="-?[0-9]+" value="${escapeHtml(defs.momsGross)}" />
          </label>
          <label>Memo (skal indeholde #vat25 for forslag)
            <input name="moms_memo" required value="${escapeHtml(defs.momsMemo)}" />
          </label>
          <div class="actions">
            <button class="primary" type="submit" ${company ? "" : "disabled"}>Hent moms-forslag</button>
          </div>
        </form>
        ${momsResult}
      </div>

      <form id="journal-preview-form" class="grid">
        <label>Memo
          <input name="memo" required placeholder="udgift #receipt" value="${escapeHtml(defs.memo)}" />
        </label>
        <div class="grid two">
          <fieldset class="leg">
            <legend>Ben 1 (debit)</legend>
            <label>Konto
              <input name="account1" required value="${escapeHtml(defs.account1)}" />
            </label>
            <label>Retning
              <select name="direction1">
                <option value="debit">debit</option>
                <option value="credit">credit</option>
              </select>
            </label>
            <label>Beløb (øre)
              <input name="amount1" required inputmode="numeric" pattern="-?[0-9]+" placeholder="12500" value="${escapeHtml(defs.amount1)}" />
            </label>
          </fieldset>
          <fieldset class="leg">
            <legend>Ben 2 (credit)</legend>
            <label>Konto
              <input name="account2" required value="${escapeHtml(defs.account2)}" />
            </label>
            <label>Retning
              <select name="direction2">
                <option value="debit">debit</option>
                <option value="credit">credit</option>
              </select>
            </label>
            <label>Beløb (øre)
              <input name="amount2" required inputmode="numeric" pattern="-?[0-9]+" placeholder="12500" value="${escapeHtml(defs.amount2)}" />
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

  const d1 = document.querySelector('select[name="direction1"]');
  if (d1) d1.value = defs.direction1;
  const d2 = document.querySelector('select[name="direction2"]');
  if (d2) d2.value = defs.direction2;

  document.getElementById("moms-suggest-form")?.addEventListener("submit", async (ev) => {
    ev.preventDefault();
    if (!company) {
      setFlash("err", "Firmasti mangler");
      await renderJournal();
      return;
    }
    const fd = new FormData(ev.target);
    try {
      const gross = parseMinor(fd.get("gross_minor"), "Brutto");
      const memo = String(fd.get("moms_memo") || "").trim();
      if (!memo) throw new Error("Memo kræves");
      journalDraft = {
        ...(journalDraft || {}),
        momsGross: String(gross),
        momsMemo: memo,
      };
      const env = await api(settings, "/api/v1/journal/moms-suggest", {
        method: "POST",
        body: JSON.stringify({ company, gross_minor: gross, memo }),
      });
      journalMomsLast = env.data || { suggested: false };
      if (journalMomsLast.suggested) {
        setFlash(
          "ok",
          `Moms-forslag: netto ${journalMomsLast.net_minor} / moms ${journalMomsLast.vat_minor} øre (poster ikke)`,
        );
      } else {
        setFlash("ok", "Ingen moms-forslag (mangler #vat25-tag)");
      }
      await renderJournal();
    } catch (e) {
      setFlash("err", e.message);
      await renderJournal();
    }
  });

  document.getElementById("moms-apply-gross")?.addEventListener("click", async () => {
    if (!journalMomsLast?.suggested) return;
    const gross = String(journalMomsLast.gross_minor);
    const memoEl = document.querySelector('#moms-suggest-form input[name="moms_memo"]');
    const memo = memoEl ? String(memoEl.value || "").trim() : defs.momsMemo;
    journalDraft = {
      ...(journalDraft || {}),
      memo: memo || defs.memo,
      amount1: gross,
      amount2: gross,
      momsGross: gross,
      momsMemo: memo || defs.momsMemo,
    };
    setFlash("ok", "Brutto udfyldt på begge ben — kør Preview");
    await renderJournal();
  });

  document.getElementById("moms-clear")?.addEventListener("click", async () => {
    journalMomsLast = null;
    setFlash("ok", "Moms-forslag ryddet");
    await renderJournal();
  });

  document.getElementById("journal-preview-form")?.addEventListener("submit", async (ev) => {
    ev.preventDefault();
    if (!company) {
      setFlash("err", "Firmasti mangler");
      await renderJournal();
      return;
    }
    try {
      const fd = new FormData(ev.target);
      journalDraft = {
        ...(journalDraft || {}),
        memo: String(fd.get("memo") || ""),
        account1: String(fd.get("account1") || ""),
        direction1: String(fd.get("direction1") || "debit"),
        amount1: String(fd.get("amount1") || ""),
        account2: String(fd.get("account2") || ""),
        direction2: String(fd.get("direction2") || "credit"),
        amount2: String(fd.get("amount2") || ""),
      };
      const entry = buildJournalEntry(fd);
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
      journalDraft = null;
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


function idStr(v) {
  if (v == null) return "";
  if (typeof v === "string") return v;
  if (typeof v === "object" && v.id != null) return String(v.id);
  return String(v);
}

/** @type {{ drafts?: object[], errors?: string[], count?: number, source?: string, provider?: string } | null} */
let bankLastResult = null;

async function renderBank() {
  const company = settings.company.trim();
  const companyHint = company
    ? ""
    : `<p class="muted">Sæt firmasti under Indstillinger før preview.</p>`;

  let resultHtml = "";
  if (bankLastResult) {
    const drafts = Array.isArray(bankLastResult.drafts) ? bankLastResult.drafts : [];
    const errors = Array.isArray(bankLastResult.errors) ? bankLastResult.errors : [];
    const draftRows = drafts
      .map((d) => {
        const minor =
          typeof d.amount_minor === "number"
            ? d.amount_minor
            : typeof d.amount?.units === "number"
              ? d.amount.units
              : null;
        return `<tr>
          <td>${escapeHtml(d.memo || "")}</td>
          <td class="money">${escapeHtml(formatDkk(minor))}</td>
          <td class="money">${escapeHtml(minor == null ? "—" : String(minor))}</td>
        </tr>`;
      })
      .join("");
    const errList = errors.map((e) => `<li>${escapeHtml(e)}</li>`).join("");
    resultHtml = `
      <div class="panel nested">
        <h2>Seneste preview</h2>
        <p class="muted">${escapeHtml(String(bankLastResult.count ?? drafts.length))} udkast ·
          ${escapeHtml(bankLastResult.source || "—")} /
          ${escapeHtml(bankLastResult.provider || "—")}</p>
        ${
          errList
            ? `<ul class="flash err" style="list-style:disc;padding-left:1.25rem">${errList}</ul>`
            : ""
        }
        <table class="table">
          <thead><tr><th>Memo</th><th>DKK</th><th>Øre (i64)</th></tr></thead>
          <tbody>${
            draftRows || `<tr><td colspan="3" class="muted">Ingen udkast</td></tr>`
          }</tbody>
        </table>
      </div>`;
  }

  app.innerHTML = `
    ${renderFlash()}
    <section class="panel">
      <h1>Bank</h1>
      <p class="lede">Import-preview — ingen journal-post. Beløb vises i DKK; API bruger øre (i64).</p>
      ${companyHint}
      <form id="bank-preview-form" class="grid">
        <div class="grid two">
          <label>Kilde
            <select name="source" id="bank-source">
              <option value="csv" selected>csv</option>
              <option value="api">api</option>
            </select>
          </label>
          <label>Provider
            <select name="provider">
              <option value="generic_dk" selected>generic_dk</option>
              <option value="revolut">revolut</option>
              <option value="stripe">stripe</option>
            </select>
          </label>
        </div>
        <div class="csv-block" id="bank-csv-block">
          <label>CSV (indsæt)
            <textarea name="csv" id="bank-csv" placeholder="Dato;Tekst;Beløb&#10;19.08.2026;Kontor;-125,50"></textarea>
          </label>
        </div>
        <p class="muted">Firma: <span class="money">${escapeHtml(company || "—")}</span>. API kræver provider-nøgler i server-miljø.</p>
        <div class="actions">
          <button class="primary" type="submit" ${company ? "" : "disabled"}>Preview import</button>
        </div>
      </form>
      ${resultHtml}
    </section>
  `;

  const sourceSel = document.getElementById("bank-source");
  const csvBlock = document.getElementById("bank-csv-block");
  const syncCsv = () => {
    if (csvBlock) csvBlock.hidden = sourceSel?.value === "api";
  };
  sourceSel?.addEventListener("change", syncCsv);
  syncCsv();

  document.getElementById("bank-preview-form")?.addEventListener("submit", async (ev) => {
    ev.preventDefault();
    if (!company) {
      setFlash("err", "Firmasti mangler");
      await renderBank();
      return;
    }
    const fd = new FormData(ev.target);
    const source = String(fd.get("source") || "csv");
    const provider = String(fd.get("provider") || "generic_dk");
    const csv = String(fd.get("csv") || "");
    const body = { company, source, provider, currency: "DKK" };
    if (source === "csv") body.csv = csv;
    try {
      const env = await api(settings, "/api/v1/bank/import/preview", {
        method: "POST",
        body: JSON.stringify(body),
      });
      const data = env.data || {};
      bankLastResult = {
        drafts: Array.isArray(data.drafts) ? data.drafts : [],
        errors: Array.isArray(env.errors) ? env.errors : [],
        count: data.count,
        source: data.source || source,
        provider: data.provider || provider,
      };
      setFlash("ok", `Preview OK · ${bankLastResult.count ?? bankLastResult.drafts.length} udkast`);
      await renderBank();
    } catch (e) {
      bankLastResult = {
        drafts: [],
        errors: [e.message],
        count: 0,
        source,
        provider,
      };
      setFlash("err", e.message);
      await renderBank();
    }
  });
}

async function renderBilag() {
  const company = settings.company.trim();
  let docsBody = `<p class="muted">Sæt firmasti under Indstillinger.</p>`;
  let excBody = "";

  if (company) {
    try {
      const q = new URLSearchParams({ company });
      const [docsEnv, excEnv] = await Promise.all([
        api(settings, `/api/v1/documents?${q}`),
        api(settings, `/api/v1/exceptions?${q}`),
      ]);
      const docs = Array.isArray(docsEnv.data) ? docsEnv.data : [];
      const exceptions = Array.isArray(excEnv.data) ? excEnv.data : [];
      const docRows = docs
        .map(
          (d) => `<tr>
            <td class="money">${escapeHtml(idStr(d.id))}</td>
            <td>${escapeHtml(d.kind || "")}</td>
            <td class="money">${escapeHtml(d.path_hint || "")}</td>
            <td>${escapeHtml(d.notes || "")}</td>
          </tr>`,
        )
        .join("");
      const excRows = exceptions
        .map(
          (e) => `<tr>
            <td class="money">${escapeHtml(idStr(e.id))}</td>
            <td>${escapeHtml(e.code || "")}</td>
            <td>${escapeHtml(e.severity || "")}</td>
            <td>${escapeHtml(e.message || "")}</td>
            <td>${e.open === false ? "lukket" : "åben"}</td>
          </tr>`,
        )
        .join("");
      docsBody = `
        <h2 class="subhead">Dokumenter</h2>
        <table class="table">
          <thead><tr><th>Id</th><th>Type</th><th>Sti</th><th>Note</th></tr></thead>
          <tbody>${docRows || `<tr><td colspan="4" class="muted">Ingen bilag</td></tr>`}</tbody>
        </table>`;
      excBody = `
        <h2 class="subhead">Undtagelser</h2>
        <table class="table">
          <thead><tr><th>Id</th><th>Kode</th><th>Alvor</th><th>Besked</th><th>Status</th></tr></thead>
          <tbody>${
            excRows || `<tr><td colspan="5" class="muted">Ingen åbne undtagelser</td></tr>`
          }</tbody>
        </table>
        <form id="exception-form" class="grid" style="margin-top:1.25rem">
          <div class="grid two">
            <label>Kode
              <input name="code" required placeholder="missing_attachment" />
            </label>
            <label>Alvor
              <select name="severity">
                <option value="info">info</option>
                <option value="warn" selected>warn</option>
                <option value="error">error</option>
              </select>
            </label>
          </div>
          <label>Besked
            <input name="message" required placeholder="Mangler scan" />
          </label>
          <div class="actions">
            <button class="primary" type="submit">Opret undtagelse</button>
          </div>
        </form>`;
    } catch (e) {
      docsBody = `<p class="flash err">${escapeHtml(e.message)}</p>`;
    }
  }

  app.innerHTML = `
    ${renderFlash()}
    <section class="panel">
      <h1>Bilag</h1>
      <p class="lede">Dokumenter og undtagelser — ingen journal-skrivning.</p>
      ${docsBody}
      ${excBody}
    </section>
  `;

  document.getElementById("exception-form")?.addEventListener("submit", async (ev) => {
    ev.preventDefault();
    const fd = new FormData(ev.target);
    try {
      await api(settings, "/api/v1/exceptions", {
        method: "POST",
        body: JSON.stringify({
          company: settings.company.trim(),
          code: String(fd.get("code") || "").trim(),
          severity: String(fd.get("severity") || "warn"),
          message: String(fd.get("message") || "").trim(),
        }),
      });
      setFlash("ok", "Undtagelse oprettet");
      await renderBilag();
    } catch (e) {
      setFlash("err", e.message);
      await renderBilag();
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
  else if (view === "bank") await renderBank();
  else if (view === "bilag") await renderBilag();
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
