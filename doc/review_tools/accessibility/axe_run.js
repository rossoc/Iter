#!/usr/bin/env node
// axe-core on every route, light and dark, plus two script-only states (search palette
// open with results; filter syntax tip open). JSON per run under out/axe/, a summary
// table on stdout and in out/axe_summary.md.
"use strict";
const fs = require("fs");
const path = require("path");
const { SHARED, OUT, ROUTES, browser } = require("./lib");
const axeSource = fs.readFileSync(require.resolve("axe-core/axe.min.js"), "utf8");

const DIR = path.join(OUT, "axe");
fs.mkdirSync(DIR, { recursive: true });

const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa", "best-practice"];

async function runAxe(page) {
  await page.addScriptTag({ content: axeSource });
  return page.evaluate((tags) => window.axe.run(document, { runOnly: { type: "tag", values: tags } }), TAGS);
}

(async () => {
  const b = await browser();
  const rows = [];
  const totals = { light: 0, dark: 0 };
  const byRule = {};
  for (const scheme of ["light", "dark"]) {
    const ctx = await b.newContext({ colorScheme: scheme, viewport: { width: 1280, height: 900 } });
    const page = await ctx.newPage();
    const states = [...ROUTES.map(([n, u]) => [n, u, null]),
      ["home+palette", "/", "palette"],
      ["org-tasks+filter-tip", "/org/1?tab=tasks", "tip"]];
    for (const [name, url, state] of states) {
      await page.goto(SHARED + url, { waitUntil: "load" });
      if (state === "palette") {
        await page.keyboard.press("/");
        await page.waitForSelector("#palette:not([hidden])");
        await page.fill(".palette-input", "task");
        await page.waitForSelector("#palette-list li[role=option]");
        await page.waitForTimeout(400);
      } else if (state === "tip") {
        await page.focus(".filter-help-btn");
        await page.waitForTimeout(100);
      }
      const r = await runAxe(page);
      const file = path.join(DIR, `${name}.${scheme}.json`);
      fs.writeFileSync(file, JSON.stringify({ url, scheme, violations: r.violations, incomplete: r.incomplete.map((i) => ({ id: i.id, nodes: i.nodes.length })) }, null, 1));
      const n = r.violations.reduce((s, v) => s + v.nodes.length, 0);
      totals[scheme] += n;
      const ids = r.violations.map((v) => `${v.id}(${v.impact},${v.nodes.length})`).join(" ");
      for (const v of r.violations) {
        byRule[v.id] = byRule[v.id] || { impact: v.impact, pages: new Set(), nodes: 0, sample: v.nodes[0] && v.nodes[0].target.join(" ") };
        byRule[v.id].pages.add(`${name}/${scheme}`);
        byRule[v.id].nodes += v.nodes.length;
      }
      const inc = r.incomplete.filter((i) => i.id === "color-contrast").map((i) => i.nodes.length)[0] || 0;
      rows.push({ name, url, scheme, violations: n, ids, incomplete: inc });
      process.stdout.write(`${scheme.padEnd(5)} ${name.padEnd(24)} ${String(n).padStart(2)} ${ids}\n`);
    }
    await ctx.close();
  }
  await b.close();
  let md = "| Page | URL | Light | Dark | Rules (light / dark) |\n|---|---|---|---|---|\n";
  const names = [...new Set(rows.map((r) => r.name))];
  for (const name of names) {
    const l = rows.find((r) => r.name === name && r.scheme === "light");
    const d = rows.find((r) => r.name === name && r.scheme === "dark");
    md += `| ${name} | \`${l.url}\` | ${l.violations} | ${d.violations} | ${l.ids || "-"}${d.ids && d.ids !== l.ids ? " / " + d.ids : ""} |\n`;
  }
  md += `\nTotal violation nodes: light ${totals.light}, dark ${totals.dark}.\n\n| Rule | Impact | Pages | Nodes | Sample target |\n|---|---|---|---|---|\n`;
  for (const [id, v] of Object.entries(byRule)) md += `| ${id} | ${v.impact} | ${v.pages.size} | ${v.nodes} | \`${v.sample}\` |\n`;
  fs.writeFileSync(path.join(OUT, "axe_summary.md"), md);
  console.log(`\nTOTAL light=${totals.light} dark=${totals.dark}; summary in ${path.join(OUT, "axe_summary.md")}`);
})().catch((e) => { console.error(e); process.exit(1); });
