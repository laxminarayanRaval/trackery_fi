// Trackery UI — vanilla JS over Tauri IPC. All filtering/aggregation is
// client-side; the vault key lives only in memory for the session.

"use strict";
const invoke = window.__TAURI__.core.invoke;

const $ = (id) => document.getElementById(id);
const els = {
  lock: $("lock"), app: $("app"), pass: $("passphrase"), unlockBtn: $("unlock-btn"),
  lockError: $("lock-error"), importBtn: $("import-btn"), fileInput: $("file-input"),
  dialog: $("import-dialog"), dialogClose: $("import-close"),
  chooseFile: $("choose-file"), chosenFile: $("chosen-file"),
  pdfPassInput: $("pdf-pass-input"), importGo: $("import-go"),
  importBusy: $("import-busy"), importError: $("import-error"),
  importResult: $("import-result"), importHistory: $("import-history"),
  status: $("status-line"), kpis: $("kpis"), charts: $("charts"),
  ledger: $("ledger"), ledgerCount: $("ledger-count"), empty: $("empty"),
  tooltip: $("tooltip"),
  fFrom: $("f-from"), fTo: $("f-to"), fDir: $("f-dir"), fMode: $("f-mode"),
  fMin: $("f-min"), fMax: $("f-max"), fQ: $("f-q"), fClear: $("f-clear"),
};

let KEY = null;
let ALL = [];
let pendingBytes = null;

// ---------- formatting ----------
const INR = new Intl.NumberFormat("en-IN", { style: "currency", currency: "INR" });
let INR_COMPACT;
try {
  INR_COMPACT = new Intl.NumberFormat("en-IN", {
    style: "currency", currency: "INR", notation: "compact", maximumFractionDigits: 1,
  });
} catch { INR_COMPACT = INR; }
const rupees = (paise) => INR.format(paise / 100);
const rupeesCompact = (paise) => INR_COMPACT.format(paise / 100);
const DATE_FMT = new Intl.DateTimeFormat("en-IN", { day: "2-digit", month: "short", year: "numeric" });
const fmtDate = (iso) => DATE_FMT.format(new Date(iso + "T00:00:00"));
const monthLabel = (ym) =>
  new Date(ym + "-01T00:00:00").toLocaleDateString("en-IN", { month: "short", year: "2-digit" });
const esc = (s) =>
  String(s).replace(/[&<>"']/g, (c) =>
    ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));

// ---------- status ----------
function status(msg, isError = false) {
  els.status.hidden = !msg;
  els.status.textContent = msg || "";
  els.status.style.color = isError ? "var(--debit)" : "var(--ink-2)";
}

// ---------- unlock ----------
async function unlock() {
  const key = els.pass.value;
  if (!key) return;
  els.unlockBtn.disabled = true;
  els.lockError.hidden = true;
  try {
    ALL = await invoke("list_transactions", { dbKey: key });
    KEY = key;
    els.lock.hidden = true;
    els.app.hidden = false;
    render();
  } catch (e) {
    els.lockError.hidden = false;
    els.lockError.textContent =
      String(e) === "wrong_db_key"
        ? "That passphrase doesn't match this vault."
        : "Could not open the vault: " + e;
  } finally {
    els.unlockBtn.disabled = false;
  }
}
els.unlockBtn.addEventListener("click", unlock);
els.pass.addEventListener("keydown", (e) => { if (e.key === "Enter") unlock(); });

// ---------- import dialog ----------
function importError(msg) {
  els.importError.hidden = !msg;
  els.importError.textContent = msg || "";
}

function openImportDialog() {
  pendingBytes = null;
  els.chosenFile.textContent = "No file chosen";
  els.pdfPassInput.value = "";
  els.importGo.disabled = true;
  importError("");
  els.importResult.hidden = true;
  els.dialog.showModal();
  refreshImportHistory();
}
els.importBtn.addEventListener("click", openImportDialog);
els.dialogClose.addEventListener("click", () => els.dialog.close());

els.chooseFile.addEventListener("click", () => els.fileInput.click());
els.fileInput.addEventListener("change", async () => {
  const file = els.fileInput.files[0];
  els.fileInput.value = "";
  if (!file) return;
  pendingBytes = Array.from(new Uint8Array(await file.arrayBuffer()));
  els.chosenFile.textContent = file.name;
  els.importGo.disabled = false;
  importError("");
  els.importResult.hidden = true;
});

