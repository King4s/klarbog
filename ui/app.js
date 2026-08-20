/** Klarbog DEV UI shell — view modules soft-split (wave 51). */
import { ui, setActiveNav, setFlash, refreshFoot } from "./js/shared.js";
import { renderHome } from "./js/home.js";
import { renderParties } from "./js/parties.js";
import { renderInvoices } from "./js/invoices.js";
import { renderBank } from "./js/bank.js";
import { renderBilag } from "./js/bilag.js";
import { renderJournal } from "./js/journal.js";
import { renderChart } from "./js/chart.js";
import { renderSettings } from "./js/settings.js";

ui.app = document.getElementById("app");
ui.footStatus = document.getElementById("foot-status");

async function render() {
  setActiveNav();
  if (ui.view === "home") await renderHome();
  else if (ui.view === "parties") await renderParties();
  else if (ui.view === "invoices") await renderInvoices();
  else if (ui.view === "bank") await renderBank();
  else if (ui.view === "bilag") await renderBilag();
  else if (ui.view === "journal") await renderJournal();
  else if (ui.view === "chart") await renderChart();
  else renderSettings();
}

document.querySelectorAll(".nav-btn").forEach((btn) => {
  btn.addEventListener("click", async () => {
    ui.view = btn.dataset.view;
    ui.flash = null;
    await render();
  });
});

refreshFoot();
render();
