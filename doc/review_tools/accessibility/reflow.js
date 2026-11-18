#!/usr/bin/env node
// Reflow (WCAG 1.4.10): every route at 320 and 400 CSS px wide. Reports horizontal page
// scroll and elements whose right edge passes the viewport, and whether text is clipped
// (scrollWidth > clientWidth with overflow hidden and no ellipsis).
"use strict";
const fs = require("fs");
const path = require("path");
const { SHARED, OUT, ROUTES, browser } = require("./lib");

const CHECK = `(() => {
  const vw = document.documentElement.clientWidth;
  const sw = Math.max(document.documentElement.scrollWidth, document.body.scrollWidth);
  const over = [];
  const clipped = [];
  for (const el of document.querySelectorAll("body *")) {
    if (el.closest("[hidden],.palette,script,style,template")) continue;
    const r = el.getBoundingClientRect();
    if (r.width === 0 && r.height === 0) continue;
    const cs = getComputedStyle(el);
    if (r.right > vw + 1 && cs.position !== "fixed") {
      over.push(el.tagName.toLowerCase() + (el.className && typeof el.className === "string" ? "." + el.className.trim().split(/\\s+/).slice(0,2).join(".") : "") + " right=" + Math.round(r.right));
    }
    if (el.scrollWidth > el.clientWidth + 2 && /hidden|clip/.test(cs.overflowX) && cs.textOverflow !== "ellipsis" && el.children.length === 0 && el.textContent.trim()) {
      clipped.push(el.tagName.toLowerCase() + (el.className && typeof el.className === "string" ? "." + el.className.trim().split(/\\s+/).slice(0,2).join(".") : "") + " text=" + el.textContent.trim().slice(0, 30));
    }
  }
  return { vw, sw, over: over.slice(0, 6), clipped: clipped.slice(0, 6) };
})()`;

(async () => {
  const b = await browser();
  const out = [];
  for (const width of [320, 400]) {
    const ctx = await b.newContext({ viewport: { width, height: 800 } });
    const page = await ctx.newPage();
    for (const [name, url] of ROUTES) {
      await page.goto(SHARED + url, { waitUntil: "load" });
      const r = await page.evaluate(CHECK);
      const bad = r.sw > r.vw + 1 || r.over.length || r.clipped.length;
      out.push({ width, name, url, ...r, bad });
      console.log(`${width} ${name.padEnd(24)} scroll=${r.sw}/${r.vw}${r.over.length ? " OVER " + r.over.join("; ") : ""}${r.clipped.length ? " CLIP " + r.clipped.join("; ") : ""}`);
    }
    await ctx.close();
  }
  await b.close();
  fs.mkdirSync(OUT, { recursive: true });
  fs.writeFileSync(path.join(OUT, "reflow.json"), JSON.stringify(out, null, 1));
  console.log(`\npages with any issue: ${out.filter((o) => o.bad).length} of ${out.length}`);
})().catch((e) => { console.error(e); process.exit(1); });
