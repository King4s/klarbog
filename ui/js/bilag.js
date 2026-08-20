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

export async function renderBilag() {
  const company = ui.settings.company.trim();
  let docsBody = `<p class="muted">Sæt firmasti under Indstillinger.</p>`;
  let excBody = "";

  if (company) {
    try {
      const q = new URLSearchParams({ company });
      const [docsEnv, excEnv] = await Promise.all([
        api(ui.settings, `/api/v1/documents?${q}`),
        api(ui.settings, `/api/v1/exceptions?${q}`),
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

  const purge = ui.retentionPurgeLast;
  let purgeHtml = `
    <div class="panel nested">
      <h2>Retention purge</h2>
      <p class="muted">POST /api/v1/retention/purge — lukkede undtagelser; journal urørt (ADR-010).</p>
      <label class="check">
        <input type="checkbox" id="purge-gc-orphans" />
        gc_orphan_documents
      </label>
      <div class="actions" style="margin-top:0.75rem">
        <button class="primary" type="button" id="purge-dry-btn" ${company ? "" : "disabled"}>Dry-run purge</button>
        <button class="ghost" type="button" id="backup-btn" ${company ? "" : "disabled"}>Backup-manifest</button>
      </div>
    </div>`;
  if (purge) {
    const purged = Array.isArray(purge.exceptions_purged) ? purge.exceptions_purged : [];
    const orphans = Array.isArray(purge.orphan_documents_gc) ? purge.orphan_documents_gc : [];
    purgeHtml += `
      <div class="panel nested">
        <h2>Purge-rapport</h2>
        <p class="muted">dry_run=${escapeHtml(String(purge.dry_run))} · after_days=${escapeHtml(
          String(purge.purge_closed_exceptions_after_days ?? "—"),
        )}</p>
        <ul class="plain">
          <li>exceptions_purged (${purged.length}):
            <span class="money">${escapeHtml(purged.slice(0, 8).join(", ") || "—")}${
              purged.length > 8 ? "…" : ""
            }</span></li>
          <li>orphan_documents_gc (${orphans.length}):
            <span class="money">${escapeHtml(orphans.slice(0, 8).join(", ") || "—")}${
              orphans.length > 8 ? "…" : ""
            }</span></li>
        </ul>
        ${
          purge.dry_run
            ? `<div class="actions">
                <button class="primary" type="button" id="purge-confirm-btn">Bekræft purge</button>
                <button class="ghost" type="button" id="purge-clear-btn">Ryd rapport</button>
              </div>`
            : `<div class="actions">
                <button class="ghost" type="button" id="purge-clear-btn">Ryd rapport</button>
              </div>`
        }
      </div>`;
  }

  ui.app.innerHTML = `
    ${renderFlash()}
    <section class="panel">
      <h1>Bilag</h1>
      <p class="lede">Dokumenter, undtagelser og retention purge — ingen journal-skrivning.</p>
      ${docsBody}
      ${excBody}
      ${purgeHtml}
    </section>
  `;

  document.getElementById("exception-form")?.addEventListener("submit", async (ev) => {
    ev.preventDefault();
    const fd = new FormData(ev.target);
    try {
      await api(ui.settings, "/api/v1/exceptions", {
        method: "POST",
        body: JSON.stringify({
          company: ui.settings.company.trim(),
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

  const runPurge = async (confirm) => {
    if (!company) return;
    const gc_orphan_documents = Boolean(
      document.getElementById("purge-gc-orphans")?.checked,
    );
    try {
      const env = await api(ui.settings, "/api/v1/retention/purge", {
        method: "POST",
        body: JSON.stringify({
          company,
          confirm: Boolean(confirm),
          gc_orphan_documents,
        }),
      });
      ui.retentionPurgeLast = env.data || null;
      setFlash(
        "ok",
        confirm
          ? "Purge bekræftet (journal urørt)"
          : "Purge dry-run OK — inspicer rapport før bekræft",
      );
      await renderBilag();
    } catch (e) {
      setFlash("err", e.message);
      await renderBilag();
    }
  };

  document.getElementById("purge-dry-btn")?.addEventListener("click", () => runPurge(false));
  document.getElementById("purge-confirm-btn")?.addEventListener("click", async () => {
    if (!window.confirm("Bekræft retention purge? Lukkede undtagelser fjernes. Journal røres ikke."))
      return;
    await runPurge(true);
  });
  document.getElementById("purge-clear-btn")?.addEventListener("click", async () => {
    ui.retentionPurgeLast = null;
    setFlash("ok", "Purge-rapport ryddet");
    await renderBilag();
  });

  document.getElementById("backup-btn")?.addEventListener("click", async () => {
    if (!company) return;
    try {
      const env = await api(ui.settings, "/api/v1/backup", {
        method: "POST",
        body: JSON.stringify({ company }),
      });
      const data = env.data || {};
      setFlash(
        "ok",
        `Backup-manifest${data.backup_key ? `: ${data.backup_key}` : " OK"} (operator-owned; ikke HA-SLA)`,
      );
    } catch (e) {
      setFlash("err", e.message);
      await renderBilag();
    }
  });
}


