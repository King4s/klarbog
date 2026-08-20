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

export async function renderChart() {
  const company = ui.settings.company.trim();
  let body = `<p class="muted">Sæt firmasti under Indstillinger.</p>`;
  if (company) {
    try {
      const q = new URLSearchParams({ company });
      const env = await api(ui.settings, `/api/v1/rules/chart?${q}`);
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

  ui.app.innerHTML = `
    ${renderFlash()}
    <section class="panel">
      <h1>Kontoplan</h1>
      <p class="lede">GET /api/v1/rules/chart — kun læsning, ingen journal-skrivning.</p>
      ${body}
    </section>
  `;
}

