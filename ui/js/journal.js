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

function journalFormDefaults() {
  const fromPending = ui.journalPending?.entry;
  const d = ui.journalDraft || {};
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

export async function renderJournal() {
  const company = ui.settings.company.trim();
  const pending = ui.journalPending;
  const defs = journalFormDefaults();
  let companyHint = company
    ? ""
    : `<p class="muted">Sæt firmasti under Indstillinger før preview/commit.</p>`;

  const moms = ui.journalMomsLast;
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

  ui.app.innerHTML = `
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
      ui.journalDraft = {
        ...(ui.journalDraft || {}),
        momsGross: String(gross),
        momsMemo: memo,
      };
      const env = await api(ui.settings, "/api/v1/journal/moms-suggest", {
        method: "POST",
        body: JSON.stringify({ company, gross_minor: gross, memo }),
      });
      ui.journalMomsLast = env.data || { suggested: false };
      if (ui.journalMomsLast.suggested) {
        setFlash(
          "ok",
          `Moms-forslag: netto ${ui.journalMomsLast.net_minor} / moms ${ui.journalMomsLast.vat_minor} øre (poster ikke)`,
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
    if (!ui.journalMomsLast?.suggested) return;
    const gross = String(ui.journalMomsLast.gross_minor);
    const memoEl = document.querySelector('#moms-suggest-form input[name="moms_memo"]');
    const memo = memoEl ? String(memoEl.value || "").trim() : defs.momsMemo;
    ui.journalDraft = {
      ...(ui.journalDraft || {}),
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
    ui.journalMomsLast = null;
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
      ui.journalDraft = {
        ...(ui.journalDraft || {}),
        memo: String(fd.get("memo") || ""),
        account1: String(fd.get("account1") || ""),
        direction1: String(fd.get("direction1") || "debit"),
        amount1: String(fd.get("amount1") || ""),
        account2: String(fd.get("account2") || ""),
        direction2: String(fd.get("direction2") || "credit"),
        amount2: String(fd.get("amount2") || ""),
      };
      const entry = buildJournalEntry(fd);
      const env = await api(ui.settings, "/api/v1/journal/preview", {
        method: "POST",
        body: JSON.stringify({ company, entry }),
      });
      const data = env.data || {};
      if (!data.confirm_token) throw new Error("Intet confirm_token i svar");
      ui.journalPending = {
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
    ui.journalPending = null;
    setFlash("ok", "Token ryddet");
    await renderJournal();
  });

  document.getElementById("journal-commit")?.addEventListener("click", async () => {
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
      ui.journalDraft = null;
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


