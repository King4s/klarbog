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

export function renderSettings() {
  ui.app.innerHTML = `
    ${renderFlash()}
    <section class="panel">
      <h1>Indstillinger</h1>
      <p class="lede">Gemmes i browseren. Firmasti skal ligge under allowlisten.</p>
      <form id="settings-form" class="grid">
        <label>API-base (tom = samme origin)
          <input name="apiBase" value="${escapeHtml(ui.settings.apiBase)}" placeholder="http://127.0.0.1:3195" />
        </label>
        <label>Firma-sti (company)
          <input name="company" value="${escapeHtml(ui.settings.company)}" placeholder="companies/min-aps" required />
        </label>
        <div class="grid two">
          <label>Actor kind
            <select name="actorKind">
              <option value="user" ${ui.settings.actorKind === "user" ? "selected" : ""}>user</option>
              <option value="agent" ${ui.settings.actorKind === "agent" ? "selected" : ""}>agent</option>
              <option value="system" ${ui.settings.actorKind === "system" ? "selected" : ""}>system</option>
            </select>
          </label>
          <label>Actor id
            <input name="actorId" value="${escapeHtml(ui.settings.actorId)}" required />
          </label>
        </div>
        <label>API-token (kun hvis serveren har KLARBOG_API_TOKEN)
          <input name="apiToken" type="password" value="${escapeHtml(ui.settings.apiToken || "")}" autocomplete="off" placeholder="tom = ikke påkrævet" />
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
    ui.settings = {
      apiBase: String(fd.get("apiBase") || "").trim(),
      company: String(fd.get("company") || "").trim(),
      actorKind: String(fd.get("actorKind") || "user"),
      actorId: String(fd.get("actorId") || "ui-dev").trim(),
      apiToken: String(fd.get("apiToken") || "").trim(),
    };
    saveSettings(ui.settings);
    setFlash("ok", "Indstillinger gemt");
    renderSettings();
    refreshFoot();
  });
}

