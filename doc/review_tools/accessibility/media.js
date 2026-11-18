#!/usr/bin/env node
// prefers-reduced-motion: reduce and forced-colors: active, emulated; computed styles of
// nav, tabs, presets, cards, pick slot, modal, focus ring and the breathing dot.
"use strict";
const fs = require("fs");
const path = require("path");
const { SHARED, OUT, TODAY, browser } = require("./lib");

const PAGES = {
  home: "/",
  report: `/org/1?tab=report&from=2026-10-27&to=${TODAY}`,
  agenda: `/board/2?date=${TODAY}`,
  agendaPick: `/board/2?date=${TODAY}&pick=2`,
  modal: "/project/1?tab=tasks&new=task",
  sessions: "/task/80?tab=sessions",
  tasks: "/org/1?tab=tasks",
};

const style = (sel, props) => `(() => { const el = document.querySelector(${JSON.stringify(sel)}); if (!el) return "MISSING"; const cs = getComputedStyle(el); const o = {}; for (const p of ${JSON.stringify(props)}) o[p] = cs.getPropertyValue(p); return JSON.stringify(o); })()`;
const styleAfter = (sel, pseudo, props) => `(() => { const el = document.querySelector(${JSON.stringify(sel)}); if (!el) return "MISSING"; const cs = getComputedStyle(el, ${JSON.stringify(pseudo)}); const o = {}; for (const p of ${JSON.stringify(props)}) o[p] = cs.getPropertyValue(p); return JSON.stringify(o); })()`;

