#!/usr/bin/env node
// Keyboard walks (real Tab/Enter/Escape key presses in Chromium). The agenda and matrix
// walks POST, so they run on the scale-1 server (OWN); the rest reads the shared one.
// Ids below are from the two stores; pass A11Y_* env vars to change servers.
"use strict";
const fs = require("fs");
const path = require("path");
const { SHARED, OWN, OUT, TODAY, browser, DESCRIBE, go } = require("./lib");

const OWN_SCHEDULED_TASK = process.env.OWN_SCHED || "6"; // wip, scheduled today on board 1 (own store)
const OWN_UNPLACED_TASK = process.env.OWN_UNPLACED || "1"; // on board 1, not placed in the matrix
const log = [];
const say = (s) => { log.push(s); console.log(s); };

async function tabs(page, n, shift = false) {
  const seq = [];
  for (let i = 0; i < n; i++) {
    await page.keyboard.press(shift ? "Shift+Tab" : "Tab");
    seq.push(await page.evaluate(DESCRIBE));
  }
  return seq;
}
async function tabUntil(page, pred, max = 80) {
  for (let i = 1; i <= max; i++) {
    await page.keyboard.press("Tab");
    const d = await page.evaluate(DESCRIBE);
    if (pred(d)) return { i, d };
  }
  return null;
}