async function runImport() {
  if (!pendingBytes) return;
  const pw = els.pdfPassInput.value;
  els.importGo.disabled = true;
  els.importBusy.hidden = false;
  importError("");
  try {
    const s = await invoke("import_statement", {
      bytes: pendingBytes, pdfPassword: pw || null, dbKey: KEY,
    });
    pendingBytes = null;
    els.chosenFile.textContent = "No file chosen";
    els.importResult.hidden = false;
    els.importResult.textContent =
      `${s.bank.toUpperCase()}: ${s.imported} new transaction${s.imported === 1 ? "" : "s"}` +
      (s.duplicates ? `, ${s.duplicates} already in the ledger (skipped)` : "") + ".";
    ALL = await invoke("list_transactions", { dbKey: KEY });
    render();
    refreshImportHistory();
  } catch (e) {
    const err = String(e);
    els.importGo.disabled = false;
    if (err === "wrong_pdf_password") {
      importError(pw ? "That password didn't open the PDF — try again."
                     : "This statement is password-protected — enter its password.");
      els.pdfPassInput.focus();
    } else if (err === "unsupported_bank") {
      importError("This bank's format isn't supported yet. Supported: ICICI, BOB, HDFC.");
    } else if (err.startsWith("malformed_statement:")) {
      importError(`Couldn't read the statement — line ${err.split(":")[1]} doesn't reconcile. ` +
        "Tell me the bank and month and I'll fix the parser.");
    } else if (err === "corrupt_pdf") {
      importError("That file doesn't look like a readable PDF.");
    } else {
      importError("Import failed: " + err);
    }
  } finally {
    els.importBusy.hidden = true;
    if (pendingBytes) els.importGo.disabled = false;
  }
}
els.importGo.addEventListener("click", runImport);
els.pdfPassInput.addEventListener("keydown", (e) => { if (e.key === "Enter") runImport(); });

async function refreshImportHistory() {
  let history = [];
  try {
    history = await invoke("list_imports", { dbKey: KEY });
  } catch { /* history is best-effort; the dialog still works without it */ }
  if (!history.length) {
    els.importHistory.innerHTML = `<p class="muted">No imports yet.</p>`;
    return;
  }
  const when = (iso) =>
    new Date(iso).toLocaleString("en-IN", { day: "2-digit", month: "short", hour: "2-digit", minute: "2-digit" });
  els.importHistory.innerHTML = `<table>
    <tr><th>When</th><th>Device</th><th>Bank</th><th class="num">New</th><th class="num">Skipped</th></tr>` +
    history.map((h) => `<tr>
      <td>${esc(when(h.imported_at))}</td>
      <td>${esc(h.device)}</td>
      <td>${esc(h.bank.toUpperCase())}</td>
      <td class="num new">${h.new_rows}</td>
      <td class="num dup">${h.dup_rows}</td>
    </tr>`).join("") + `</table>`;
}

// ---------- filters ----------
for (const el of [els.fFrom, els.fTo, els.fDir, els.fMode, els.fMin, els.fMax]) {
  el.addEventListener("change", render);
}
els.fQ.addEventListener("input", render);
els.fClear.addEventListener("click", () => {
  for (const el of [els.fFrom, els.fTo, els.fMin, els.fMax, els.fQ]) el.value = "";
  els.fDir.value = ""; els.fMode.value = "";
  render();
});

function filteredSlice() {
  const from = els.fFrom.value, to = els.fTo.value;
  const dir = els.fDir.value, mode = els.fMode.value;
  const min = els.fMin.value ? Number(els.fMin.value) * 100 : null;
  const max = els.fMax.value ? Number(els.fMax.value) * 100 : null;
  const q = els.fQ.value.trim().toLowerCase();
  return ALL.filter((t) => {
    if (from && t.date < from) return false;
    if (to && t.date > to) return false;
    if (dir && t.direction !== dir) return false;
    if (mode && (t.mode || "other") !== mode) return false;
    if (min !== null && t.amount_paise < min) return false;
    if (max !== null && t.amount_paise > max) return false;
    if (q) {
      const hay = `${t.narration} ${t.cp_name || ""} ${t.cp_vpa || ""} ${t.cp_reference || ""}`.toLowerCase();
      if (!hay.includes(q)) return false;
    }
    return true;
  });
}