(async () => {
  const b = await browser();
  const out = [];
  const say = (s) => { out.push(s); console.log(s); };

  say("### prefers-reduced-motion: reduce");
  let ctx = await b.newContext({ reducedMotion: "reduce", viewport: { width: 1280, height: 900 } });
  let page = await ctx.newPage();
  await page.goto(SHARED + PAGES.home);
  say(`  .group transition-duration: ${await page.evaluate(style(".group", ["transition-duration"]))}`);
  await page.hover(".group");
  say(`  .group:hover transform: ${await page.evaluate(style(".group", ["transform"]))}`);
  say(`  .bar nav a::before transition: ${await page.evaluate(styleAfter(".bar nav a", "::before", ["transition-duration"]))}`);
  await page.goto(SHARED + PAGES.modal);
  say(`  .modal-box animation-name: ${await page.evaluate(style(".modal-box", ["animation-name", "animation-duration"]))}`);
  await page.goto(SHARED + PAGES.sessions);
  say(`  .live animation-name: ${await page.evaluate(style(".live", ["animation-name"]))}`);
  await page.goto(SHARED + PAGES.agenda);
  say(`  .tcard transition/transform: ${await page.evaluate(style(".tcard", ["transition-duration", "transform"]))}`);
  await page.goto(SHARED + PAGES.report);
  say(`  html scroll-behavior: ${await page.evaluate(style("html", ["scroll-behavior"]))}`);
  say(`  .tabs a::after transition: ${await page.evaluate(styleAfter(".tabs a", "::after", ["transition-duration"]))}`);
  await ctx.close();

  say("\n### prefers-reduced-motion: no-preference (the baseline)");
  ctx = await b.newContext({ reducedMotion: "no-preference", viewport: { width: 1280, height: 900 } });
  page = await ctx.newPage();
  await page.goto(SHARED + PAGES.modal);
  say(`  .modal-box animation: ${await page.evaluate(style(".modal-box", ["animation-name", "animation-duration"]))}`);
  await page.goto(SHARED + PAGES.sessions);
  say(`  .live animation: ${await page.evaluate(style(".live", ["animation-name", "animation-iteration-count"]))}`);
  await ctx.close();

  say("\n### forced-colors: active");
  ctx = await b.newContext({ forcedColors: "active", viewport: { width: 1280, height: 900 } });
  page = await ctx.newPage();
  await page.goto(SHARED + "/settings");
  say(`  nav current (.bar nav a[aria-current]) on /settings: ${await page.evaluate(style(".bar nav a[aria-current]", ["text-decoration-line", "outline-style", "outline-width", "outline-color"]))}`);
  await page.goto(SHARED + PAGES.report);
  say(`  nav current (.bar nav a[aria-current]): ${await page.evaluate(style(".bar nav a[aria-current]", ["text-decoration-line", "outline-style", "outline-width", "outline-color"]))}`);
  say(`  tab current (.tabs a[aria-current]): ${await page.evaluate(style(".tabs a[aria-current]", ["text-decoration-line", "color"]))} ::after ${await page.evaluate(styleAfter(".tabs a[aria-current]", "::after", ["border-bottom-width", "border-bottom-color", "opacity"]))}`);
  say(`  tab other ::after opacity: ${await page.evaluate(styleAfter(".tabs a:not([aria-current])", "::after", ["opacity"]))}`);
  say(`  preset current (.presets a[aria-current]): ${await page.evaluate(style(".presets a[aria-current]", ["outline-style", "outline-width", "outline-color"]))}`);
  say(`  .u link underline: ${await page.evaluate(style("a.u", ["text-decoration-line"]))}`);
  say(`  .status.wip / .count borders: ${await page.evaluate(style(".status", ["border-color", "border-style"]))} ${await page.evaluate(style(".count", ["border-color"]))}`);
  await page.focus(".tabs a");
  say(`  focused tab outline: ${await page.evaluate(style(".tabs a:focus-visible", ["outline-style", "outline-width", "outline-color"]))}`);
  await page.goto(SHARED + PAGES.agendaPick);
  say(`  card: ${await page.evaluate(style(".tcard", ["border-style", "border-color", "background-color"]))} ::before(edge) ${await page.evaluate(styleAfter(".tcard", "::before", ["background-image", "background-color"]))}`);
  say(`  picked card: ${await page.evaluate(style(".tcard[aria-current]", ["border-style", "border-color"]))} words="${await page.evaluate(() => (document.querySelector(".tcard-moving") || {}).textContent)}"`);
  say(`  priority chip (.tcard-prio): ${await page.evaluate(style(".tcard-prio", ["border-color", "border-style"]))}`);
  say(`  pick slot: ${await page.evaluate(style(".post-form.slot .button", ["border-style", "border-color"]))}`);
  say(`  pickbar notice: ${await page.evaluate(style(".pickbar", ["border-left-width", "border-style"]))}`);
  say(`  now line: ${await page.evaluate(style(".now", ["background-color"]))}`);
  say(`  drop zone (static): ${await page.evaluate(style(".hour .drop", ["border-style", "border-color"]))}`);
  await page.evaluate(() => document.querySelector(".hour .drop").classList.add("over"));
  say(`  drop zone .over: ${await page.evaluate(style(".hour .drop.over", ["border-style", "outline-style", "outline-color"]))}`);
  await page.goto(SHARED + PAGES.modal);
  say(`  modal box: ${await page.evaluate(style(".modal-box", ["border-width", "border-color"]))} backdrop bg=${await page.evaluate(style(".modal-backdrop", ["background-color"]))}`);
  say(`  invalid cue / error notice left border: ${await page.evaluate(style(".notice.error", ["border-left-width"]))} (none on this page unless refused)`);
  await page.focus("#f-name");
  say(`  focused input: ${await page.evaluate(style("#f-name:focus", ["outline-style", "outline-width", "outline-color", "border-color"]))}`);
  await page.goto(SHARED + PAGES.tasks);
  await page.focus(".filter-help-btn");
  say(`  filter tip: ${await page.evaluate(style(".filter-tip", ["border-width", "border-color", "display"]))}`);
  await ctx.close();
  await b.close();
  fs.mkdirSync(OUT, { recursive: true });
  fs.writeFileSync(path.join(OUT, "media.log"), out.join("\n"));
})().catch((e) => { console.error(e); process.exit(1); });
