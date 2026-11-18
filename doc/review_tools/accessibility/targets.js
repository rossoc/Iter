#!/usr/bin/env node
// Target size (WCAG 2.5.8, 24x24 min; 44 recommended): every link, button and input on a
// set of pages at 1280 and 400 px, fine pointer. An undersized target passes by the
// spacing exception when a 24px circle on its centre meets no other target or circle.
"use strict";
const fs = require("fs");
const path = require("path");
const { SHARED, OUT, browser } = require("./lib");

const PAGES = ["/", "/org/1?tab=tasks", "/org/1?tab=report&from=2026-10-27&to=2026-11-02", "/org/1?tab=tasks&new=task",
  "/task/1?tab=sessions", "/board/2?date=2026-11-02", "/board/2?date=2026-11-02&pick=2", "/board/2/matrix?pick=24", "/instructions", "/agents", "/settings"];

const MEASURE = `(() => {
  const sel = 'a[href], button, input:not([type=hidden]), select, textarea, summary, [tabindex="0"]';
  const els = [...document.querySelectorAll(sel)].filter((el) => !el.closest("[hidden],.palette,[inert]"));
  const boxes = els.map((el) => {
    const r = el.getBoundingClientRect();
    return { el, r, cx: r.left + r.width / 2, cy: r.top + r.height / 2 };
  }).filter((b) => b.r.width > 0 && b.r.height > 0);
  const label = (el) => el.tagName.toLowerCase() + (el.className && typeof el.className === "string" ? "." + el.className.trim().split(/\\s+/).slice(0,2).join(".") : "") + " \\"" + ((el.getAttribute("aria-label") || el.textContent || el.value || "").trim().replace(/\\s+/g, " ").slice(0, 28)) + "\\"";
  const small = [];
  for (const b of boxes) {
    if (b.r.width >= 24 && b.r.height >= 24) continue;
    // inline text link exception: in a sentence/block of text
    const cs = getComputedStyle(b.el);
    const inline = cs.display.startsWith("inline") && b.el.parentElement && b.el.parentElement.textContent.trim().length > b.el.textContent.trim().length + 10 && !b.el.closest("li,td,nav");
    let hit = null;
    for (const o of boxes) {
      if (o === b) continue;
      // circle of b (r=12) against o's box, and against o's circle when o is small too
      const dx = Math.max(o.r.left - b.cx, 0, b.cx - o.r.right);
      const dy = Math.max(o.r.top - b.cy, 0, b.cy - o.r.bottom);
      if (Math.hypot(dx, dy) < 12) { hit = label(o.el); break; }
      if ((o.r.width < 24 || o.r.height < 24) && Math.hypot(o.cx - b.cx, o.cy - b.cy) < 24) { hit = label(o.el); break; }
    }
    small.push({ what: label(b.el), w: Math.round(b.r.width), h: Math.round(b.r.height), inline, spacingOk: !hit, hit });
  }
  return { total: boxes.length, small };
})()`;

(async () => {
  const b = await browser();
  const all = [];
  for (const width of [1280, 400]) {
    const ctx = await b.newContext({ viewport: { width, height: 900 } });
    const page = await ctx.newPage();
    for (const url of PAGES) {
      await page.goto(SHARED + url, { waitUntil: "load" });
      const r = await page.evaluate(MEASURE);
      const fails = r.small.filter((s) => !s.spacingOk && !s.inline);
      console.log(`${width} ${url.padEnd(52)} targets=${r.total} under24=${r.small.length} FAIL(no spacing, not inline)=${fails.length}`);
      for (const f of fails) console.log(`     ${f.w}x${f.h} ${f.what} <-> ${f.hit}`);
      const under = r.small.filter((s) => s.spacingOk || s.inline);
      const seen = new Set();
      for (const u of under) { const k = u.what.split(" \"")[0] + ` ${u.w}x${u.h}`; if (!seen.has(k)) { seen.add(k); } }
      console.log(`     pass-by-exception kinds: ${[...seen].slice(0, 12).join(" | ")}`);
      all.push({ width, url, ...r });
    }
    await ctx.close();
  }
  await b.close();
  fs.mkdirSync(OUT, { recursive: true });
  fs.writeFileSync(path.join(OUT, "targets.json"), JSON.stringify(all, null, 1));
})().catch((e) => { console.error(e); process.exit(1); });