function refreshModeOptions() {
  const current = els.fMode.value;
  const modes = [...new Set(ALL.map((t) => t.mode || "other"))].sort();
  els.fMode.innerHTML =
    `<option value="">All</option>` +
    modes.map((m) => `<option value="${m}">${m.toUpperCase()}</option>`).join("");
  if (modes.includes(current)) els.fMode.value = current;
}

// ---------- render ----------
function render() {
  const hasData = ALL.length > 0;
  els.empty.hidden = hasData;
  for (const el of [els.kpis, els.charts]) el.style.display = hasData ? "" : "none";
  document.querySelector(".filters").style.display = hasData ? "" : "none";
  document.querySelector(".ledger-wrap").style.display = hasData ? "" : "none";
  if (!hasData) return;

  refreshModeOptions();
  const slice = filteredSlice();
  renderKpis(slice);
  renderCharts(slice);
  renderLedger(slice);
}

// ---------- KPIs ----------
function renderKpis(slice) {
  const inn = slice.filter((t) => t.direction === "credit").reduce((s, t) => s + t.amount_paise, 0);
  const out = slice.filter((t) => t.direction === "debit").reduce((s, t) => s + t.amount_paise, 0);
  const net = inn - out;
  // Closing balance: latest row per bank in the slice (list is newest-first).
  const latest = new Map();
  for (const t of slice) if (!latest.has(t.bank) && t.balance_paise != null) latest.set(t.bank, t);
  const closing = [...latest.values()].reduce((s, t) => s + t.balance_paise, 0);
  const asOf = slice.length ? fmtDate(slice[0].date) : "—";

  els.kpis.innerHTML = `
    <div class="kpi credit"><div class="label">Money in</div>
      <div class="value">${rupees(inn)}</div><div class="sub">↑ credits in view</div></div>
    <div class="kpi debit"><div class="label">Money out</div>
      <div class="value">${rupees(out)}</div><div class="sub">↓ debits in view</div></div>
    <div class="kpi"><div class="label">Net</div>
      <div class="value">${net < 0 ? "−" : ""}${rupees(Math.abs(net))}</div>
      <div class="sub">in − out</div></div>
    <div class="kpi"><div class="label">Closing balance</div>
      <div class="value">${latest.size ? rupees(closing) : "—"}</div>
      <div class="sub">as of ${asOf}</div></div>`;
}

// ---------- chart plumbing ----------
const CREDIT = "var(--credit)", DEBIT = "var(--debit)", ACCENT = "var(--accent)";

function card(title, legendHtml, svg, tableHtml) {
  return `<div class="card">
    <div class="card-head"><h3>${title}</h3>
      <button class="card-toggle" data-toggle>Show table</button></div>
    ${legendHtml || ""}
    <div data-view="chart">${svg}</div>
    <div data-view="table" hidden>${tableHtml}</div>
  </div>`;
}

els.charts.addEventListener("click", (e) => {
  const btn = e.target.closest("[data-toggle]");
  if (!btn) return;
  const cardEl = btn.closest(".card");
  const chart = cardEl.querySelector('[data-view="chart"]');
  const table = cardEl.querySelector('[data-view="table"]');
  const showTable = table.hidden;
  table.hidden = !showTable;
  chart.hidden = showTable;
  btn.textContent = showTable ? "Show chart" : "Show table";
});

function niceMax(v) {
  if (v <= 0) return 1;
  const pow = 10 ** Math.floor(Math.log10(v));
  for (const m of [1, 2, 2.5, 5, 10]) if (v <= m * pow) return m * pow;
  return 10 * pow;
}
function yTicks(maxPaise) {
  const top = niceMax(maxPaise);
  return { top, ticks: [0, top / 4, top / 2, (3 * top) / 4, top] };
}

