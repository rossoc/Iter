// Pure-function test with node:test, no DOM. The two `ready` helpers are copied
// from src/web/board.js:26 and src/web/org.js:8 (they are closures today; to test
// the real source they would have to be exported, e.g. `window.iter = { ready }`).
import { test } from "node:test";
import assert from "node:assert/strict";

const boardReady = function (v) { return /^\d{4}-\d\d-\d\d$/.test(v) && +v.slice(0, 4) >= 1970; };
const orgReady = function (v) { return v === "" || (/^\d{4}-\d\d-\d\d$/.test(v) && +v.slice(0, 4) >= 1970); };

test("a full date from 1970 on is ready; partial years and old years are not", () => {
  for (const ready of [boardReady, orgReady]) {
    assert.equal(ready("2026-11-02"), true);
    assert.equal(ready("1970-01-01"), true);
    assert.equal(ready("1969-12-31"), false);
    assert.equal(ready("0202-11-02"), false, "Chrome mid-typing year");
    assert.equal(ready("2026-1-2"), false);
    assert.equal(ready("2026-13-45"), true, "GAP: the regex does not check month/day ranges; the server does");
  }
});

test("only the report form treats an empty field as ready (open-ended range)", () => {
  assert.equal(orgReady(""), true);
  assert.equal(boardReady(""), false);
});
