#!/usr/bin/env node
// The dialog pages again, 700ms after load (past the 250ms modal-in animation axe sampled
// through), and once more with reduced motion: are the modal colour-contrast hits real?
"use strict";
const fs = require("fs");
const { SHARED, browser } = require("./lib");
const axeSource = fs.readFileSync(require.resolve("axe-core/axe.min.js"), "utf8");
const PAGES = ["/org/1?tab=tasks&new=task", "/project/1?tab=tasks&new=task", "/task/1?tab=notes&new=note", "/task/1?tab=sessions&new=session", "/task/1?tab=agent&new=agent", "/org/1?tab=report&from=2026-10-27&to=2026-11-02&share=1"];
(async () => {
  const b = await browser();
  for (const [label, opts, wait] of [["light, wait 700ms", { colorScheme: "light" }, 700], ["light, reduced motion, wait 100ms", { colorScheme: "light", reducedMotion: "reduce" }, 100], ["dark, wait 700ms", { colorScheme: "dark" }, 700]]) {
    const ctx = await b.newContext({ ...opts, viewport: { width: 1280, height: 900 } });
    const page = await ctx.newPage();
    console.log(`### ${label}`);
    for (const url of PAGES) {
      await page.goto(SHARED + url, { waitUntil: "load" });
      await page.waitForTimeout(wait);
      await page.addScriptTag({ content: axeSource });
      const r = await page.evaluate(() => window.axe.run(document, { runOnly: { type: "tag", values: ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa", "best-practice"] } }));
      const v = r.violations.map((x) => `${x.id}(${x.nodes.length}: ${x.nodes.map((n) => n.target[0] + (n.any[0] && n.any[0].data ? " " + n.any[0].data.fgColor + "/" + n.any[0].data.bgColor + "=" + n.any[0].data.contrastRatio : "")).join("; ")})`).join(" ");
      console.log(`  ${url}: ${r.violations.length ? v : "0 violations"}`);
    }
    await ctx.close();
  }
  await b.close();
})().catch((e) => { console.error(e); process.exit(1); });