(async () => {
  const b = await browser();
  const ctx = await b.newContext({ viewport: { width: 1280, height: 900 } });
  const page = await ctx.newPage();

  say("### A. Home Tab order (shared)");
  await page.goto(SHARED + "/");
  (await tabs(page, 12)).forEach((d, i) => say(`  ${i + 1}. ${d}`));

  say("\n### B. Agenda: card action -> pick -> hour -> moved -> cancel (own store)");
  await page.goto(`${OWN}/board/1?date=${TODAY}`);
  say(`  title: ${await page.title()}`);
  let hit = await tabUntil(page, (d) => d.includes("tcard-act"));
  say(`  first card action reached after ${hit && hit.i} tabs: ${hit && hit.d}`);
  const opacity = await page.evaluate(() => getComputedStyle(document.activeElement).opacity);
  say(`  its opacity while focused: ${opacity}`);
  // the day input and the hidden submit button on the way
  await page.goto(`${OWN}/board/1?date=${TODAY}`);
  hit = await tabUntil(page, (d) => d.includes("#day-input"));
  say(`  day input reached after ${hit && hit.i} tabs: ${hit && hit.d}`);
  if (hit) {
    const before = page.url();
    await page.keyboard.press("ArrowUp");
    await page.waitForTimeout(500);
    say(`  ArrowUp on the date: url changed? ${page.url() !== before} (F2 of the Oct review)`);
    for (let k = 1; k <= 5; k++) { await page.keyboard.press("Tab"); const d = await page.evaluate(DESCRIBE); say(`  Tab x${k} after the date input: ${d}`); if (d.startsWith("button")) break; }
    const clip = await page.evaluate(() => { const cs = getComputedStyle(document.activeElement); return `clip-path=${cs.clipPath} outline=${cs.outlineStyle} ${cs.outlineWidth} size=${document.activeElement.offsetWidth}x${document.activeElement.offsetHeight}`; });
    say(`  that element: ${clip}`);
  }
  // pick mode by keyboard: open the picked task's Move/Schedule link
  await page.goto(`${OWN}/board/1?date=${TODAY}`);
  const actSel = `#card-${OWN_SCHEDULED_TASK} .tcard-act`;
  const hasCard = await page.$(actSel);
  say(`  card ${OWN_SCHEDULED_TASK} on the page: ${!!hasCard}`);
  if (hasCard) {
    await page.focus(actSel);
    say(`  focused: ${await page.evaluate(DESCRIBE)}`);
    await go(page, () => page.keyboard.press("Enter"));
    say(`  after Enter: url=${new URL(page.url()).search}${new URL(page.url()).hash} title="${await page.title()}"`);
    say(`  active element on the pick page: ${await page.evaluate(DESCRIBE)}`);
    const seq = await tabs(page, 6);
    seq.forEach((d, i) => say(`    tab ${i + 1}: ${d}`));
    // choose an hour: the first "Schedule/Move here" slot button
    const slot = await page.$(".post-form.slot .button");
    say(`  slot button text: "${slot && (await slot.textContent()).trim()}"`);
    await slot.focus();
    await go(page, () => page.keyboard.press("Enter"));
    const u = new URL(page.url());
    say(`  after choosing the hour: ${u.search}${u.hash} title="${await page.title()}"`);
    say(`  active element: ${await page.evaluate(DESCRIBE)}`);
    say(`  status region: role=${await page.evaluate(() => { const n = document.querySelector(".board-notes"); return n ? n.getAttribute("role") + ' "' + n.textContent.trim() + '"' : "none"; })}`);
    await page.waitForTimeout(300);
    say(`  status text after 300ms (board.js rewrites it): "${await page.evaluate(() => { const n = document.querySelector(".board-notes"); return n ? n.textContent.trim() : ""; })}" url now ${new URL(page.url()).search}`);
    // cancel: back into pick mode, Tab to Cancel, Enter
    await page.focus(`#card-${OWN_SCHEDULED_TASK} .tcard-act`);
    await go(page, () => page.keyboard.press("Enter"));
    const cancel = await tabUntil(page, (d) => d.includes('"Cancel"'), 12);
    say(`  Cancel reached after ${cancel && cancel.i} tabs from the pick banner: ${cancel && cancel.d}`);
    await go(page, () => page.keyboard.press("Enter"));
    const after = await page.evaluate(DESCRIBE);
    say(`  after Cancel: ${new URL(page.url()).hash} active=${after}`);
    const ring = await page.evaluate(() => { const el = document.activeElement; const cs = getComputedStyle(el); return `${el.id} outline=${cs.outlineStyle} focus-visible=${el.matches(":focus-visible")}`; });
    say(`  focus indicator on the card: ${ring}`);
    // Unschedule (undo) so the store is as before
    await page.focus(`#card-${OWN_SCHEDULED_TASK} .tcard-act`);
    await go(page, () => page.keyboard.press("Enter"));
    const un = await page.$('.pickbar .post-form .button:has-text("Unschedule")');
    say(`  Unschedule button in the banner: ${!!un}`);
  }

  say("\n### C. Matrix pick mode (own store)");
  await page.goto(`${OWN}/board/1/matrix`);
  const place = await page.$(`#card-${OWN_UNPLACED_TASK} .tcard-act`);
  say(`  card ${OWN_UNPLACED_TASK} action: ${place && (await place.textContent()).trim()}`);
  if (place) {
    await place.focus();
    await go(page, () => page.keyboard.press("Enter"));
    say(`  pick page: ${new URL(page.url()).search}${new URL(page.url()).hash} active=${await page.evaluate(DESCRIBE)} title="${await page.title()}"`);
    const q = await page.$$(".quad .post-form .button");
    say(`  quadrant buttons: ${q.length}: ${(await Promise.all(q.map((e) => e.evaluate((el) => el.textContent.replace(/\s+/g, " ").trim())))).join(" | ")}`);
    await q[1].focus();
    await go(page, () => page.keyboard.press("Enter"));
    say(`  after Place: ${new URL(page.url()).search}${new URL(page.url()).hash} active=${await page.evaluate(DESCRIBE)} note="${await page.evaluate(() => (document.querySelector(".board-notes") || {}).textContent || "")}"`);
    // put it back
    await page.focus(`#card-${OWN_UNPLACED_TASK} .tcard-act`);
    await go(page, () => page.keyboard.press("Enter"));
    const unplace = await page.$('.pickbar .post-form .button:has-text("Unplace")');
    if (unplace) { await unplace.focus(); await page.keyboard.press("Enter"); await page.waitForLoadState("load"); say(`  unplaced again: ${new URL(page.url()).search}`); }
  }

  say("\n### D. New task pop-up (shared, GET only)");
  await page.goto(`${SHARED}/project/1?tab=tasks&new=task`);
  say(`  on load active: ${await page.evaluate(DESCRIBE)} title="${await page.title()}"`);
  say(`  inert: ${await page.evaluate(() => ["a.skip", "header.bar", "main", "footer.site-foot"].map((s) => s + "=" + (document.querySelector(s) && document.querySelector(s).hasAttribute("inert"))).join(" "))}`);
  (await tabs(page, 9)).forEach((d, i) => say(`  tab ${i + 1}: ${d}`));
  say(`  shift+tab x2: ${(await tabs(page, 2, true)).join(" | ")}`);
  const x = await page.$(".modal-x");
  say(`  close ✕: name="${await x.evaluate((el) => el.textContent.trim())}" ${await x.evaluate((el) => Math.round(el.getBoundingClientRect().width) + "x" + Math.round(el.getBoundingClientRect().height))}`);
  say(`  backdrop: ${await page.evaluate(() => { const b = document.querySelector(".modal-backdrop"); return `tabindex=${b.getAttribute("tabindex")} aria-hidden=${b.getAttribute("aria-hidden")} href=${b.getAttribute("href")}`; })}`);
  await go(page, () => page.keyboard.press("Escape"));
  say(`  Escape: url=${new URL(page.url()).search} active=${await page.evaluate(DESCRIBE)} (focus return to the opener?)`);
  const opener = await page.$('a.add-link');
  say(`  opener link present on the page after close: ${!!opener} "${opener && (await opener.textContent()).trim()}"`);

  say("\n### E. Search palette (shared)");
  await page.goto(`${SHARED}/org/1`);
  await page.focus(".search-btn");
  await page.keyboard.press("Enter");
  await page.waitForSelector("#palette:not([hidden])");
  say(`  opened: active=${await page.evaluate(DESCRIBE)} inert=${await page.evaluate(() => ["header.bar", "main", "footer.site-foot"].map((s) => document.querySelector(s).hasAttribute("inert")).join(","))}`);
  await page.keyboard.type("task-1");
  await page.waitForSelector("#palette-list li[role=option]");
  await page.waitForTimeout(300);
  say(`  results: ${await page.evaluate(() => document.querySelectorAll("#palette-list li").length)} status="${await page.evaluate(() => document.getElementById("palette-status").textContent)}" activedescendant=${await page.evaluate(() => document.querySelector(".palette-input").getAttribute("aria-activedescendant"))}`);
  await page.keyboard.press("ArrowDown");
  say(`  ArrowDown: activedescendant=${await page.evaluate(() => document.querySelector(".palette-input").getAttribute("aria-activedescendant"))} selected="${await page.evaluate(() => document.querySelector("#palette-list li[aria-selected=true]").textContent.trim().slice(0, 40))}"`);
  await page.keyboard.press("Tab");
  await page.waitForTimeout(200);
  say(`  Tab from the input: active=${await page.evaluate(DESCRIBE)} (iframe focus?) frameFocused=${await page.evaluate(() => document.activeElement === document.querySelector(".palette-frame"))}`);
  await page.keyboard.press("Escape");
  await page.waitForTimeout(200);
  say(`  Escape from the frame: palette hidden=${await page.evaluate(() => document.getElementById("palette").hidden)} active=${await page.evaluate(DESCRIBE)}`);
  if (!(await page.evaluate(() => document.getElementById("palette").hidden))) {
    await page.focus(".palette-input");
    await page.keyboard.press("Escape");
    say(`  Escape from the input: hidden=${await page.evaluate(() => document.getElementById("palette").hidden)} active=${await page.evaluate(DESCRIBE)}`);
  }
  await page.keyboard.press("Enter"); // reopen via the focused button
  await page.waitForSelector("#palette:not([hidden])");
  await page.keyboard.type("task-1");
  await page.waitForSelector("#palette-list li[role=option]");
  await go(page, () => page.keyboard.press("Enter"));
  say(`  Enter on a result navigates to: ${new URL(page.url()).pathname}`);
  await page.goto(`${SHARED}/org/1?tab=tasks`);
  await page.focus("#filter-q");
  await page.keyboard.press("/");
  await page.waitForTimeout(100);
  say(`  "/" typed inside a text field opens the palette? ${!(await page.evaluate(() => document.getElementById("palette").hidden))} (should be false)`);
  await page.click("main h1");
  await page.keyboard.press("/");
  await page.waitForTimeout(100);
  say(`  "/" on the page opens it: ${!(await page.evaluate(() => document.getElementById("palette").hidden))}; any setting to turn the single-key shortcut off? none found in the source (2.1.4)`);

  say("\n### F. Combobox: enhanced <select> in the New task pop-up, input[list] on an instruction (shared)");
  await page.goto(`${SHARED}/org/1?tab=tasks&new=task`);
  await page.waitForTimeout(200);
  const cb = await page.$("button.cb-button");
  say(`  select replaced by a button: ${!!cb} ${cb && (await cb.evaluate((el) => `role=${el.getAttribute("role")} haspopup=${el.getAttribute("aria-haspopup")} labelledby=${el.getAttribute("aria-labelledby")} name="${document.getElementById(el.getAttribute("aria-labelledby")).textContent.trim()}" value="${el.textContent.trim()}"`))}`);
  if (cb) {
    await cb.focus();
    await page.keyboard.press("ArrowDown");
    await page.waitForTimeout(100);
    say(`  ArrowDown: expanded=${await cb.evaluate((el) => el.getAttribute("aria-expanded"))} controls=${await cb.evaluate((el) => el.getAttribute("aria-controls"))} active=${await cb.evaluate((el) => el.getAttribute("aria-activedescendant"))} listbox visible=${await page.evaluate(() => { const l = document.querySelector(".cb-pop"); return l && !l.hidden; })} options=${await page.evaluate(() => document.querySelectorAll(".cb-pop li[role=option]").length)}`);
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Enter");
    await page.waitForTimeout(100);
    say(`  ArrowDown+Enter: value="${await cb.evaluate((el) => el.textContent.trim())}" native="${await page.evaluate(() => document.querySelector("select.cb-native").value)}" expanded=${await cb.evaluate((el) => el.getAttribute("aria-expanded"))}`);
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Escape");
    await page.waitForTimeout(300);
    say(`  Escape with the list open: closed=${await page.evaluate(() => document.querySelector(".cb-pop").hidden)} modal still here=${!!(await page.$("[role=dialog]"))} url=${new URL(page.url()).search}`);
    await go(page, () => page.keyboard.press("Escape"));
    say(`  second Escape: url=${new URL(page.url()).search} (closes the pop-up)`);
  }
  await page.goto(`${SHARED}/instruction/develop?tab=settings`);
  await page.waitForTimeout(200);
  const model = await page.$("#f-model");
  say(`  model input: role=${await model.evaluate((el) => el.getAttribute("role"))} list attr=${await model.evaluate((el) => el.getAttribute("list"))} autocomplete=${await model.evaluate((el) => el.getAttribute("aria-autocomplete"))}`);
  await model.focus();
  await page.keyboard.press("ArrowDown");
  await page.waitForTimeout(100);
  say(`  ArrowDown: expanded=${await model.evaluate((el) => el.getAttribute("aria-expanded"))} options=${await page.evaluate(() => document.querySelectorAll(".cb-pop:not([hidden]) li").length)} active=${await model.evaluate((el) => el.getAttribute("aria-activedescendant"))}`);
  await page.keyboard.press("Escape");
  say(`  Escape: expanded=${await model.evaluate((el) => el.getAttribute("aria-expanded"))}`);

  say("\n### G. Filter bar (shared)");
  await page.goto(`${SHARED}/org/1?tab=tasks`);
  await page.focus("#filter-q");
  say(`  field: ${await page.evaluate(DESCRIBE)} label="${await page.evaluate(() => document.activeElement.labels[0].textContent)}"`);
  await page.keyboard.press("Tab");
  say(`  Tab: ${await page.evaluate(DESCRIBE)} tip display=${await page.evaluate(() => getComputedStyle(document.getElementById("filter-syntax")).display)} describedby=${await page.evaluate(() => document.activeElement.getAttribute("aria-describedby"))}`);
  await page.keyboard.press("Escape");
  say(`  Escape: tip display=${await page.evaluate(() => getComputedStyle(document.getElementById("filter-syntax")).display)} (F6 of the Oct review)`);
  await page.keyboard.press("Tab");
  say(`  Tab: ${await page.evaluate(DESCRIBE)}`);
  await page.focus("#filter-q");
  await page.keyboard.type("is:done");
  await go(page, () => page.keyboard.press("Enter"));
  say(`  Enter in the field: ${new URL(page.url()).search} status="${await page.evaluate(() => (document.querySelector(".filter-count") || {}).textContent || "none")}" clear link=${!!(await page.$(".filter-clear"))}`);

  say("\n### H. Report date form (shared)");
  await page.goto(`${SHARED}/org/1?tab=report&from=2026-10-27&to=${TODAY}`);
  await page.focus('input[name="to"]');
  say(`  To focused: ${await page.evaluate(DESCRIBE)}`);
  await page.keyboard.press("Tab");
  say(`  Tab: ${await page.evaluate(DESCRIBE)} type=${await page.evaluate(() => document.activeElement.type)} (A3b: Apply reachable and visible?)`);
  await page.focus('input[name="from"]');
  const before = page.url();
  await page.keyboard.press("ArrowUp");
  await page.waitForTimeout(600);
  say(`  ArrowUp on From: navigated=${page.url() !== before} value=${await page.evaluate(() => document.querySelector('input[name="from"]').value)}`);
  await go(page, () => page.keyboard.press("Enter"));
  say(`  Enter: ${new URL(page.url()).search}`);
  await page.goto(`${SHARED}/org/1?tab=report&from=nonsense&to=${TODAY}`);
  say(`  bad date: alert="${await page.evaluate(() => (document.querySelector("[role=alert]") || {}).textContent || "NONE")}" (A3)`);
  say(`  aria-current on the report page: ${await page.evaluate(() => [...document.querySelectorAll("[aria-current]")].map((e) => e.getAttribute("aria-current") + ":" + e.textContent.trim().slice(0, 12)).join(" "))} (A1)`);

  say("\n### I. Drag-and-drop alternative: every card has an action link, every hour a slot button in pick mode (shared)");
  await page.goto(`${SHARED}/board/2?date=${TODAY}`);
  say(`  cards=${await page.evaluate(() => document.querySelectorAll(".tcard").length)} action links=${await page.evaluate(() => document.querySelectorAll(".tcard .tcard-act").length)} draggable set by script=${await page.evaluate(() => document.querySelectorAll("[draggable=true]").length)}`);
  await page.goto(`${SHARED}/board/2?date=${TODAY}&pick=2`);
  say(`  pick mode: slot buttons=${await page.evaluate(() => document.querySelectorAll(".post-form.slot .button").length)} picked card aria-current=${await page.evaluate(() => (document.querySelector(".tcard[aria-current]") || {}).id)} words="${await page.evaluate(() => (document.querySelector(".tcard-moving") || {}).textContent)}" banner role=${await page.evaluate(() => document.getElementById("pick").getAttribute("role"))} labelledby ok=${await page.evaluate(() => !!document.getElementById(document.getElementById("pick").getAttribute("aria-labelledby")))}`);
  say(`  pick slot button accessible name: "${await page.evaluate(() => document.querySelector(".post-form.slot .button").textContent.replace(/\s+/g, " ").trim())}" small button name: "${await page.evaluate(() => { const b = document.querySelector(".pickbar .post-form .button"); return b ? b.textContent.replace(/\s+/g, " ").trim() : "none"; })}"`);

  await b.close();
  fs.mkdirSync(OUT, { recursive: true });
  fs.writeFileSync(path.join(OUT, "keyboard.log"), log.join("\n"));
})().catch((e) => { console.error(e); process.exit(1); });
