#!/usr/bin/env node
// Computed WCAG 2.x contrast for the token pairs of src/web/base.css, light and dark.
// color-mix(in oklab, A p%, B) is reproduced in OKLab (CSS Color 4 matrices); the sRGB
// mixes of ui/*.css (status borders, error background) are reproduced in sRGB.
// Usage: node contrast.js <path to base.css>
"use strict";
const fs = require("fs");

const css = fs.readFileSync(process.argv[2], "utf8");

function tokens(block) {
  const out = {};
  for (const m of block.matchAll(/--([a-z0-9-]+):\s*([^;}]+)/g)) out[m[1]] = m[2].trim();
  return out;
}
const lightBlock = css.match(/:root\{\s*color-scheme[\s\S]*?\n\}/)[0];
const darkBlock = css.match(/@media \(prefers-color-scheme:dark\)\{:root\{[\s\S]*?\}\}/)[0];
const light = tokens(lightBlock);
const dark = Object.assign({}, light, tokens(darkBlock));

function hex(c) {
  c = c.trim();
  const m = c.match(/^#([0-9a-f]{3,8})$/i);
  if (!m) return null;
  let h = m[1];
  if (h.length === 3) h = h.split("").map((x) => x + x).join("");
  return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16) / 255);
}
const lin = (c) => (c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4));
const gam = (c) => (c <= 0.0031308 ? 12.92 * c : 1.055 * Math.pow(c, 1 / 2.4) - 0.055);
function toOklab([r, g, b]) {
  const [R, G, B] = [lin(r), lin(g), lin(b)];
  const l = Math.cbrt(0.4122214708 * R + 0.5363325363 * G + 0.0514459929 * B);
  const m = Math.cbrt(0.2119034982 * R + 0.6806995451 * G + 0.1073969566 * B);
  const s = Math.cbrt(0.0883024619 * R + 0.2817188376 * G + 0.6299787005 * B);
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ];
}
function fromOklab([L, a, bb]) {
  const l = Math.pow(L + 0.3963377774 * a + 0.2158037573 * bb, 3);
  const m = Math.pow(L - 0.1055613458 * a - 0.0638541728 * bb, 3);
  const s = Math.pow(L - 0.0894841775 * a - 1.291485548 * bb, 3);
  const R = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
  const G = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
  const B = -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s;
  return [R, G, B].map((c) => Math.min(1, Math.max(0, gam(c))));
}
function mixOklab(a, pa, b) {
  const A = toOklab(a), B = toOklab(b), pb = 1 - pa;
  return fromOklab([0, 1, 2].map((i) => A[i] * pa + B[i] * pb));
}
function mixSrgb(a, pa, b) {
  // alpha-aware: `transparent` is b === null -> result has alpha pa over whatever is below
  return [0, 1, 2].map((i) => a[i] * pa + (b ? b[i] : 0) * (1 - pa));
}
function over(fg, alpha, bg) {
  return [0, 1, 2].map((i) => fg[i] * alpha + bg[i] * (1 - alpha));
}
function lum([r, g, b]) {
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}
function ratio(a, b) {
  const [x, y] = [lum(a), lum(b)];
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}
function toHex(c) {
  return "#" + c.map((x) => Math.round(x * 255).toString(16).padStart(2, "0")).join("");
}

function resolve(t) {
  const col = (name) => hex(t[name]);
  const bg = col("bg");
  return {
    bg, bg2: col("bg-2"), bg3: col("bg-3"), line: col("line"), line2: col("line-2"),
    controlLine: col("control-line"), fg: col("fg"), muted: col("muted"), faint: col("faint"),
    accent: col("accent"), ok: col("ok"), danger: col("danger"), onAccent: col("on-accent"),
    accentTint: mixOklab(col("accent"), 0.12, bg), // --accent-tint: color-mix(in oklab, accent 12%, bg)
    errorBg: mixSrgb(col("danger"), 0.10, bg), // .notice.error background
  };
}

const rows = [];
function row(what, pair, kind) {
  const [l, d] = pair;
  rows.push({ what, kind, light: l, dark: d });
}
const fmt = (x) => x.toFixed(2);
const need = (kind) => (kind === "text" ? 4.5 : kind === "large" ? 3 : kind === "ui" ? 3 : 0);

