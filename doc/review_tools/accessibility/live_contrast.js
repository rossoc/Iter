#!/usr/bin/env node
// Contrast from the browser's own computed colours (color-mix resolved by Chromium, read
// back through a canvas), light and dark, for the pairs the token script flagged and a
// few that depend on state (selected search result, combobox active item, drop zone
// during a drag, picked card, hover button).
"use strict";
const fs = require("fs");
const path = require("path");
const { SHARED, OUT, TODAY, browser } = require("./lib");

const HELPERS = `
  function rgb(str) { if (!str || str === "transparent" || /^rgba\(.*,\s*0\)$/.test(str)) return [0, 0, 0, 0]; const c = document.createElement("canvas"); c.width = c.height = 1; const g = c.getContext("2d"); g.clearRect(0, 0, 1, 1); g.fillStyle = str; g.fillRect(0, 0, 1, 1); const d = g.getImageData(0, 0, 1, 1).data; return [d[0], d[1], d[2], d[3]]; }
  function lum([r, g, b]) { const f = (c) => { c /= 255; return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4); }; return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b); }
  function ratio(a, b) { const [x, y] = [lum(a), lum(b)]; return ((Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05)).toFixed(2); }
  function over(fg, bg) { const a = fg[3] / 255; return [0,1,2].map((i) => Math.round(fg[i] * a + bg[i] * (1 - a))); }
  function bgOf(el) { let e = el; let acc = null; while (e && e !== document.documentElement) { const c = rgb(getComputedStyle(e).backgroundColor); if (c[3] > 0) { acc = acc ? over(acc, c) : (c[3] === 255 ? c : null); if (acc && c[3] === 255) return acc; if (!acc) acc = c; } e = e.parentElement; } const page = rgb(getComputedStyle(document.body).backgroundColor); return acc ? over(acc, page) : page; }
  function want(sel, cls) { let el = document.querySelector(sel); if (!el) { el = document.createElement("span"); el.className = cls; el.textContent = "sample"; document.querySelector("main").appendChild(el); } return el; }
  function hex(c) { return "#" + c.slice(0,3).map((x) => x.toString(16).padStart(2, "0")).join(""); }
  function pair(label, el, prop) { if (!el) return label + ": MISSING"; const cs = getComputedStyle(el); const fg = rgb(cs.getPropertyValue(prop || "color")); const bg = bgOf(prop ? el.parentElement || el : el); const f = fg[3] < 255 ? over(fg, bg) : fg; return label + ": " + hex(f) + " on " + hex(bg) + " = " + ratio(f, bg); }
  function border(label, el) { if (!el) return label + ": MISSING"; const cs = getComputedStyle(el); const c = rgb(cs.borderTopColor); const bg = bgOf(el.parentElement); const f = c[3] < 255 ? over(c, bg) : c; return label + " (border): " + hex(f) + " on " + hex(bg) + " = " + ratio(f, bg); }
`;