// Tooltip: mouse + keyboard focus share one code path.
function showTip(target) {
  const tt = target.getAttribute("data-tt");
  if (!tt) return;
  const [title, ...rows] = tt.split("|");
  els.tooltip.innerHTML =
    `<div class="tt-title">${esc(title)}</div>` +
    rows.map((r) => `<div class="tt-row">${esc(r)}</div>`).join("");
  els.tooltip.hidden = false;
  const r = target.getBoundingClientRect();
  const tw = els.tooltip.offsetWidth, th = els.tooltip.offsetHeight;
  let x = r.left + r.width / 2 - tw / 2;
  x = Math.max(8, Math.min(x, window.innerWidth - tw - 8));
  let y = r.top - th - 8;
  if (y < 8) y = r.bottom + 8;
  els.tooltip.style.left = x + "px";
  els.tooltip.style.top = y + "px";
}
function hideTip() { els.tooltip.hidden = true; }
for (const [over, out] of [["mouseover", "mouseout"], ["focusin", "focusout"]]) {
  els.charts.addEventListener(over, (e) => {
    const t = e.target.closest("[data-tt]");
    if (t) showTip(t);
  });
  els.charts.addEventListener(out, hideTip);
}

// ---------- charts ----------
function renderCharts(slice) {
  els.charts.innerHTML =
    monthlyFlowCard(slice) + byModeCard(slice) + balanceCard(slice) + counterpartyCard(slice);
}

function monthlyFlowCard(slice) {
  const byMonth = new Map();
  for (const t of slice) {
    const ym = t.date.slice(0, 7);
    const m = byMonth.get(ym) || { in: 0, out: 0 };
    m[t.direction === "credit" ? "in" : "out"] += t.amount_paise;
    byMonth.set(ym, m);
  }
  const months = [...byMonth.keys()].sort();
  const W = 520, H = 220, padL = 46, padR = 10, padT = 10, padB = 26;
  const plotW = W - padL - padR, plotH = H - padT - padB;
  const maxV = Math.max(1, ...months.flatMap((m) => [byMonth.get(m).in, byMonth.get(m).out]));
  const { top, ticks } = yTicks(maxV);
  const band = plotW / Math.max(1, months.length);
  const barW = Math.min(24, band / 2 - 6);
  let bars = "", grid = "", labels = "";
  for (const t of ticks) {
    const y = padT + plotH - (t / top) * plotH;
    grid += `<line x1="${padL}" y1="${y}" x2="${W - padR}" y2="${y}" stroke="var(--grid)" stroke-width="1"/>`;
    labels += `<text class="chart-axis" x="${padL - 6}" y="${y + 3.5}" text-anchor="end">${rupeesCompact(t)}</text>`;
  }
  months.forEach((ym, i) => {
    const { in: vi, out: vo } = byMonth.get(ym);
    const cx = padL + band * i + band / 2;
    const hIn = (vi / top) * plotH, hOut = (vo / top) * plotH;
    // income fixed left, expense fixed right — position is the CVD-safe channel
    bars += `<rect class="bar" x="${cx - barW - 1}" y="${padT + plotH - hIn}" width="${barW}" height="${Math.max(hIn, 0.5)}" rx="4" fill="${CREDIT}"/>`;
    bars += `<rect class="bar" x="${cx + 1}" y="${padT + plotH - hOut}" width="${barW}" height="${Math.max(hOut, 0.5)}" rx="4" fill="${DEBIT}"/>`;
    bars += `<rect x="${padL + band * i}" y="${padT}" width="${band}" height="${plotH}" fill="transparent" tabindex="0"
      data-tt="${monthLabel(ym)}|In: ${rupees(vi)}|Out: ${rupees(vo)}"/>`;
    labels += `<text class="chart-axis" x="${cx}" y="${H - 8}" text-anchor="middle">${monthLabel(ym)}</text>`;
  });
  const baseline = `<line x1="${padL}" y1="${padT + plotH}" x2="${W - padR}" y2="${padT + plotH}" stroke="var(--baseline)" stroke-width="1"/>`;
  const legend = `<div class="legend">
    <span><span class="key" style="background:${CREDIT}"></span>Money in</span>
    <span><span class="key" style="background:${DEBIT}"></span>Money out</span></div>`;
  const table = `<table><tr><th>Month</th><th class="num">In</th><th class="num">Out</th></tr>` +
    months.map((m) => `<tr><td>${monthLabel(m)}</td><td class="amount">${rupees(byMonth.get(m).in)}</td><td class="amount">${rupees(byMonth.get(m).out)}</td></tr>`).join("") + `</table>`;
  const svg = `<svg viewBox="0 0 ${W} ${H}" role="img" aria-label="Monthly money in versus money out">${grid}${baseline}${bars}${labels}</svg>`;
  return card("Monthly flow", legend, months.length ? svg : noData(), table);
}

