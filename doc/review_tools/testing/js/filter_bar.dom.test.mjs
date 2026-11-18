// DOM test: loads the real src/web/ui/filter_bar.js into jsdom and checks the Escape behaviour.
// Needs jsdom: `npm i --prefix <dir> jsdom` and NODE_PATH=<dir>/node_modules (see run.sh).
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
const { JSDOM } = createRequire(import.meta.url)("jsdom");

const root = new URL("../../../../", import.meta.url).pathname; // repo root
const script = readFileSync(root + "src/web/ui/filter_bar.js", "utf8");

test("Escape dismisses the filter tip and focusing the ? button brings it back", () => {
  const dom = new JSDOM(`<div class="filter-help"><button class="filter-help-btn">?</button><div class="tip">syntax</div></div>`, { runScripts: "outside-only" });
  dom.window.eval(script);
  const { document } = dom.window;
  const help = document.querySelector(".filter-help");
  const btn = document.querySelector(".filter-help-btn");
  help.dispatchEvent(new dom.window.KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
  assert.equal(help.classList.contains("dismissed"), true);
  btn.dispatchEvent(new dom.window.FocusEvent("focusin", { bubbles: true }));
  assert.equal(help.classList.contains("dismissed"), false);
});
