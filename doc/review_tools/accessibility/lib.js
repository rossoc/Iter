// Shared bits of the accessibility review scripts: the route map, a browser, and a
// label for a focused element.
"use strict";
const path = require("path");
const { chromium } = require("playwright");

const SHARED = process.env.A11Y_SHARED || "http://127.0.0.1:3999"; // scale 4, GETs only
const OWN = process.env.A11Y_OWN || "http://127.0.0.1:4020"; // scale 1, may POST
const OUT = path.join(__dirname, "out");

// Real ids from the scale-4 store (sqlite3 $SCRATCH/iter-home/iter/iter.db): org 1 has
// projects 1, 9, 17; board 1 has task 45 scheduled today 07:00 and task 27 placed in the
// matrix; task 1 is wip with 7 sessions; project 5 has no organization.
const TODAY = "2026-11-02";
const ROUTES = [
  ["home", "/"],
  ["settings", "/settings"],
  ["org-info", "/org/1"],
  ["org-projects", "/org/1?tab=projects"],
  ["org-projects-new", "/org/1?tab=projects&new=project"],
  ["org-tasks", "/org/1?tab=tasks"],
  ["org-tasks-new", "/org/1?tab=tasks&new=task"],
  ["org-notes", "/org/1?tab=notes"],
  ["org-report", `/org/1?tab=report&from=2026-10-27&to=${TODAY}`],
  ["org-report-bad", `/org/1?tab=report&from=nonsense&to=${TODAY}`],
  ["org-share", `/org/1?tab=report&from=2026-10-27&to=${TODAY}&share=1`],
  ["org-edit", "/org/1/edit"],
  ["org-note", `/org/1/notes/${TODAY}`],
  ["org-note-edit", `/org/1/notes/${TODAY}?tab=edit`],
  ["project-info", "/project/1"],
  ["project-loose", "/project/5"],
  ["project-tasks", "/project/1?tab=tasks"],
  ["project-tasks-new", "/project/1?tab=tasks&new=task"],
  ["project-notes", "/project/1?tab=notes"],
  ["project-edit", "/project/1/edit"],
  ["project-note", `/project/1/notes/${TODAY}`],
  ["task-info", "/task/1"],
  ["task-sessions", "/task/1?tab=sessions"],
  ["task-sessions-new", "/task/1?tab=sessions&new=session"],
  ["task-notes", "/task/1?tab=notes"],
  ["task-notes-new", "/task/1?tab=notes&new=note"],
  ["task-agent", "/task/1?tab=agent"],
  ["task-agent-new", "/task/1?tab=agent&new=agent"],
  ["task-wiki", "/task/1?tab=wiki"],
  ["task-edit", "/task/1/edit"],
  ["agents", "/agents"],
  ["boards", "/boards"],
  ["board-agenda", `/board/2?date=${TODAY}`],
  ["board-agenda-folded", `/board/2?date=${TODAY}&side=off`],
  ["board-agenda-pick", `/board/2?date=${TODAY}&pick=2`],
  ["board-agenda-moved", `/board/2?date=${TODAY}&moved=2`],
  ["board-matrix", "/board/2/matrix"],
  ["board-matrix-pick", "/board/2/matrix?pick=24"],
  ["board-matrix-moved", "/board/2/matrix?moved=24"],
  ["board-info", "/board/1/info"],
  ["board-edit", "/board/1/edit"],
  ["instructions", "/instructions"],
  ["instructions-new", "/instructions?new=instruction"],
  ["instruction", "/instruction/develop"],
  ["error-400", "/org/x"],
  ["error-404", "/org/99999"],
  ["error-404-unmatched", "/nope"],
];

async function browser() {
  return chromium.launch({ channel: "chrome" });
}

// A short description of the active element: tag, id/class, name, size.
const DESCRIBE = `(() => {
  const el = document.activeElement;
  if (!el || el === document.body) return "BODY";
  const r = el.getBoundingClientRect();
  const cs = getComputedStyle(el);
  const name = (el.getAttribute("aria-label") || el.textContent || "").trim().replace(/\\s+/g, " ").slice(0, 50);
  const id = el.id ? "#" + el.id : "";
  const cls = el.className && typeof el.className === "string" ? "." + el.className.trim().split(/\\s+/).join(".") : "";
  const ring = cs.outlineStyle !== "none" && parseFloat(cs.outlineWidth) > 0 ? "ring" : "no-ring";
  const inert = el.closest("[inert]") ? " INERT" : "";
  return el.tagName.toLowerCase() + id + cls + " \\"" + name + "\\" " + Math.round(r.width) + "x" + Math.round(r.height) + " " + ring + inert;
})()`;

// An action that navigates: wait for the new document, not the old one's load state.
async function go(page, action) {
  await Promise.all([page.waitForNavigation({ waitUntil: "load" }), action()]);
  await page.waitForTimeout(80);
}

module.exports = { SHARED, OWN, OUT, ROUTES, TODAY, browser, DESCRIBE, go };