for (const [label, t] of [["light", resolve(light)], ["dark", resolve(dark)]]) {
  const r = (a, b) => ratio(a, b);
  const pairs = [
    ["--fg on --bg", r(t.fg, t.bg), "text"],
    ["--fg on --bg-2 (controls, buttons)", r(t.fg, t.bg2), "text"],
    ["--fg on --bg-3 (code, hover button)", r(t.fg, t.bg3), "text"],
    ["--muted on --bg (labels, hints, nav)", r(t.muted, t.bg), "text"],
    ["--muted on --bg-2 (kbd, hover)", r(t.muted, t.bg2), "text"],
    ["--muted on --bg-3", r(t.muted, t.bg3), "text"],
    ["--muted on --accent-tint (hover transitions)", r(t.muted, t.accentTint), "text"],
    ["--faint on --bg (decoration)", r(t.faint, t.bg), "text"],
    ["--faint on --bg-2", r(t.faint, t.bg2), "text"],
    ["--line on --bg (hairlines, decorative)", r(t.line, t.bg), "decor"],
    ["--line-2 on --bg (status/kbd/modal border)", r(t.line2, t.bg), "ui"],
    ["--line-2 on --bg-2 (kbd border)", r(t.line2, t.bg2), "ui"],
    ["--control-line on --bg (link rest line, inputs)", r(t.controlLine, t.bg), "ui"],
    ["--control-line on --bg-2 (input border)", r(t.controlLine, t.bg2), "ui"],
    ["--accent on --bg (links hover, focus ring)", r(t.accent, t.bg), "text"],
    ["--accent on --bg-2", r(t.accent, t.bg2), "text"],
    ["--accent on --accent-tint (nav current, WIP pill, Today, pick slot hover)", r(t.accent, t.accentTint), "text"],
    ["--accent-tint on --bg (tint vs page: selected bg cue)", r(t.accentTint, t.bg), "decor"],
    ["--on-accent on --accent (primary button, skip link, brand mark)", r(t.onAccent, t.accent), "text"],
    ["--ok on --bg (done pill)", r(t.ok, t.bg), "text"],
    ["--ok 45% border on --bg (done pill border)", r(over(t.ok, 0.45, t.bg), t.bg), "ui"],
    ["--danger on --bg", r(t.danger, t.bg), "text"],
    ["--danger on error notice bg (danger 10% + bg)", r(t.danger, t.errorBg), "ui"],
    ["--fg on error notice bg", r(t.fg, t.errorBg), "text"],
    ["--accent 45% border on --accent-tint (WIP pill border)", r(over(t.accent, 0.45, t.accentTint), t.accentTint), "ui"],
    ["--muted on --bg, 11px uppercase .label/th", r(t.muted, t.bg), "text"],
    [".drop.over .empty-line (--muted on --accent-tint)", r(t.muted, t.accentTint), "text"],
    ["hour-label --muted on --bg", r(t.muted, t.bg), "text"],
    ["Urgent default #facc15 edge on --bg (card edge, 4px)", r(hex("#facc15"), t.bg), "ui"],
    ["Important default #3b82f6 edge on --bg", r(hex("#3b82f6"), t.bg), "ui"],
    ["bug chip #ef4444 with ink", null, "chip:#ef4444"],
    ["docs chip #10b981 with ink", null, "chip:#10b981"],
    ["research chip #8b5cf6 with ink", null, "chip:#8b5cf6"],
    ["Urgent chip #facc15 with ink", null, "chip:#facc15"],
    ["Important chip #3b82f6 with ink", null, "chip:#3b82f6"],
  ];
  for (const [what, v, kind] of pairs) {
    let val = v;
    if (kind.startsWith("chip:")) {
      const c = hex(kind.slice(5));
      const L = lum(c);
      const ink = (L + 0.05) / 0.05 >= 1.05 / (L + 0.05) ? [0, 0, 0] : [1, 1, 1]; // ui/ink.rs rule
      val = ratio(c, ink);
      const k = "text";
      const found = rows.find((x) => x.what === what);
      if (found) found[label] = val; else rows.push({ what: what + ` (ink ${toHex(ink)})`, kind: k, [label]: val });
      continue;
    }
    const found = rows.find((x) => x.what === what);
    if (found) found[label] = val; else rows.push({ what, kind, [label]: val });
  }
  console.log(`${label}: accent-tint = ${toHex(t.accentTint)}, error bg = ${toHex(t.errorBg)}`);
}

console.log("\n| Pair | Need | Light | Dark |\n|---|---|---|---|");
for (const x of rows) {
  const n = need(x.kind);
  const mark = (v) => (v == null ? "" : n && v < n ? `**${fmt(v)}**` : fmt(v));
  console.log(`| ${x.what} | ${n ? n + ":1" : "n/a"} | ${mark(x.light)} | ${mark(x.dark)} |`);
}