function hBarCard(title, rows, tipLabel) {
  // rows: [{label, value}] sorted desc — magnitude comparison, single hue.
  const W = 520, rowH = 26, padL = 130, padR = 70, padT = 4;
  const H = padT + rows.length * rowH + 6;
  const maxV = Math.max(1, ...rows.map((r) => r.value));
  let marks = "";
  rows.forEach((r, i) => {
    const y = padT + i * rowH;
    const w = Math.max((r.value / maxV) * (W - padL - padR), 1);
    marks += `<text class="chart-axis" x="${padL - 8}" y="${y + rowH / 2 + 3.5}" text-anchor="end">${esc(truncate(r.label, 18))}</text>`;
    marks += `<rect class="bar" x="${padL}" y="${y + (rowH - 16) / 2}" width="${w}" height="16" rx="4" fill="${ACCENT}"/>`;
    marks += `<text class="chart-dlabel" x="${padL + w + 6}" y="${y + rowH / 2 + 3.5}">${rupeesCompact(r.value)}</text>`;
    marks += `<rect x="0" y="${y}" width="${W}" height="${rowH}" fill="transparent" tabindex="0"
      data-tt="${esc(r.label)}|${tipLabel}: ${rupees(r.value)}"/>`;
  });
  const table = `<table><tr><th>${title}</th><th class="num">${tipLabel}</th></tr>` +
    rows.map((r) => `<tr><td>${esc(r.label)}</td><td class="amount">${rupees(r.value)}</td></tr>`).join("") + `</table>`;
  const svg = `<svg viewBox="0 0 ${W} ${H}" role="img" aria-label="${title}">${marks}</svg>`;
  return card(title, "", rows.length ? svg : noData(), table);
}

function byModeCard(slice) {
  const sums = new Map();
  for (const t of slice) if (t.direction === "debit") {
    const m = (t.mode || "other").toUpperCase();
    sums.set(m, (sums.get(m) || 0) + t.amount_paise);
  }
  const rows = [...sums.entries()].map(([label, value]) => ({ label, value }))
    .sort((a, b) => b.value - a.value);
  return hBarCard("Spend by mode", rows, "Spent");
}

function counterpartyCard(slice) {
  const sums = new Map();
  for (const t of slice) if (t.direction === "debit") {
    const name = t.cp_name || firstWords(t.narration, 3);
    sums.set(name, (sums.get(name) || 0) + t.amount_paise);
  }
  let rows = [...sums.entries()].map(([label, value]) => ({ label, value }))
    .sort((a, b) => b.value - a.value);
  if (rows.length > 7) {
    const other = rows.slice(7).reduce((s, r) => s + r.value, 0);
    rows = rows.slice(0, 7).concat({ label: "Other", value: other });
  }
  return hBarCard("Top spends", rows, "Spent");
}

