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

export async function renderParties() {
  const company = ui.settings.company.trim();
  let body = `<p class="muted">Sæt firmasti under Indstillinger.</p>`;
  if (company) {
    try {
      const q = new URLSearchParams({ company });
      const env = await api(ui.settings, `/api/v1/crm/parties?${q}`);
      const parties = Array.isArray(env.data) ? env.data : env.data?.parties || [];
      const rows = parties
        .map((p) => {
          const id = p.party_id || p.id || "";
          return `<tr>
            <td class="money">${escapeHtml(id)}</td>
            <td>${escapeHtml(p.display_name || "")}</td>
            <td class="actions-cell">
              <button type="button" class="ghost gdpr-dry" data-id="${escapeHtml(id)}">GDPR dry-run</button>
            </td>
          </tr>`;
        })
        .join("");
      body = `
        <table class="table">
          <thead><tr><th>Id</th><th>Navn</th><th>GDPR</th></tr></thead>
          <tbody>${rows || `<tr><td colspan="3" class="muted">Ingen parter</td></tr>`}</tbody>
        </table>
        <form id="party-form" class="grid" style="margin-top:1.25rem">
          <label>Nyt partenavn
            <input name="display_name" required placeholder="Kunde ApS" />
          </label>
          <div class="actions">
            <button class="primary" type="submit">Opret part</button>
            <button class="ghost" type="button" id="gdpr-export-btn">GDPR-eksport</button>
          </div>
        </form>
        <label class="check" style="margin-top:0.75rem">
          <input type="checkbox" id="gdpr-delete-docs" />
          Ved erase: slet dokumenter (ellers strip party_id)
        </label>
      `;
    } catch (e) {
      body = `<p class="flash err">${escapeHtml(e.message)}</p>`;
    }
  }

  const report = ui.gdprEraseLast;
  let reportHtml = `<p class="muted">GDPR erase: altid dry-run først. Journal er uforanderlig (journal_refs_retained).</p>`;
  if (report) {
    const refs = Array.isArray(report.journal_refs_retained)
      ? report.journal_refs_retained
      : [];
    const stripped = Array.isArray(report.documents_stripped)
      ? report.documents_stripped
      : [];
    const deleted = Array.isArray(report.documents_deleted)
      ? report.documents_deleted
      : [];
    reportHtml = `
      <div class="panel nested">
        <h2>GDPR erase-rapport</h2>
        <p class="muted">dry_run=${escapeHtml(String(report.dry_run))} · party
          <span class="money">${escapeHtml(report.party_id || "")}</span></p>
        <ul class="plain">
          <li>Navn før → efter: ${escapeHtml(report.display_name_before || "")}
            → ${escapeHtml(report.display_name_after || "")}</li>
          <li>Dokumenter strip: ${stripped.length}</li>
          <li>Dokumenter slet: ${deleted.length}</li>
          <li>journal_refs_retained (${refs.length}):
            <span class="money">${escapeHtml(refs.slice(0, 8).join(", ") || "—")}${
              refs.length > 8 ? "…" : ""
            }</span></li>
        </ul>
        ${
          report.dry_run
            ? `<div class="actions">
                <button class="primary" type="button" id="gdpr-confirm-btn"
                  data-id="${escapeHtml(report.party_id || "")}">Bekræft erase</button>
                <button class="ghost" type="button" id="gdpr-clear-report">Ryd rapport</button>
              </div>
              <p class="muted">Bekræft sætter display_name → erased. Journal røres ikke.</p>`
            : `<div class="actions">
                <button class="ghost" type="button" id="gdpr-clear-report">Ryd rapport</button>
              </div>`
        }
      </div>`;
  }

  ui.app.innerHTML = `
    ${renderFlash()}
    <section class="panel">
      <h1>Parter</h1>
      <p class="lede">CRM + GDPR erase (dry-run → confirm). Journal skrives aldrig her.</p>
      ${body}
      ${reportHtml}
    </section>
  `;

  document.getElementById("party-form")?.addEventListener("submit", async (ev) => {
    ev.preventDefault();
    const fd = new FormData(ev.target);
    try {
      await api(ui.settings, "/api/v1/crm/parties", {
        method: "POST",
        body: JSON.stringify({
          company: ui.settings.company.trim(),
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

  const runErase = async (partyId, confirm) => {
    if (!company || !partyId) return;
    const delete_documents = Boolean(document.getElementById("gdpr-delete-docs")?.checked);
    try {
      const env = await api(ui.settings, "/api/v1/gdpr/erase-party", {
        method: "POST",
        body: JSON.stringify({
          company,
          party_id: partyId,
          confirm: Boolean(confirm),
          delete_documents,
        }),
      });
      ui.gdprEraseLast = env.data || null;
      setFlash(
        "ok",
        confirm
          ? `Erase bekræftet for ${partyId} (journal urørt)`
          : `Dry-run OK for ${partyId} — inspicer journal_refs_retained`,
      );
      await renderParties();
    } catch (e) {
      setFlash("err", e.message);
      await renderParties();
    }
  };

  document.querySelectorAll(".gdpr-dry").forEach((btn) => {
    btn.addEventListener("click", () => runErase(btn.dataset.id, false));
  });

  document.getElementById("gdpr-confirm-btn")?.addEventListener("click", async (ev) => {
    const id = ev.currentTarget.dataset.id;
    if (!window.confirm(`Bekræft GDPR erase for ${id}? display_name → erased.`)) return;
    await runErase(id, true);
  });

  document.getElementById("gdpr-clear-report")?.addEventListener("click", async () => {
    ui.gdprEraseLast = null;
    setFlash("ok", "Rapport ryddet");
    await renderParties();
  });

  document.getElementById("gdpr-export-btn")?.addEventListener("click", async () => {
    if (!company) return;
    try {
      const env = await api(ui.settings, "/api/v1/gdpr-export", {
        method: "POST",
        body: JSON.stringify({ company }),
      });
      const data = env.data || {};
      setFlash(
        "ok",
        `GDPR-eksport skrevet${data.path ? `: ${data.path}` : ""} (ingen binære blobs)`,
      );
    } catch (e) {
      setFlash("err", e.message);
      await renderParties();
    }
  });
}


