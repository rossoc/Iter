#!/usr/bin/env node
// A3b of the Oct review: from the report's To date, how many Tabs reach Apply, and is it visible then?
"use strict";
const { SHARED, TODAY, browser, DESCRIBE } = require("./lib");
(async () => {
  const b = await browser();
  const page = await (await b.newContext({ viewport: { width: 1280, height: 900 } })).newPage();
  await page.goto(`${SHARED}/org/1?tab=report&from=2026-10-27&to=${TODAY}`);
  await page.focus('input[name="to"]');
  for (let i = 1; i <= 5; i++) {
    await page.keyboard.press("Tab");
    const d = await page.evaluate(DESCRIBE);
    const extra = await page.evaluate(() => { const el = document.activeElement; const cs = getComputedStyle(el); return `pos=${cs.position} clip=${cs.clipPath}`; });
    console.log(`  Tab x${i}: ${d} ${extra}`);
    if (d.startsWith("button")) break;
  }
  await page.goto(`${SHARED}/board/2?date=${TODAY}`);
  await page.focus("#day-input");
  for (let i = 1; i <= 5; i++) {
    await page.keyboard.press("Tab");
    const d = await page.evaluate(DESCRIBE);
    const extra = await page.evaluate(() => { const el = document.activeElement; const cs = getComputedStyle(el); return `pos=${cs.position} clip=${cs.clipPath} visible-ring=${cs.outlineStyle !== "none" && cs.clipPath === "none"}`; });
    console.log(`  agenda Tab x${i}: ${d} ${extra}`);
    if (d.startsWith("button")) break;
  }
  await b.close();
})();
