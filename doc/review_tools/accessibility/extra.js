#!/usr/bin/env node
// Follow-ups: what overflows at 320px (and is it visible), footer link spacing for 2.5.8,
// the instruction page's heading list, the agent compose dialog, and 320px screenshots.
"use strict";
const fs = require("fs");
const path = require("path");
const { SHARED, OUT, TODAY, browser } = require("./lib");

(async () => {
  const b = await browser();
  const out = [];
  const say = (s) => { out.push(s); console.log(s); };
  const shots = path.join(OUT, "shots");
  fs.mkdirSync(shots, { recursive: true });

  say("### Overflow at 320px: widest elements by right edge, excluding .sr (shared)");
  let ctx = await b.newContext({ viewport: { width: 320, height: 700 } });
  let page = await ctx.newPage();
  for (const url of ["/", "/org/1?tab=projects", "/board/1/info", `/board/2?date=${TODAY}`, "/task/1", "/settings"]) {
    await page.goto(SHARED + url);
    const r = await page.evaluate(() => {
      const vw = document.documentElement.clientWidth;
      const items = [];
      for (const el of document.querySelectorAll("body *")) {
        if (el.closest(".sr,.palette,[hidden],template,script,style") || el.classList.contains("sr")) continue;
        const r = el.getBoundingClientRect();
        if (r.width === 0) continue;
        if (r.right > vw + 1) items.push({ el: el.tagName.toLowerCase() + (el.className && typeof el.className === "string" ? "." + el.className.trim().split(/\s+/).slice(0, 2).join(".") : "") + (el.id ? "#" + el.id : ""), right: Math.round(r.right), width: Math.round(r.width), text: el.textContent.trim().replace(/\s+/g, " ").slice(0, 30) });
      }
      items.sort((a, b) => b.right - a.right);
      const bar = document.querySelector(".bar");
      const bcs = getComputedStyle(bar);
      const html = document.documentElement;
      return { vw, sw: html.scrollWidth, bodyOverflowX: getComputedStyle(document.body).overflowX, barOverflow: bcs.overflowX, barFlexWrap: bcs.flexWrap, barGap: bcs.gap, barWidth: Math.round(bar.getBoundingClientRect().width), navWrap: getComputedStyle(document.querySelector(".bar nav")).flexWrap, searchBtnRight: Math.round(document.querySelector(".search-btn").getBoundingClientRect().right), top: items.slice(0, 8) };
    });
    say(`  ${url}: scrollWidth=${r.sw} bar width=${r.barWidth} bar overflow-x=${r.barOverflow} flex-wrap=${r.barFlexWrap} nav wrap=${r.navWrap} search-btn right=${r.searchBtnRight}`);
    for (const t of r.top) say(`     right=${t.right} w=${t.width} ${t.el} "${t.text}"`);
    await page.screenshot({ path: path.join(shots, `320-${url.replace(/[^a-z0-9]+/gi, "_")}.png`), fullPage: false });
  }
  // can the user scroll the bar sideways / is the Settings link reachable by touch at 320?
  await page.goto(SHARED + "/");
  const settings = await page.evaluate(() => { const a = [...document.querySelectorAll(".bar nav a")].pop(); const r = a.getBoundingClientRect(); return { left: Math.round(r.left), right: Math.round(r.right), vw: document.documentElement.clientWidth }; });
  say(`  Settings nav link at 320px: left=${settings.left} right=${settings.right} viewport=${settings.vw} (off-screen until the page is scrolled sideways)`);
  await ctx.close();

  say("\n### 400px: same");
  ctx = await b.newContext({ viewport: { width: 400, height: 700 } });
  page = await ctx.newPage();
  await page.goto(SHARED + "/");
  say("  " + JSON.stringify(await page.evaluate(() => { const a = [...document.querySelectorAll(".bar nav a")].pop(); const r = a.getBoundingClientRect(); return { settingsRight: Math.round(r.right), searchRight: Math.round(document.querySelector(".search-btn").getBoundingClientRect().right), vw: document.documentElement.clientWidth, sw: document.documentElement.scrollWidth }; })));
  await page.screenshot({ path: path.join(shots, "400-home.png") });
  await page.goto(SHARED + "/org/1?tab=projects");
  say("  projects table at 400: " + JSON.stringify(await page.evaluate(() => { const t = document.querySelector("table.projects"); const p = document.querySelector(".path"); return { tableWidth: t && Math.round(t.getBoundingClientRect().width), pathWidth: p && Math.round(p.getBoundingClientRect().width), pathScroll: p && p.scrollWidth, cellMaxWidth: p && getComputedStyle(p.closest("td")).maxWidth, sw: document.documentElement.scrollWidth }; })));
  await page.screenshot({ path: path.join(shots, "400-org-projects.png") });
  await ctx.close();

  say("\n### Footer link spacing (1280px): vertical gap between stacked links in .foot-sub and .foot-orgs");
  ctx = await b.newContext({ viewport: { width: 1280, height: 900 } });
  page = await ctx.newPage();
  await page.goto(SHARED + "/");
  say("  " + JSON.stringify(await page.evaluate(() => {
    const li = document.querySelector(".foot-orgs > li");
    const org = li.querySelector("a.u").getBoundingClientRect();
    const subs = [...li.querySelectorAll(".foot-sub a")].map((a) => a.getBoundingClientRect());
    const cols = [...document.querySelectorAll(".foot-col > ul > li > a.u")].slice(0, 3).map((a) => a.getBoundingClientRect());
    const gap = (a, b2) => Math.round(b2.top - a.bottom);
    return { orgLink: { h: Math.round(org.height) }, firstSub: { h: Math.round(subs[0].height), gapFromOrg: gap(org, subs[0]) }, subGaps: subs.slice(1).map((s, i) => gap(subs[i], s)), subFont: getComputedStyle(li.querySelector(".foot-sub a")).fontSize, subLineHeight: getComputedStyle(li.querySelector(".foot-sub a")).lineHeight, colGaps: cols.slice(1).map((c, i) => gap(cols[i], c)), colH: cols.map((c) => Math.round(c.height)) };
  })));
  say("  rule: a 24px circle centred on a 17px-tall link reaches 3.5px above and below it; a neighbour closer than that fails the spacing exception");

  say("\n### Pick banner edge: .pickbar border-left-width in normal colours (pick_bar.css says 4px)");
  await page.goto(`${SHARED}/board/2?date=${TODAY}&pick=2`);
  say("  " + (await page.evaluate(() => { const p = document.getElementById("pick"); const cs = getComputedStyle(p); return `border-left-width=${cs.borderLeftWidth} border-left-color=${cs.borderLeftColor} data-mode=${p.dataset.mode}`; })));
  say("\n### Instruction page headings");
  await page.goto(SHARED + "/instruction/develop");
  say("  " + (await page.evaluate(() => [...document.querySelectorAll("h1,h2,h3,h4")].filter((h) => !h.closest("footer")).map((h) => h.tagName + ":" + h.textContent.trim().slice(0, 30)).join(" | "))));

  say("\n### Agent compose dialog (/task/1?tab=agent&new=agent)");
  await page.goto(SHARED + "/task/1?tab=agent&new=agent");
  say("  " + JSON.stringify(await page.evaluate(() => { const d = document.querySelector(".modal-box[role=dialog]"); return d ? { title: document.getElementById(d.getAttribute("aria-labelledby")).textContent, fields: [...d.querySelectorAll("input:not([type=hidden]),select,textarea,button")].map((c) => (c.id || c.name || c.textContent.trim().slice(0, 12)) + (c.labels && c.labels.length ? "" : c.getAttribute("aria-label") ? "" : c.tagName === "BUTTON" ? "" : " UNLABELLED")), autofocus: d.querySelector("[autofocus]") && d.querySelector("[autofocus]").id, active: document.activeElement.id, cbButtons: d.querySelectorAll(".cb-button").length } : "no dialog"; })));

  say("\n### Day input accessible name and role (agenda)");
  await page.goto(`${SHARED}/board/2?date=${TODAY}`);
  const snap = await page.accessibility.snapshot({ root: await page.$(".day-pick") }).catch(() => null);
  say("  " + JSON.stringify(snap));
  say("\n### Pick banner and card accessibility snapshot (agenda pick)");
  await page.goto(`${SHARED}/board/2?date=${TODAY}&pick=2`);
  const snap2 = await page.accessibility.snapshot({ root: await page.$("#pick") }).catch(() => null);
  say("  pickbar: " + JSON.stringify(snap2));
  const snap3 = await page.accessibility.snapshot({ root: await page.$(".tcard[aria-current]") }).catch(() => null);
  say("  picked card: " + JSON.stringify(snap3));
  const snap4 = await page.accessibility.snapshot({ root: await page.$(".side") }).catch(() => null);
  say("  side lists (headings inside summary?): " + JSON.stringify(snap4).slice(0, 900));
  say("\n### Search palette option snapshot");
  await page.goto(SHARED + "/");
  await page.keyboard.press("/");
  await page.fill(".palette-input", "task-1");
  await page.waitForSelector("#palette-list li[role=option]");
  const snap5 = await page.accessibility.snapshot({ root: await page.$("#palette-list") }).catch(() => null);
  say("  " + JSON.stringify(snap5).slice(0, 700));
  await b.close();
  fs.writeFileSync(path.join(OUT, "extra.log"), out.join("\n"));
})().catch((e) => { console.error(e); process.exit(1); });