function balanceCard(slice) {
  // Last balance per date per bank, ascending.
  const banks = [...new Set(slice.map((t) => t.bank))];
  const series = banks.map((bank) => {
    const byDate = new Map();
    for (const t of [...slice].reverse()) // oldest → newest
      if (t.bank === bank && t.balance_paise != null) byDate.set(t.date, t.balance_paise);
    return { bank, points: [...byDate.entries()].map(([d, v]) => ({ d, v })) };
  }).filter((s) => s.points.length);
  const all = series.flatMap((s) => s.points);
  if (!all.length) return card("Balance over time", "", noData(), "<table></table>");

  const W = 520, H = 220, padL = 52, padR = 14, padT = 10, padB = 26;
  const plotW = W - padL - padR, plotH = H - padT - padB;
  const dates = [...new Set(all.map((p) => p.d))].sort();
  const x = (d) => padL + (dates.indexOf(d) / Math.max(1, dates.length - 1)) * plotW;
  const maxV = Math.max(...all.map((p) => p.v));
  const { top, ticks } = yTicks(maxV);
  const y = (v) => padT + plotH - (v / top) * plotH;

  let grid = "", labels = "";
  for (const t of ticks) {
    grid += `<line x1="${padL}" y1="${y(t)}" x2="${W - padR}" y2="${y(t)}" stroke="var(--grid)" stroke-width="1"/>`;
    labels += `<text class="chart-axis" x="${padL - 6}" y="${y(t) + 3.5}" text-anchor="end">${rupeesCompact(t)}</text>`;
  }
  const step = Math.max(1, Math.ceil(dates.length / 6));
  dates.forEach((d, i) => {
    if (i % step === 0 || i === dates.length - 1)
      labels += `<text class="chart-axis" x="${x(d)}" y="${H - 8}" text-anchor="middle">${fmtDate(d).slice(0, 6)}</text>`;
  });

  const colors = [ACCENT, "var(--debit)", CREDIT]; // slot order; ≤3 banks realistically
  let marks = "";
  series.forEach((s, si) => {
    const c = colors[si % colors.length];
    const path = s.points.map((p, i) => `${i ? "L" : "M"}${x(p.d).toFixed(1)},${y(p.v).toFixed(1)}`).join("");
    marks += `<path d="${path}" fill="none" stroke="${c}" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"/>`;
    for (const p of s.points) {
      marks += `<circle class="dot" cx="${x(p.d)}" cy="${y(p.v)}" r="4" fill="${c}" stroke="var(--surface)" stroke-width="2"/>`;
      marks += `<circle cx="${x(p.d)}" cy="${y(p.v)}" r="12" fill="transparent" tabindex="0"
        data-tt="${fmtDate(p.d)}${series.length > 1 ? " · " + s.bank.toUpperCase() : ""}|Balance: ${rupees(p.v)}"/>`;
    }
  });
  const legend = series.length > 1
    ? `<div class="legend">` + series.map((s, si) =>
        `<span><span class="key" style="background:${colors[si % colors.length]}"></span>${s.bank.toUpperCase()}</span>`).join("") + `</div>`
    : "";
  const table = `<table><tr><th>Date</th>${series.length > 1 ? "<th>Bank</th>" : ""}<th class="num">Balance</th></tr>` +
    series.flatMap((s) => s.points.map((p) =>
      `<tr><td>${fmtDate(p.d)}</td>${series.length > 1 ? `<td>${s.bank.toUpperCase()}</td>` : ""}<td class="amount">${rupees(p.v)}</td></tr>`)).join("") + `</table>`;
  const svg = `<svg viewBox="0 0 ${W} ${H}" role="img" aria-label="Balance over time">${grid}${marks}${labels}</svg>`;
  return card("Balance over time", legend, svg, table);
}

function noData() {
  return `<p class="muted" style="padding:30px 0;text-align:center">Nothing in this view — widen the filters.</p>`;
}
const truncate = (s, n) => (s.length > n ? s.slice(0, n - 1) + "…" : s);
const firstWords = (s, n) => s.split(/\s+/).slice(0, n).join(" ");

// ---------- ledger ----------
function renderLedger(slice) {
  const multiBank = new Set(ALL.map((t) => t.bank)).size > 1;
  els.ledgerCount.textContent = `${slice.length} of ${ALL.length} transactions`;
  const head = `<tr>
    <th>Date</th><th>Details</th>${multiBank ? "<th>Bank</th>" : ""}
    <th class="num">Credit&nbsp;↑</th><th class="num">Debit&nbsp;↓</th><th class="num">Balance</th></tr>`;
  const rows = slice.map((t) => {
    const name = t.cp_name || firstWords(t.narration, 4);
    const credit = t.direction === "credit";
    return `<tr class="${t.direction}">
      <td class="mono">${fmtDate(t.date)}</td>
      <td><span class="txn-name">${esc(name)}</span>${t.mode ? `<span class="chip">${t.mode}</span>` : ""}
        <div class="txn-narr" title="${esc(t.narration)}">${esc(t.narration)}${t.cp_vpa ? " · " + esc(t.cp_vpa) : ""}</div></td>
      ${multiBank ? `<td>${t.bank.toUpperCase()}</td>` : ""}
      <td class="amount credit">${credit ? rupees(t.amount_paise) : '<span class="muted">—</span>'}</td>
      <td class="amount debit">${credit ? '<span class="muted">—</span>' : rupees(t.amount_paise)}</td>
      <td class="amount">${t.balance_paise != null ? rupees(t.balance_paise) : ""}</td>
    </tr>`;
  }).join("");
  els.ledger.innerHTML = `<table>${head}${rows || ""}</table>`;
}

els.pass.focus();