(async () => {
  const b = await browser();
  const out = [];
  const say = (s) => { out.push(s); console.log(s); };
  for (const scheme of ["light", "dark"]) {
    say(`### ${scheme}`);
    const ctx = await b.newContext({ colorScheme: scheme, viewport: { width: 1280, height: 900 } });
    const page = await ctx.newPage();
    const ev = (code) => page.evaluate("(() => {" + HELPERS + " return (" + code + "); })()");
    await page.goto(`${SHARED}/org/1?tab=tasks`);
    say(await ev(`pair("body text --fg", document.querySelector("main h1"))`));
    say(await ev(`pair(".label (eyebrow, --muted 11px uppercase)", document.querySelector("main .label"))`));
    say(await ev(`pair(".tabs a (inactive tab --muted)", document.querySelector(".tabs a:not([aria-current])"))`));
    say(await ev(`pair(".bar nav a (--muted)", document.querySelector(".bar nav a:not([aria-current])"))`));
    say(await ev(`pair(".status.wip text on tint", document.querySelector(".status.wip"))`));
    say(await ev(`border(".status.wip", document.querySelector(".status.wip"))`));
    say(await ev(`pair(".status.done text", want(".status.done", "status done"))`));
    say(await ev(`border(".status.done", want(".status.done", "status done"))`));
    say(await ev(`pair(".status.queue text", document.querySelector(".status.queue"))`));
    say(await ev(`pair("th (--muted)", document.querySelector("th"))`));
    say(await ev(`pair(".search-btn text", document.querySelector(".search-btn"))`));
    say(await ev(`border(".search-btn", document.querySelector(".search-btn"))`));
    say(await ev(`pair("kbd text", document.querySelector(".search-btn kbd"))`));
    say(await ev(`border(".button (control-line)", document.querySelector(".filter .button"))`));
    say(await ev(`border("input.control", document.querySelector("#filter-q"))`));
    say(await ev(`pair(".filter-help-btn ?", document.querySelector(".filter-help-btn"))`));
    say(await ev(`pair(".foot-note", document.querySelector(".foot-note"))`));
    say(await ev(`pair(".foot-sub a (project links in footer)", document.querySelector(".foot-sub a"))`));
    say(await ev(`pair(".crumbs separator (--faint, decorative)", document.querySelector(".crumbs li+li"), null)`));
    await page.hover(".filter .button");
    say(await ev(`pair(".button:hover text (accent on bg-3)", document.querySelector(".filter .button"))`));
    // search palette, selected hit
    await page.keyboard.press("/");
    await page.waitForSelector("#palette:not([hidden])");
    await page.fill(".palette-input", "task");
    await page.waitForSelector("#palette-list li[aria-selected=true]");
    say(await ev(`pair(".palette-sub of the selected result (--muted on tint)", document.querySelector("#palette-list li[aria-selected=true] .palette-sub"))`));
    say(await ev(`pair(".palette-sub of another result", document.querySelector("#palette-list li:not([aria-selected]) .palette-sub"))`));
    say(await ev(`pair(".palette-title of the selected result", document.querySelector("#palette-list li[aria-selected=true] .palette-title"))`));
    say(await ev(`pair(".palette-title mark (accent)", document.querySelector("#palette-list li[aria-selected=true] mark"))`));
    say(await ev(`pair(".palette-hint", document.querySelector(".palette-hint"))`));
    say(await ev(`pair(".palette-input placeholder", document.querySelector(".palette-input"), "color")`));
    // agenda: drop zone during drag, picked card, hour label
    await page.goto(`${SHARED}/board/2?date=${TODAY}&pick=2`);
    say(await ev(`pair(".hour-label", document.querySelector(".hour-label"))`));
    say(await ev(`pair(".tcard-meta text on card", document.querySelector(".tcard:not([aria-current]) .tcard-meta"))`));
    say(await ev(`pair(".tcard-prio text", document.querySelector(".tcard-prio"))`));
    say(await ev(`border(".tcard-prio", document.querySelector(".tcard-prio"))`));
    say(await ev(`pair(".tcard[aria-current] .tcard-meta (fg on bg-3)", document.querySelector(".tcard[aria-current] .tcard-meta"))`));
    say(await ev(`border(".tcard[aria-current] dashed accent", document.querySelector(".tcard[aria-current]"))`));
    say(await ev(`pair(".post-form.slot .button (accent text)", document.querySelector(".post-form.slot .button"))`));
    say(await ev(`border(".post-form.slot .button dashed accent", document.querySelector(".post-form.slot .button"))`));
    say(await ev(`pair(".pickbar text", document.querySelector(".pickbar p"))`));
    say(await ev(`pair(".day-nav .button[aria-current] Today", document.querySelector(".day-nav .button[aria-current]"))`));
    say(await ev(`pair(".button.small Cancel text", document.querySelector(".pickbar a.button"))`));
    await page.evaluate(() => { const z = [...document.querySelectorAll(".side .drop-list")].find((d) => d.querySelector(".empty-line")); if (z) z.classList.add("over"); });
    say(await ev(`pair(".drop.over .empty-line (A6 of the Oct review)", document.querySelector(".drop.over .empty-line"))`));
    say(await ev(`pair(".empty-line at rest", want(".empty-line", "empty-line"))`));
    say(await ev(`pair("summary .label (collapsible heading)", document.querySelector(".collapsible summary .label"))`));
    say(await ev(`pair(".count badge", document.querySelector(".count"))`));
    say(await ev(`border(".tcard edge default Urgent (--urgent) vs card", (() => { const e = document.querySelector(".tcard.edge-urgent, .tcard.edge-both"); return e; })())`));
    say(await ev(`(() => { const root = document.querySelector("[data-post]"); const s = root ? root.getAttribute("style") : ""; return "board root style: " + s; })()`));
    say(await ev(`(() => { const e = document.querySelector(".tcard.edge-urgent, .tcard.edge-both"); if (!e) return "edge: no urgent card on the page"; const cs = getComputedStyle(e, "::before"); const img = cs.backgroundImage; const m = img.match(/rgb\\([^)]*\\)|#[0-9a-f]{3,8}|oklab\\([^)]*\\)|color\\([^)]*\\)/i); const c = m ? rgb(m[0]) : null; const bg = bgOf(e); return "edge colour " + (c ? hex(c) + " on card " + hex(bg) + " = " + ratio(c, bg) : img.slice(0, 80)); })()`));
    // combobox active item with hint, if any datalist has labels: the instruction page
    await page.goto(`${SHARED}/instruction/develop?tab=settings`);
    await page.waitForTimeout(200);
    await page.focus("#f-model");
    await page.keyboard.press("ArrowDown");
    await page.waitForTimeout(100);
    say(await ev(`pair(".cb-pop li.cb-active text", document.querySelector(".cb-pop li.cb-active"))`));
    say(await ev(`pair(".cb-pop li.cb-active .cb-hint (--muted on tint)", document.querySelector(".cb-pop li.cb-active .cb-hint") || (() => { const li = document.querySelector(".cb-pop li.cb-active"); if (!li) return null; const s = document.createElement("span"); s.className = "cb-hint"; s.textContent = "x"; li.appendChild(s); return s; })())`));
    say(await ev(`pair(".cb-pop li (rest) text", document.querySelector(".cb-pop li:not(.cb-active)"))`));
    say(await ev(`pair(".hint under a field", document.querySelector(".field .hint"))`));
    say(await ev(`pair(".required-note", document.querySelector(".required-note"))`));
    say(await ev(`pair(".req asterisk (--muted)", document.querySelector(".req"))`));
    // error notice
    await page.goto(`${SHARED}/org/1?tab=report&from=nonsense&to=${TODAY}`);
    say(await ev(`pair(".notice.error text", document.querySelector(".notice.error"))`));
    say(await ev(`border(".notice.error", document.querySelector(".notice.error"))`));
    say(await ev(`pair(".presets a current (accent on bg)", document.querySelector(".presets a[aria-current]"))`));
    say(await ev(`pair(".presets a other (--muted on bg-2)", document.querySelector(".presets a:not([aria-current])"))`));
    say(await ev(`pair(".range label (--muted)", document.querySelector(".range label"))`));
    say(await ev(`pair(".summary text", document.querySelector(".summary > span:not(.big)"))`));
    say(await ev(`pair(".time", document.querySelector(".time"))`));
    say(await ev(`pair(".sessions td.mono", document.querySelector(".sessions td.mono"))`));
    say(await ev(`pair(".button.primary text", document.querySelector(".button.primary") || document.querySelector(".skip"))`));
    await ctx.close();
  }
  await b.close();
  fs.mkdirSync(OUT, { recursive: true });
  fs.writeFileSync(path.join(OUT, "live_contrast.log"), out.join("\n"));
})().catch((e) => { console.error(e); process.exit(1); });
