#!/usr/bin/env node
// Form refusals as a screen-reader user would meet them: submit each pop-up form empty
// on the scale-1 server and record what comes back (document, title, landmarks, focus,
// alert, aria-invalid). The New task form is the one that renders its error in place.
"use strict";
const fs = require("fs");
const path = require("path");
const { OWN, OUT, TODAY, browser, DESCRIBE, go } = require("./lib");

const STATE = `(() => ({
  url: location.href, title: document.title, contentType: document.contentType,
  h1: [...document.querySelectorAll("h1")].map((h) => h.textContent.trim()),
  main: document.querySelectorAll("main").length,
  alerts: [...document.querySelectorAll("[role=alert]")].map((a) => a.textContent.trim()),
  errorBox: (() => { const n = document.getElementById("form-error"); return n ? { text: n.textContent.trim(), role: n.getAttribute("role"), focused: document.activeElement === n, tabindex: n.getAttribute("tabindex"), link: n.querySelector("a") && n.querySelector("a").getAttribute("href") } : null; })(),
  invalid: [...document.querySelectorAll("[aria-invalid=true]")].map((c) => c.id + " describedby=" + c.getAttribute("aria-describedby")),
  cue: [...document.querySelectorAll(".bad")].map((c) => c.textContent.trim()),
  dialog: !!document.querySelector("[role=dialog]"),
  bodyText: document.body.innerText.trim().slice(0, 160),
  links: document.querySelectorAll("a").length,
  kept: (() => { const d = document.querySelector("#f-description"); return d ? d.value : null; })(),
}))()`;

(async () => {
  const b = await browser();
  const ctx = await b.newContext({ viewport: { width: 1280, height: 900 } });
  const page = await ctx.newPage();
  const out = [];
  const say = (s) => { out.push(s); console.log(s); };

  say("### 1. New task, empty name (POST /task/new)");
  await page.goto(`${OWN}/project/1?tab=tasks&new=task`);
  say(`  before: active=${await page.evaluate(DESCRIBE)} required attr on name=${await page.evaluate(() => document.getElementById("f-name").required)} aria-required=${await page.evaluate(() => document.getElementById("f-name").getAttribute("aria-required"))}`);
  await page.fill("#f-description", "kept?");
  await page.focus("#f-name");
  await go(page, () => page.keyboard.press("Enter"));
  await page.waitForTimeout(100);
  say("  after: " + JSON.stringify(await page.evaluate(STATE), null, 0));
  say(`  active element: ${await page.evaluate(DESCRIBE)}`);

  say("\n### 2. New note, empty name (POST /task/{id}/notes)");
  await page.goto(`${OWN}/task/1?tab=notes&new=note`);
  say(`  before: active=${await page.evaluate(DESCRIBE)} required attr=${await page.evaluate(() => document.getElementById("f-name").required)}`);
  await page.fill("#f-text", "some text I typed");
  await page.focus("#f-name");
  await go(page, () => page.keyboard.press("Enter"));
  say("  after: " + JSON.stringify(await page.evaluate(STATE), null, 0));
  await page.goBack();
  await page.waitForLoadState("load");
  say(`  Back: url=${new URL(page.url()).search} text kept=${JSON.stringify(await page.evaluate(() => (document.getElementById("f-text") || {}).value))}`);

  say("\n### 3. New session, empty (POST /task/{id}/sessions)");
  await page.goto(`${OWN}/task/1?tab=sessions&new=session`);
  say(`  before: active=${await page.evaluate(DESCRIBE)} required attr=${await page.evaluate(() => document.getElementById("f-start").required)}`);
  await page.focus("#f-start");
  await go(page, () => page.keyboard.press("Enter"));
  say("  after: " + JSON.stringify(await page.evaluate(STATE), null, 0));

  say("\n### 3b. New session, end before start");
  await page.goto(`${OWN}/task/1?tab=sessions&new=session`);
  await page.fill("#f-start", "2026-11-02T10:00");
  await page.fill("#f-end", "2026-11-02T09:00");
  await page.focus("#f-end");
  await go(page, () => page.keyboard.press("Enter"));
  say("  after: " + JSON.stringify(await page.evaluate(STATE), null, 0));

  say("\n### 4. New instruction, empty name (POST /instructions)");
  await page.goto(`${OWN}/instructions?new=instruction`);
  say(`  before: active=${await page.evaluate(DESCRIBE)} required attr=${await page.evaluate(() => document.getElementById("f-name").required)}`);
  await page.focus("#f-name");
  await go(page, () => page.keyboard.press("Enter"));
  say("  after: " + JSON.stringify(await page.evaluate(STATE), null, 0));

  say("\n### 5. New instruction, name with a space");
  await page.goto(`${OWN}/instructions?new=instruction`);
  await page.fill("#f-name", "Bad Name");
  await go(page, () => page.keyboard.press("Enter"));
  say("  after: " + JSON.stringify(await page.evaluate(STATE), null, 0));

  say("\n### 6. Instruction save with an empty prompt (POST /instruction/{name})");
  await page.goto(`${OWN}/instruction/develop?tab=settings`);
  const promptSel = "#f-prompt";
  if (await page.$(promptSel)) {
    await page.fill(promptSel, "");
    const save = await page.$('button[name="do"][value="save"]');
    await go(page, () => save.click());
    say("  after: " + JSON.stringify(await page.evaluate(STATE), null, 0));
  } else say("  no prompt field on this page");

  say("\n### 7. New project pop-up with an empty name (POST, rendered in place?)");
  await page.goto(`${OWN}/org/1?tab=projects&new=project`);
  say(`  before: active=${await page.evaluate(DESCRIBE)}`);
  const nameField = await page.$("#f-name");
  if (nameField) {
    await page.focus("#f-name");
    await go(page, () => page.keyboard.press("Enter"));
    await page.waitForTimeout(100);
    say("  after: " + JSON.stringify(await page.evaluate(STATE), null, 0));
  } else say(`  fields: ${await page.evaluate(() => [...document.querySelectorAll("[role=dialog] input,[role=dialog] select")].map((i) => i.id || i.name).join(","))}`);

  say("\n### 8. Task edit with an empty name (POST /task/{id}/edit)");
  await page.goto(`${OWN}/task/1/edit`);
  await page.fill("#f-name", "");
  await page.focus("#f-name");
  await go(page, () => page.keyboard.press("Enter"));
  await page.waitForTimeout(100);
  say("  after: " + JSON.stringify(await page.evaluate(STATE), null, 0));
  say(`  active: ${await page.evaluate(DESCRIBE)}`);

  await b.close();
  fs.mkdirSync(OUT, { recursive: true });
  fs.writeFileSync(path.join(OUT, "forms.log"), out.join("\n"));
})().catch((e) => { console.error(e); process.exit(1); });
