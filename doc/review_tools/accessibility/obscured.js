#!/usr/bin/env node
// Focus not obscured (WCAG 2.4.11): Tab through the agenda in pick mode (sticky bar plus
// sticky pick banner and day head) and the report page; is any focused element hidden
// behind a sticky part at 1280x800 and 1000x620?
"use strict";
const { SHARED, TODAY, browser, DESCRIBE } = require("./lib");
(async () => {
  const b = await browser();
  for (const [w, h] of [[1280, 800], [1000, 620]]) {
    const ctx = await b.newContext({ viewport: { width: w, height: h } });
    const page = await ctx.newPage();
    for (const url of [`/board/2?date=${TODAY}&pick=2`, `/board/2?date=${TODAY}`, `/org/1?tab=report&from=2026-10-27&to=${TODAY}`, "/task/1?tab=sessions"]) {
      await page.goto(SHARED + url);
      let hidden = 0, checked = 0, samples = [];
      for (let i = 0; i < 90; i++) {
        await page.keyboard.press("Tab");
        const r = await page.evaluate(() => {
          const el = document.activeElement;
          if (!el || el === document.body) return null;
          const r = el.getBoundingClientRect();
          if (r.width === 0 || r.height === 0) return null;
          const sticky = [...document.querySelectorAll(".bar, .sticky-head, .side, .panel, .toc")].filter((s) => ["sticky", "fixed"].includes(getComputedStyle(s).position) && !s.contains(el));
          let covered = 0;
          for (const s of sticky) {
            const q = s.getBoundingClientRect();
            const x = Math.max(0, Math.min(r.right, q.right) - Math.max(r.left, q.left));
            const y = Math.max(0, Math.min(r.bottom, q.bottom) - Math.max(r.top, q.top));
            covered = Math.max(covered, (x * y) / (r.width * r.height));
          }
          const offscreen = r.bottom < 0 || r.top > innerHeight;
          return { covered: Math.round(covered * 100), offscreen, what: el.tagName.toLowerCase() + (el.id ? "#" + el.id : "") + " " + (el.textContent || "").trim().slice(0, 20) };
        });
        if (!r) continue;
        checked++;
        if (r.covered > 0 || r.offscreen) { hidden++; if (samples.length < 4) samples.push(`${r.what} covered=${r.covered}% offscreen=${r.offscreen}`); }
      }
      console.log(`${w}x${h} ${url}: focused ${checked}, obscured or off-screen ${hidden}${samples.length ? " e.g. " + samples.join(" | ") : ""}`);
    }
    await ctx.close();
  }
  await b.close();
})().catch((e) => { console.error(e); process.exit(1); });
