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

export async function renderHome() {
  let statusHtml = `<p class="muted">Indlæser status…</p>`;
  try {
    const env = await api(ui.settings, "/api/v1/status");
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

  ui.app.innerHTML = `
    ${renderFlash()}
    <section class="panel">
      <h1>Oversigt</h1>
      <p class="lede">Klarbog kører lokalt. API og UI deler samme loopback-origin.</p>
      ${statusHtml}
    </section>
  `;
}

/** @type {object | null} */
