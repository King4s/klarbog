import {
  ui,
  api,
  formatDkk,
  parseMinor,
  buildJournalEntry,
  setFlash,
  renderFlash,
  escapeHtml,
  setActiveNav,
  idStr,
  saveSettings,
  refreshFoot,
} from "./shared.js";

import { renderJournal } from "./journal.js";

export async function renderBank() {
  const company = ui.settings.company.trim();
  const companyHint = company
    ? ""
    : `<p class="muted">Sæt firmasti under Indstillinger før preview.</p>`;
  const rd = ui.bankReconcileDraft || {};

  let resultHtml = "";
  if (ui.bankLastResult) {
    const drafts = Array.isArray(ui.bankLastResult.drafts) ? ui.bankLastResult.drafts : [];
    const errors = Array.isArray(ui.bankLastResult.errors) ? ui.bankLastResult.errors : [];
    const draftRows = drafts
      .map((d, idx) => {
        const minor =
          typeof d.amount_minor === "number"
            ? d.amount_minor
            : typeof d.amount?.units === "number"
              ? d.amount.units
              : null;
        const memo = d.memo || d.text || "";
        const date = d.date || d.booking_date || "";
        return `<tr>
          <td class="money">${idx}</td>
          <td>${escapeHtml(date)}</td>
          <td>${escapeHtml(memo)}</td>
          <td class="money">${escapeHtml(formatDkk(minor))}</td>
          <td class="money">${escapeHtml(minor == null ? "—" : String(minor))}</td>
          <td class="actions-cell">
            <button type="button" class="ghost bank-fill-row" data-idx="${idx}"
              data-date="${escapeHtml(date)}" data-text="${escapeHtml(memo)}"
              data-amount="${escapeHtml(minor == null ? "" : String(minor))}">Udfyld afstem</button>
          </td>
        </tr>`;
      })
      .join("");
    const errList = errors.map((e) => `<li>${escapeHtml(e)}</li>`).join("");
    resultHtml = `
      <div class="panel nested">
        <h2>Seneste import-preview</h2>
        <p class="muted">${escapeHtml(String(ui.bankLastResult.count ?? drafts.length))} udkast ·
          ${escapeHtml(ui.bankLastResult.source || "—")} /
          ${escapeHtml(ui.bankLastResult.provider || "—")}</p>
        ${
          errList
            ? `<ul class="flash err" style="list-style:disc;padding-left:1.25rem">${errList}</ul>`
            : ""
        }
        <table class="table">
          <thead><tr><th>#</th><th>Dato</th><th>Memo</th><th>DKK</th><th>Øre</th><th></th></tr></thead>
          <tbody>${
            draftRows || `<tr><td colspan="6" class="muted">Ingen udkast</td></tr>`
          }</tbody>
        </table>
      </div>`;
  }

  let suggestHtml = "";
  if (ui.bankSuggestLast) {
    const rows = Array.isArray(ui.bankSuggestLast.rows) ? ui.bankSuggestLast.rows : [];
    const matchRows = rows
      .map((r) => {
        const suggestions = Array.isArray(r.suggestions) ? r.suggestions : [];
        const best = suggestions[0];
        const sugList = suggestions
          .map(
            (s) =>
              `<li><span class="money">${escapeHtml(s.invoice_id || "")}</span>
               · ${escapeHtml(String(s.confidence_bps ?? "—"))} bps
               · ${escapeHtml(s.party_name || s.kind || "")}</li>`,
          )
          .join("");
        const fillBtn = best
          ? `<button type="button" class="primary bank-fill-suggest"
              data-invoice="${escapeHtml(best.invoice_id || "")}"
              data-date="${escapeHtml(r.date || "")}"
              data-text="${escapeHtml(r.text || "")}"
              data-amount="${escapeHtml(String(r.amount_minor ?? ""))}"
              data-bps="${escapeHtml(String(best.confidence_bps ?? ""))}">Udfyld apply</button>`
          : `<span class="muted">Ingen forslag${
              r.unsafe_match_reason
                ? ` · ${escapeHtml(r.unsafe_match_reason)}`
                : ""
            }</span>`;
        return `<tr>
          <td class="money">${escapeHtml(String(r.row_index ?? ""))}</td>
          <td>${escapeHtml(r.date || "")}</td>
          <td>${escapeHtml(r.text || "")}</td>
          <td class="money">${escapeHtml(formatDkk(r.amount_minor))}</td>
          <td><ul class="plain">${sugList || "<li class='muted'>—</li>"}</ul>${fillBtn}</td>
        </tr>`;
      })
      .join("");
    const raised = Array.isArray(ui.bankSuggestLast.exceptions_raised)
      ? ui.bankSuggestLast.exceptions_raised
      : [];
    suggestHtml = `
      <div class="panel nested">
        <h2>Afstem-forslag</h2>
        <p class="muted">${escapeHtml(String(ui.bankSuggestLast.count ?? rows.length))} banklinjer ·
          safe ≥ 5000 bps · poster ikke</p>
        ${
          raised.length
            ? `<p class="muted">exceptions_raised: ${raised.map((x) => escapeHtml(x)).join(", ")}</p>`
            : ""
        }
        <table class="table">
          <thead><tr><th>#</th><th>Dato</th><th>Tekst</th><th>DKK</th><th>Forslag</th></tr></thead>
          <tbody>${
            matchRows || `<tr><td colspan="5" class="muted">Ingen matches</td></tr>`
          }</tbody>
        </table>
      </div>`;
  }

  const pay = ui.journalPending;
  const payBlock =
    pay && pay.from_bank
      ? `<div class="panel nested">
        <h2>Afstem-preview klar</h2>
        <p class="muted">Faktura <span class="money">${escapeHtml(pay.from_bank)}</span> — poster ikke før commit.</p>
        <p class="muted">confirm_token</p>
        <p class="token money">${escapeHtml(pay.confirm_token)}</p>
        <p class="muted">Udløber (unix ms): ${escapeHtml(String(pay.expires_unix_ms ?? "—"))}</p>
        <p class="muted">Digest: <span class="money">${escapeHtml(pay.payload_digest || "—")}</span></p>
        <div class="actions">
          <button class="primary" type="button" id="bank-journal-commit" ${company ? "" : "disabled"}>Commit journal</button>
          <button class="ghost" type="button" id="bank-goto-journal">Åbn Journal</button>
          <button class="ghost" type="button" id="bank-clear-preview">Ryd token</button>
        </div>
      </div>`
      : `<p class="muted">Afstem apply med <span class="money">preview:true</span> giver ConfirmStore-token (ingen auto-post).</p>`;

  ui.app.innerHTML = `
    ${renderFlash()}
    <section class="panel">
      <h1>Bank</h1>
      <p class="lede">Import → foreslå matches → apply preview → commit. Beløb i DKK-visning; API bruger øre (i64).</p>
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
              <option value="revolut">revolut (dormant uden token)</option>
              <option value="stripe">stripe</option>
            </select>
          </label>
        </div>
        <div class="csv-block" id="bank-csv-block">
          <label>CSV (indsæt)
            <textarea name="csv" id="bank-csv" placeholder="Dato;Tekst;Beløb&#10;19.08.2026;Kontor;-125,50">${escapeHtml(ui.bankLastResult?.csv || "")}</textarea>
          </label>
        </div>
        <p class="muted">Firma: <span class="money">${escapeHtml(company || "—")}</span>. API kræver provider-nøgler i server-miljø.</p>
        <div class="actions">
          <button class="primary" type="submit" ${company ? "" : "disabled"}>Preview import</button>
          <button class="ghost" type="button" id="bank-suggest-btn" ${company ? "" : "disabled"}>Foreslå matches</button>
        </div>
      </form>
      ${resultHtml}
      ${suggestHtml}

      <div class="panel nested">
        <h2>Afstem apply (preview)</h2>
        <p class="muted">POST /api/v1/bank/reconcile/apply med preview:true — journalforslag, aldrig auto-post.</p>
        <form id="bank-reconcile-form" class="grid">
          <label>invoice_id
            <input name="invoice_id" required placeholder="inv_…" value="${escapeHtml(rd.invoice_id || "")}" />
          </label>
          <div class="grid two">
            <label>Dato (YYYY-MM-DD)
              <input name="date" required placeholder="2026-05-20" value="${escapeHtml(rd.date || "2026-05-20")}" />
            </label>
            <label>amount_minor (øre)
              <input name="amount_minor" required inputmode="numeric" pattern="-?[0-9]+" value="${escapeHtml(rd.amount_minor || "50000")}" />
            </label>
          </div>
          <label>Tekst
            <input name="text" required placeholder="Customer payment …" value="${escapeHtml(rd.text || "")}" />
          </label>
          <label class="check">
            <input type="checkbox" name="force" ${rd.force ? "checked" : ""} />
            force (kun user under safe-threshold)
          </label>
          <div class="actions">
            <button class="primary" type="submit" ${company ? "" : "disabled"}>Afstem preview</button>
          </div>
        </form>
      </div>
      ${payBlock}
    </section>
  `;

  if (ui.bankLastResult?.source) {
    const src = document.getElementById("bank-source");
    if (src) src.value = ui.bankLastResult.source;
  }
  if (ui.bankLastResult?.provider) {
    const prov = document.querySelector('#bank-preview-form select[name="provider"]');
    if (prov) prov.value = ui.bankLastResult.provider;
  }

  const sourceSel = document.getElementById("bank-source");
  const csvBlock = document.getElementById("bank-csv-block");
  const syncCsv = () => {
    if (csvBlock) csvBlock.hidden = sourceSel?.value === "api";
  };
  sourceSel?.addEventListener("change", syncCsv);
  syncCsv();

  document.querySelectorAll(".bank-fill-row").forEach((btn) => {
    btn.addEventListener("click", async () => {
      ui.bankReconcileDraft = {
        ...(ui.bankReconcileDraft || {}),
        date: btn.dataset.date || "",
        text: btn.dataset.text || "",
        amount_minor: btn.dataset.amount || "",
      };
      setFlash("ok", `Udkast #${btn.dataset.idx} udfyldt — sæt invoice_id og kør Afstem preview`);
      await renderBank();
    });
  });

  document.querySelectorAll(".bank-fill-suggest").forEach((btn) => {
    btn.addEventListener("click", async () => {
      const bps = Number(btn.dataset.bps || "0");
      ui.bankReconcileDraft = {
        invoice_id: btn.dataset.invoice || "",
        date: btn.dataset.date || "",
        text: btn.dataset.text || "",
        amount_minor: btn.dataset.amount || "",
        force: bps > 0 && bps < 5000,
      };
      setFlash(
        "ok",
        `Match ${btn.dataset.invoice} udfyldt (${btn.dataset.bps} bps)${
          ui.bankReconcileDraft.force ? " — force sat (under 5000)" : ""
        }`,
      );
      await renderBank();
    });
  });

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
      const env = await api(ui.settings, "/api/v1/bank/import/preview", {
        method: "POST",
        body: JSON.stringify(body),
      });
      const data = env.data || {};
      ui.bankLastResult = {
        drafts: Array.isArray(data.drafts) ? data.drafts : [],
        errors: Array.isArray(env.errors) ? env.errors : [],
        count: data.count,
        source: data.source || source,
        provider: data.provider || provider,
        csv,
      };
      setFlash("ok", `Preview OK · ${ui.bankLastResult.count ?? ui.bankLastResult.drafts.length} udkast`);
      await renderBank();
    } catch (e) {
      ui.bankLastResult = {
        drafts: [],
        errors: [e.message],
        count: 0,
        source,
        provider,
        csv,
      };
      setFlash("err", e.message);
      await renderBank();
    }
  });

  document.getElementById("bank-suggest-btn")?.addEventListener("click", async () => {
    if (!company) {
      setFlash("err", "Firmasti mangler");
      await renderBank();
      return;
    }
    const form = document.getElementById("bank-preview-form");
    const fd = new FormData(form);
    const source = String(fd.get("source") || "csv");
    const provider = String(fd.get("provider") || "generic_dk");
    const csv = String(fd.get("csv") || "");
    const body = {
      company,
      source,
      provider,
      currency: "DKK",
      raise_exceptions: true,
    };
    if (source === "csv") body.csv = csv;
    try {
      const env = await api(ui.settings, "/api/v1/bank/reconcile/suggest", {
        method: "POST",
        body: JSON.stringify(body),
      });
      const data = env.data || {};
      ui.bankSuggestLast = {
        count: data.count,
        rows: Array.isArray(data.rows) ? data.rows : [],
        exceptions_raised: Array.isArray(data.exceptions_raised) ? data.exceptions_raised : [],
      };
      ui.bankLastResult = {
        ...(ui.bankLastResult || {}),
        source,
        provider,
        csv,
      };
      setFlash(
        "ok",
        `Forslag OK · ${ui.bankSuggestLast.count ?? ui.bankSuggestLast.rows.length} linjer (poster ikke)`,
      );
      await renderBank();
    } catch (e) {
      setFlash("err", e.message);
      await renderBank();
    }
  });

  document.getElementById("bank-reconcile-form")?.addEventListener("submit", async (ev) => {
    ev.preventDefault();
    if (!company) {
      setFlash("err", "Firmasti mangler");
      await renderBank();
      return;
    }
    const fd = new FormData(ev.target);
    try {
      const invoice_id = String(fd.get("invoice_id") || "").trim();
      const date = String(fd.get("date") || "").trim();
      const text = String(fd.get("text") || "").trim();
      const amount_minor = parseMinor(fd.get("amount_minor"), "amount_minor");
      const force = Boolean(fd.get("force"));
      if (!invoice_id) throw new Error("invoice_id kræves");
      if (!date || !text) throw new Error("dato og tekst kræves");
      ui.bankReconcileDraft = {
        invoice_id,
        date,
        text,
        amount_minor: String(amount_minor),
        force,
      };
      const env = await api(ui.settings, "/api/v1/bank/reconcile/apply", {
        method: "POST",
        body: JSON.stringify({
          company,
          invoice_id,
          preview: true,
          force,
          currency: "DKK",
          row: { date, text, amount_minor },
        }),
      });
      const data = env.data || {};
      if (!data.confirm_token || !data.entry) {
        throw new Error("Mangler confirm_token eller entry i afstem-preview");
      }
      ui.journalPending = {
        entry: data.entry,
        confirm_token: String(data.confirm_token),
        expires_unix_ms: data.expires_unix_ms,
        payload_digest: data.payload_digest,
        from_bank: invoice_id,
      };
      setFlash(
        "ok",
        `Afstem preview OK · confidence_bps=${data.confidence_bps ?? "—"} · forced=${data.forced === true}`,
      );
      await renderBank();
    } catch (e) {
      setFlash("err", e.message);
      await renderBank();
    }
  });

  document.getElementById("bank-clear-preview")?.addEventListener("click", async () => {
    ui.journalPending = null;
    setFlash("ok", "Token ryddet");
    await renderBank();
  });

  document.getElementById("bank-goto-journal")?.addEventListener("click", async () => {
    ui.view = "journal";
    setActiveNav();
    await renderJournal();
  });

  document.getElementById("bank-journal-commit")?.addEventListener("click", async () => {
    if (!company || !ui.journalPending) return;
    try {
      const env = await api(ui.settings, "/api/v1/journal/commit", {
        method: "POST",
        body: JSON.stringify({
          company,
          entry: ui.journalPending.entry,
          confirm_token: ui.journalPending.confirm_token,
        }),
      });
      const data = env.data || {};
      ui.journalPending = null;
      setFlash(
        "ok",
        `Posted ${data.id || "ok"} · digest ${String(data.digest || "").slice(0, 16)}…`,
      );
      await renderBank();
    } catch (e) {
      setFlash("err", e.message);
      await renderBank();
    }
  });
}


/** @type {object | null} */
