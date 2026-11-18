#!/usr/bin/env node
// Structure audit of the emitted DOM per route: title, h1 count, heading order, landmarks
// and their labels, table captions, aria-current values, duplicate ids, aria-describedby /
// aria-labelledby targets, role=alert, inert parts under a dialog, autofocus, images.
"use strict";
const fs = require("fs");
const path = require("path");
const { SHARED, OUT, ROUTES, browser } = require("./lib");

const AUDIT = `(() => {
  const q = (s) => [...document.querySelectorAll(s)];
  const vis = (el) => !el.closest("[hidden],template");
  const hs = q("h1,h2,h3,h4,h5,h6").filter(vis).map((h) => h.tagName[1] + ":" + h.textContent.trim().replace(/\\s+/g, " ").slice(0, 32) + (h.closest("summary") ? "[in summary]" : "") + (h.closest("footer") ? "[foot]" : ""));
  const levels = hs.map((h) => +h[0]);
  let skips = 0;
  for (let i = 1; i < levels.length; i++) if (levels[i] > levels[i - 1] + 1) skips++;
  const ids = q("[id]").map((e) => e.id);
  const dup = ids.filter((id, i) => ids.indexOf(id) !== i);
  const missingRefs = [];
  for (const attr of ["aria-describedby", "aria-labelledby", "aria-controls", "aria-activedescendant", "for", "list"]) {
    for (const el of q("[" + attr + "]")) {
      for (const id of el.getAttribute(attr).split(/\\s+/).filter(Boolean)) if (!document.getElementById(id)) missingRefs.push(attr + "=" + id);
    }
  }
  const dialog = document.querySelector("[role=dialog]");
  const inert = dialog ? { bar: !!document.querySelector("header.bar[inert]"), main: !!document.querySelector("main[inert]"), skip: !!document.querySelector("a.skip[inert]"), foot: !!document.querySelector("footer.site-foot[inert]"), modalJs: !!document.querySelector('script[src="/modal.js"]'), labelled: dialog.getAttribute("aria-labelledby") && !!document.getElementById(dialog.getAttribute("aria-labelledby")), autofocus: !!dialog.querySelector("[autofocus]"), active: document.activeElement && document.activeElement !== document.body ? document.activeElement.tagName + "#" + document.activeElement.id : "body" } : null;
  const tables = q("table").map((t) => ({ caption: t.caption ? t.caption.textContent.trim().slice(0, 40) : null, srCaption: !!(t.caption && t.caption.classList.contains("sr")), ths: t.querySelectorAll("th").length, roles: t.getAttribute("role") }));
  const controls = q("input:not([type=hidden]),select,textarea").filter(vis).map((c) => {
    const lab = c.labels && c.labels.length ? "label" : c.getAttribute("aria-label") ? "aria-label" : c.getAttribute("aria-labelledby") ? "labelledby" : "NONE";
    return { id: c.id || c.name, lab, req: c.required || c.getAttribute("aria-required") === "true", invalid: c.getAttribute("aria-invalid"), desc: c.getAttribute("aria-describedby") };
  });
  return {
    title: document.title,
    lang: document.documentElement.lang,
    h1: q("h1").filter(vis).length,
    headings: hs, skips,
    main: q("main").length,
    navs: q("nav").filter(vis).map((n) => n.getAttribute("aria-label") || n.getAttribute("aria-labelledby") || "UNLABELLED"),
    asides: q("aside").filter(vis).map((n) => n.getAttribute("aria-label") || (n.getAttribute("aria-labelledby") ? "by:" + n.getAttribute("aria-labelledby") : "UNLABELLED")),
    regions: q("section[aria-labelledby],section[aria-label]").filter(vis).length,
    search: q("[role=search]").length,
    current: q("[aria-current]").filter(vis).map((e) => e.getAttribute("aria-current") + ":" + e.tagName.toLowerCase() + "." + (e.className || "").toString().split(" ")[0] + ":" + e.textContent.trim().slice(0, 18)),
    dup, missingRefs,
    alerts: q("[role=alert]").map((a) => a.textContent.trim().slice(0, 60)),
    status: q("[role=status],[aria-live]").filter(vis).map((a) => a.getAttribute("role") || a.getAttribute("aria-live")),
    dialog: inert,
    tables, controls,
    unlabelledControls: controls.filter((c) => c.lab === "NONE").length,
    imgs: q("img").length, svgNoHidden: q("svg:not([aria-hidden=true])").length,
    skipLink: !!document.querySelector("a.skip[href='#main']"),
    titleInBody: !!document.body.querySelector("title"),
  };
})()`;

(async () => {
  const b = await browser();
  const ctx = await b.newContext({ viewport: { width: 1280, height: 900 } });
  const page = await ctx.newPage();
  const out = {};
  for (const [name, url] of ROUTES) {
    await page.goto(SHARED + url, { waitUntil: "load" });
    await page.waitForTimeout(150);
    const r = await page.evaluate(AUDIT);
    out[name] = { url, ...r };
    const flags = [];
    if (r.h1 !== 1) flags.push(`h1=${r.h1}`);
    if (r.main !== 1) flags.push(`main=${r.main}`);
    if (r.skips) flags.push(`heading-skips=${r.skips}`);
    if (r.dup.length) flags.push(`dupIds=${r.dup.join(",")}`);
    if (r.missingRefs.length) flags.push(`missingRefs=${r.missingRefs.join(",")}`);
    if (r.navs.includes("UNLABELLED")) flags.push("unlabelled-nav");
    if (r.unlabelledControls) flags.push(`unlabelledControls=${r.unlabelledControls}`);
    const pages = r.current.filter((c) => c.startsWith("page")).length;
    if (pages > 1) flags.push(`aria-current=page x${pages}`);
    if (r.tables.some((t) => !t.caption)) flags.push("table-without-caption");
    if (r.dialog && !(r.dialog.bar && r.dialog.main && r.dialog.skip && r.dialog.foot)) flags.push(`inert:${JSON.stringify(r.dialog)}`);
    if (!r.skipLink) flags.push("no-skip-link");
    if (r.svgNoHidden) flags.push(`svg-not-hidden=${r.svgNoHidden}`);
    console.log(`${name.padEnd(24)} "${r.title}" current=[${r.current.join(" | ")}] alerts=${r.alerts.length} ${flags.join(" ")}`);
  }
  await b.close();
  fs.mkdirSync(OUT, { recursive: true });
  fs.writeFileSync(path.join(OUT, "audit.json"), JSON.stringify(out, null, 1));
})().catch((e) => { console.error(e); process.exit(1); });
