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

export async function renderInvoices() {
  const company = ui.settings.company.trim();
  let body = `<p class="muted">Sæt firmasti under Indstillinger.</p>`;
  let partyOptions = `<option value="">— vælg part —</option>`;

  if (company) {
    try {
      const q = new URLSearchParams({ company });
      const [invEnv, partyEnv] = await Promise.all([
        api(ui.settings, `/api/v1/invoices/drafts?${q}`),
        api(ui.settings, `/api/v1/crm/parties?${q}`),
      ]);
      const invoices = Array.isArray(invEnv.data) ? invEnv.data : invEnv.data?.invoices || [];
      const parties = Array.isArray(partyEnv.data)
        ? partyEnv.data
        : partyEnv.data?.parties || [];
      partyOptions += parties
        .map((p) => {
          const id = p.party_id || p.id || "";
          const name = p.display_name || id;
          return `<option value="${escapeHtml(id)}">${escapeHtml(name)} (${escapeHtml(id)})</option>`;
        })
        .join("");
      const rows = invoices
        .map((inv) => {
          const id = inv.invoice_id || inv.id || "";
          const total = inv.total_minor ?? inv.amount_minor ?? null;
          const status = inv.status || "";
          const canCollect = status === "sent" || status === "part_paid";
          const canSend = status === "draft";
          return `<tr data-invoice-id="${escapeHtml(id)}">
            <td class="money">${escapeHtml(id)}</td>
            <td>${escapeHtml(status)}</td>
            <td class="money">${escapeHtml(formatDkk(total))}</td>
            <td class="money">${escapeHtml(inv.party_id || "")}</td>
            <td class="actions-cell">
              ${
                canSend
                  ? `<button type="button" class="ghost inv-send" data-id="${escapeHtml(id)}">Sæt sent</button>`
                  : ""
              }
              ${
                canCollect
                  ? `<button type="button" class="ghost inv-part" data-id="${escapeHtml(id)}">Delbetalt preview</button>
                     <button type="button" class="primary inv-paid" data-id="${escapeHtml(id)}">Betalt preview</button>`
                  : ""
              }
            </td>
          </tr>`;
        })
        .join("");
      body = `
        <table class="table">
          <thead><tr><th>Id</th><th>Status</th><th>Beløb</th><th>Part</th><th>Handling</th></tr></thead>
          <tbody>${rows || `<tr><td colspan="5" class="muted">Ingen fakturaer</td></tr>`}</tbody>
        </table>

        <div class="panel nested">
          <h2>Ny kladde</h2>
          <p class="muted">POST /api/v1/invoices/drafts — beløb i øre (i64). Returnerer journalforslag; poster ikke.</p>
          <form id="invoice-draft-form" class="grid">
            <div class="grid two">
              <label>Part
                <select name="party_id" required>${partyOptions}</select>
              </label>
              <label>Type
                <select name="kind">
                  <option value="sale" selected>sale</option>
                  <option value="purchase">purchase</option>
                </select>
              </label>
            </div>
            <label>Linje-beskrivelse
              <input name="description" required placeholder="Consulting" value="Consulting" />
            </label>
            <label>amount_minor (øre)
              <input name="amount_minor" required inputmode="numeric" pattern="-?[0-9]+" value="12500" />
            </label>
            <div class="actions">
              <button class="primary" type="submit">Opret kladde</button>
            </div>
          </form>
        </div>

        <div class="panel nested">
          <h2>Delbetaling (øre)</h2>
          <p class="muted">Bruges af «Delbetalt preview». Beløb skal være &gt; 0 og &lt; resterende (i64).</p>
          <label>amount_minor
            <input id="inv-part-amount" inputmode="numeric" pattern="-?[0-9]+" value="2000" />
          </label>
        </div>
      `;
    } catch (e) {
      body = `<p class="flash err">${escapeHtml(e.message)}</p>`;
    }
  }

  const pay = ui.journalPending;
  const payBlock =
    pay && pay.from_invoice
      ? `<div class="panel nested">
        <h2>Betalings-preview klar</h2>
        <p class="muted">Fra faktura <span class="money">${escapeHtml(pay.from_invoice)}</span> — poster ikke før commit.</p>
        <p class="muted">confirm_token</p>
        <p class="token money">${escapeHtml(pay.confirm_token)}</p>
        <p class="muted">Udløber (unix ms): ${escapeHtml(String(pay.expires_unix_ms ?? "—"))}</p>
        <p class="muted">Digest: <span class="money">${escapeHtml(pay.payload_digest || "—")}</span></p>
        <div class="actions">
          <button class="primary" type="button" id="inv-journal-commit" ${company ? "" : "disabled"}>Commit journal</button>
          <button class="ghost" type="button" id="inv-goto-journal">Åbn Journal</button>
          <button class="ghost" type="button" id="inv-clear-preview">Ryd token</button>
        </div>
      </div>`
      : `<p class="muted">Mark-paid / mark-part-paid med <span class="money">preview:true</span> giver ConfirmStore-token (ingen auto-post).</p>`;

  ui.app.innerHTML = `
    ${renderFlash()}
    <section class="panel">
      <h1>Fakturaer</h1>
      <p class="lede">Beløb i kroner (visning); API bruger øre (i64). Kladde + betaling → journalforslag, aldrig auto-post.</p>
      ${body}
      ${payBlock}
    </section>
  `;

  document.getElementById("invoice-draft-form")?.addEventListener("submit", async (ev) => {
    ev.preventDefault();
    if (!company) return;
    const fd = new FormData(ev.target);
    try {
      const party_id = String(fd.get("party_id") || "").trim();
      const kind = String(fd.get("kind") || "sale");
      const description = String(fd.get("description") || "").trim();
      const amount_minor = parseMinor(fd.get("amount_minor"), "amount_minor");
      if (!party_id) throw new Error("Vælg en part");
      if (!description) throw new Error("Beskrivelse kræves");
      if (amount_minor <= 0) throw new Error("amount_minor skal være > 0");
      const env = await api(ui.settings, "/api/v1/invoices/drafts", {
        method: "POST",
        body: JSON.stringify({
          company,
          party_id,
          kind,
          lines: [{ description, amount_minor, currency: "DKK" }],
        }),
      });
      const data = env.data || {};
      const inv = data.invoice || {};
      const id = inv.invoice_id || inv.id || "?";
      setFlash(
        "ok",
        `Kladde ${id} oprettet · total ${formatDkk(inv.total_minor ?? amount_minor)} (journalforslag uden post)`,
      );
      await renderInvoices();
    } catch (e) {
      setFlash("err", e.message);
      await renderInvoices();
    }
  });

  document.querySelectorAll(".inv-send").forEach((btn) => {
    btn.addEventListener("click", async () => {
      if (!company) return;
      try {
        await api(ui.settings, "/api/v1/invoices/status", {
          method: "PATCH",
          body: JSON.stringify({
            company,
            invoice_id: btn.dataset.id,
            status: "sent",
          }),
        });
        setFlash("ok", `Faktura ${btn.dataset.id} → sent`);
        await renderInvoices();
      } catch (e) {
        setFlash("err", e.message);
        await renderInvoices();
      }
    });
  });

  const runPayPreview = async (path, invoiceId, extra) => {
    if (!company) return;
    try {
      const env = await api(ui.settings, path, {
        method: "POST",
        body: JSON.stringify({
          company,
          invoice_id: invoiceId,
          preview: true,
          ...extra,
        }),
      });
      const data = env.data || {};
      if (!data.confirm_token || !data.journal_entry) {
        throw new Error("Mangler confirm_token eller journal_entry i preview");
      }
      ui.journalPending = {
        entry: data.journal_entry,
        confirm_token: String(data.confirm_token),
        expires_unix_ms: data.expires_unix_ms,
        payload_digest: data.payload_digest,
        from_invoice: invoiceId,
      };
      setFlash("ok", `Preview OK for ${invoiceId} — token klar (poster ikke)`);
      await renderInvoices();
    } catch (e) {
      setFlash("err", e.message);
      await renderInvoices();
    }
  };

  document.querySelectorAll(".inv-paid").forEach((btn) => {
    btn.addEventListener("click", () =>
      runPayPreview("/api/v1/invoices/mark-paid", btn.dataset.id, {}),
    );
  });

  document.querySelectorAll(".inv-part").forEach((btn) => {
    btn.addEventListener("click", async () => {
      try {
        const raw = document.getElementById("inv-part-amount")?.value;
        const amount_minor = parseMinor(raw, "Delbetaling");
        await runPayPreview("/api/v1/invoices/mark-part-paid", btn.dataset.id, {
          amount_minor,
        });
      } catch (e) {
        setFlash("err", e.message);
        await renderInvoices();
      }
    });
  });

  document.getElementById("inv-clear-preview")?.addEventListener("click", async () => {
    ui.journalPending = null;
    setFlash("ok", "Token ryddet");
    await renderInvoices();
  });

  document.getElementById("inv-goto-journal")?.addEventListener("click", async () => {
    ui.view = "journal";
    setActiveNav();
    await renderJournal();
  });

  document.getElementById("inv-journal-commit")?.addEventListener("click", async () => {
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
      await renderInvoices();
    } catch (e) {
      setFlash("err", e.message);
      await renderInvoices();
    }
  });
}


