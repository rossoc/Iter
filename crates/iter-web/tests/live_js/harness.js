// Runs ui/live.js under node against a small hand-made DOM, a fake EventSource, a fake fetch and
// a fake clock, and checks one scenario of how it follows the server (R2e-R2g of task 302).
//
//   node harness.js <path of live.js> <scenario>
//
// Prints "ok" and exits 0 when the scenario holds; otherwise prints why and exits 1. Driven by
// crates/iter-web/src/live_js_tests.rs, which skips when node is missing. No packages needed.
//
// The page is the same at every version but for its heading (`v<n>`), so a swap shows as the
// heading of <main> changing. The "server" answers a fetch of the page with the version it is
// at, and the DOMParser builds that page again.
"use strict";
const fs = require("fs");
const vm = require("vm");

// ---- clock -------------------------------------------------------------------------------------

const clock = { now: 0, seq: 0, timers: [] };
function setTimer(fn, ms, repeat) {
  const id = ++clock.seq;
  const wait = Math.max(0, Number(ms) || 0);
  clock.timers.push({ id, at: clock.now + wait, fn, every: repeat ? Math.max(1, wait) : 0 });
  return id;
}
function clearTimer(id) {
  clock.timers = clock.timers.filter((t) => t.id !== id);
}
async function flush() {
  for (let i = 0; i < 30; i++) await new Promise((r) => setImmediate(r));
}
// Runs the timers due within `ms`, in order, letting promises settle between them.
async function advance(ms) {
  const end = clock.now + ms;
  for (;;) {
    await flush();
    const due = clock.timers
      .filter((t) => t.at <= end)
      .sort((a, b) => a.at - b.at || a.id - b.id)[0];
    if (!due) break;
    clock.now = due.at;
    if (due.every) due.at += due.every;
    else clearTimer(due.id);
    due.fn();
  }
  clock.now = end;
  await flush();
}

// ---- selectors ---------------------------------------------------------------------------------

// Splits on `sep` outside brackets, parentheses and quotes.
function splitTop(text, isSep) {
  const parts = [];
  let depth = 0, quote = null, cur = "";
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quote) {
      if (c === quote) quote = null;
      cur += c;
    } else if (c === '"' || c === "'") {
      quote = c;
      cur += c;
    } else if (c === "[" || c === "(") {
      depth++;
      cur += c;
    } else if (c === "]" || c === ")") {
      depth--;
      cur += c;
    } else if (depth === 0 && isSep(c)) {
      parts.push(cur);
      parts.push(c);
      cur = "";
    } else cur += c;
  }
  parts.push(cur);
  return parts;
}

// A compound selector (`a.b#c[d=e]:not(...)`) as a test of one element.
function compound(text) {
  const tests = [];
  let rest = text;
  while (rest.length) {
    let m;
    if ((m = /^\*/.exec(rest))) {
      // any
    } else if ((m = /^[a-zA-Z][\w-]*/.exec(rest))) {
      const tag = m[0].toUpperCase();
      tests.push((el) => el.tagName === tag);
    } else if ((m = /^#([\w-]+)/.exec(rest))) {
      const id = m[1];
      tests.push((el) => el.id === id);
    } else if ((m = /^\.([\w-]+)/.exec(rest))) {
      const cls = m[1];
      tests.push((el) => el.classList.contains(cls));
    } else if ((m = /^\[\s*([\w-]+)\s*(?:([~^$*|]?=)\s*(?:"([^"]*)"|'([^']*)'|([^\]\s]*))\s*)?\]/.exec(rest))) {
      const name = m[1], op = m[2], value = m[3] ?? m[4] ?? m[5];
      tests.push((el) => {
        if (!el.hasAttribute(name)) return false;
        if (!op) return true;
        const have = el.getAttribute(name);
        if (op === "=") return have === value;
        if (op === "^=") return have.startsWith(value);
        if (op === "$=") return have.endsWith(value);
        if (op === "*=") return have.includes(value);
        if (op === "~=") return have.split(/\s+/).includes(value);
        if (op === "|=") return have === value || have.startsWith(value + "-");
        return false;
      });
    } else if ((m = /^:not\(/.exec(rest))) {
      let depth = 1, i = m[0].length;
      for (; i < rest.length && depth; i++) {
        if (rest[i] === "(") depth++;
        if (rest[i] === ")") depth--;
      }
      const inner = selector(rest.slice(m[0].length, i - 1));
      tests.push((el) => !inner(el));
      m = [rest.slice(0, i)];
    } else if ((m = /^:(checked|disabled|focus)/.exec(rest))) {
      const what = m[1];
      tests.push((el) =>
        what === "focus" ? el.ownerDocument.activeElement === el : !!el[what] || el.hasAttribute(what));
    } else {
      throw new Error("the DOM stub does not know the selector " + JSON.stringify(text));
    }
    rest = rest.slice(m[0].length);
  }
  return (el) => tests.every((t) => t(el));
}

// A selector list as a test of one element.
function selector(text) {
  const alternatives = splitTop(text, (c) => c === ",").filter((p) => p !== ",");
  const tests = alternatives.map((alt) => {
    // [compound, combinator, compound, ...]
    const raw = splitTop(alt.trim().replace(/\s*>\s*/g, ">"), (c) => c === " " || c === ">");
    const steps = [];
    for (let i = 0; i < raw.length; i += 2) {
      steps.push({ test: compound(raw[i]), comb: raw[i + 1] });
    }
    return (el) => matchFrom(steps, steps.length - 1, el);
  });
  return (el) => tests.some((t) => t(el));
}
function matchFrom(steps, i, el) {
  if (!steps[i].test(el)) return false;
  if (i === 0) return true;
  const comb = steps[i - 1].comb;
  for (let p = el.parentNode; p && p.nodeType === 1; p = p.parentNode) {
    if (matchFrom(steps, i - 1, p)) return true;
    if (comb === ">") return false;
  }
  return false;
}

// ---- nodes -------------------------------------------------------------------------------------

const observers = [];
function mutated() {
  observers.forEach((o) => {
    if (!o.on || o.queued) return;
    o.queued = true;
    queueMicrotask(() => {
      o.queued = false;
      if (o.on) o.fn([], o);
    });
  });
}

class EventStub {
  constructor(type, init) {
    init = init || {};
    this.type = type;
    this.bubbles = !!init.bubbles;
    this.cancelable = !!init.cancelable;
    this.detail = init.detail;
    this.relatedTarget = init.relatedTarget || null;
    this.submitter = init.submitter || null;
    this.key = init.key;
    this.data = init.data;
    this.defaultPrevented = false;
    this.target = null;
    this.currentTarget = null;
    this.stopped = false;
  }
  preventDefault() {
    if (this.cancelable) this.defaultPrevented = true;
  }
  stopPropagation() {
    this.stopped = true;
  }
  stopImmediatePropagation() {
    this.stopped = true;
  }
}
class CustomEventStub extends EventStub {}

class Listening {
  constructor() {
    this.listeners = [];
  }
  addEventListener(type, fn, opts) {
    if (!fn) return;
    const capture = opts === true || !!(opts && opts.capture);
    if (this.listeners.some((l) => l.type === type && l.fn === fn && l.capture === capture)) return;
    this.listeners.push({ type, fn, capture, once: !!(opts && opts.once) });
  }
  removeEventListener(type, fn, opts) {
    const capture = opts === true || !!(opts && opts.capture);
    this.listeners = this.listeners.filter((l) => !(l.type === type && l.fn === fn && l.capture === capture));
  }
  fire(ev, phase) {
    ev.currentTarget = this;
    for (const l of this.listeners.slice()) {
      if (l.type !== ev.type) continue;
      if (phase === "capture" && !l.capture) continue;
      if (phase === "bubble" && l.capture) continue;
      if (l.once) this.removeEventListener(l.type, l.fn, l.capture);
      typeof l.fn === "function" ? l.fn.call(this, ev) : l.fn.handleEvent(ev);
    }
    const prop = this["on" + ev.type];
    if (phase !== "capture" && typeof prop === "function" && (phase === "target" || ev.bubbles)) prop.call(this, ev);
  }
  // An object outside the page (the window, a stream): only its own listeners hear it.
  dispatchEvent(ev) {
    ev.target = this;
    this.fire(ev, "target");
    return !ev.defaultPrevented;
  }
}

class NodeStub extends Listening {
  constructor(doc) {
    super();
    this.ownerDocument = doc;
    this.parentNode = null;
    this.childNodes = [];
  }
  get children() {
    return this.childNodes.filter((n) => n.nodeType === 1);
  }
  get firstChild() {
    return this.childNodes[0] || null;
  }
  get firstElementChild() {
    return this.children[0] || null;
  }
  get parentElement() {
    return this.parentNode && this.parentNode.nodeType === 1 ? this.parentNode : null;
  }
  adopt(n) {
    if (n.nodeType === 11) return n.childNodes.slice().map((c) => this.adopt(c)).flat();
    if (n.parentNode) n.parentNode.detach(n);
    n.parentNode = this;
    return [n];
  }
  detach(n) {
    this.childNodes = this.childNodes.filter((c) => c !== n);
    n.parentNode = null;
  }
  appendChild(n) {
    this.childNodes.push(...this.adopt(n));
    mutated();
    return n;
  }
  append(...ns) {
    ns.forEach((n) => this.appendChild(typeof n === "string" ? new TextStub(this.ownerDocument, n) : n));
  }
  insertBefore(n, ref) {
    const added = this.adopt(n);
    const at = ref ? this.childNodes.indexOf(ref) : -1;
    if (at < 0) this.childNodes.push(...added);
    else this.childNodes.splice(at, 0, ...added);
    mutated();
    return n;
  }
  removeChild(n) {
    this.detach(n);
    mutated();
    return n;
  }
  remove() {
    if (this.parentNode) this.parentNode.removeChild(this);
  }
  replaceChildren(...ns) {
    this.childNodes.slice().forEach((c) => this.detach(c));
    ns.forEach((n) => this.childNodes.push(...this.adopt(typeof n === "string" ? new TextStub(this.ownerDocument, n) : n)));
    mutated();
  }
  replaceWith(...ns) {
    const parent = this.parentNode;
    if (!parent) return;
    const at = parent.childNodes.indexOf(this);
    parent.detach(this);
    const added = ns.map((n) => parent.adopt(n)).flat();
    parent.childNodes.splice(at, 0, ...added);
    mutated();
  }
  replaceChild(n, old) {
    old.replaceWith(n);
    return old;
  }
  contains(n) {
    for (let x = n; x; x = x.parentNode) if (x === this) return true;
    return false;
  }
  get isConnected() {
    let x = this;
    while (x.parentNode) x = x.parentNode;
    return x === this.ownerDocument;
  }
  get textContent() {
    return this.childNodes.map((c) => c.textContent).join("");
  }
  set textContent(v) {
    this.replaceChildren(new TextStub(this.ownerDocument, String(v)));
  }
  descendants() {
    const out = [];
    const walk = (n) =>
      n.childNodes.forEach((c) => {
        if (c.nodeType === 1) {
          out.push(c);
          walk(c);
        }
      });
    walk(this);
    return out;
  }
  querySelectorAll(sel) {
    const test = selector(sel);
    return this.descendants().filter(test);
  }
  querySelector(sel) {
    return this.querySelectorAll(sel)[0] || null;
  }
  getElementsByTagName(tag) {
    return this.querySelectorAll(tag);
  }
  // Capture from the top, then the target, then bubbling; the window last.
  dispatchEvent(ev) {
    ev.target = this;
    const path = [];
    for (let x = this.parentNode; x; x = x.parentNode) path.push(x);
    const top = path.length ? path[path.length - 1] : this;
    if (top === world.document) path.push(world.window);
    for (let i = path.length - 1; i >= 0 && !ev.stopped; i--) path[i].fire(ev, "capture");
    if (!ev.stopped) this.fire(ev, "target");
    if (ev.bubbles) for (let i = 0; i < path.length && !ev.stopped; i++) path[i].fire(ev, "bubble");
    return !ev.defaultPrevented;
  }
}

class TextStub extends NodeStub {
  constructor(doc, text) {
    super(doc);
    this.nodeType = 3;
    this.data = text;
  }
  get textContent() {
    return this.data;
  }
  set textContent(v) {
    this.data = String(v);
  }
  cloneNode() {
    return new TextStub(this.ownerDocument, this.data);
  }
}

const FIELDS = ["INPUT", "TEXTAREA", "SELECT", "BUTTON"];

class ElementStub extends NodeStub {
  constructor(doc, tag) {
    super(doc);
    this.nodeType = 1;
    this.tagName = tag.toUpperCase();
    this.nodeName = this.tagName;
    this.attrs = new Map();
    this._value = null;
    const el = this;
    this.dataset = new Proxy(
      {},
      {
        get: (_, k) => (typeof k === "string" ? el.getAttribute(dataName(k)) ?? undefined : undefined),
        set: (_, k, v) => (el.setAttribute(dataName(k), String(v)), true),
        deleteProperty: (_, k) => (el.removeAttribute(dataName(k)), true),
        has: (_, k) => el.hasAttribute(dataName(k)),
      }
    );
    this.style = {};
  }
  getAttribute(n) {
    return this.attrs.has(n) ? this.attrs.get(n) : null;
  }
  setAttribute(n, v) {
    this.attrs.set(n, String(v));
    mutated();
  }
  hasAttribute(n) {
    return this.attrs.has(n);
  }
  removeAttribute(n) {
    this.attrs.delete(n);
    mutated();
  }
  toggleAttribute(n, force) {
    const on = force === undefined ? !this.hasAttribute(n) : !!force;
    if (on) this.setAttribute(n, "");
    else this.removeAttribute(n);
    return on;
  }
  get id() {
    return this.getAttribute("id") || "";
  }
  set id(v) {
    this.setAttribute("id", v);
  }
  get className() {
    return this.getAttribute("class") || "";
  }
  set className(v) {
    this.setAttribute("class", v);
  }
  get classList() {
    const el = this;
    const list = () => el.className.split(/\s+/).filter(Boolean);
    return {
      contains: (c) => list().includes(c),
      add: (...cs) => (el.className = [...new Set([...list(), ...cs])].join(" ")),
      remove: (...cs) => (el.className = list().filter((c) => !cs.includes(c)).join(" ")),
      toggle: (c, force) => {
        const on = force === undefined ? !list().includes(c) : !!force;
        if (on) el.classList.add(c);
        else el.classList.remove(c);
        return on;
      },
    };
  }
  get hidden() {
    return this.hasAttribute("hidden");
  }
  set hidden(v) {
    this.toggleAttribute("hidden", !!v);
  }
  get disabled() {
    return this.hasAttribute("disabled");
  }
  set disabled(v) {
    this.toggleAttribute("disabled", !!v);
  }
  get name() {
    return this.getAttribute("name") || "";
  }
  get type() {
    return this.getAttribute("type") || (this.tagName === "INPUT" ? "text" : this.tagName === "BUTTON" ? "submit" : "");
  }
  get href() {
    const h = this.getAttribute("href");
    return h === null ? "" : new URL(h, world.location.href).href;
  }
  get value() {
    return this._value !== null ? this._value : this.getAttribute("value") || "";
  }
  set value(v) {
    this._value = String(v);
  }
  get tabIndex() {
    const t = this.getAttribute("tabindex");
    return t === null ? (["A", "BUTTON", "INPUT", "TEXTAREA", "SELECT"].includes(this.tagName) ? 0 : -1) : +t;
  }
  set tabIndex(v) {
    this.setAttribute("tabindex", String(v));
  }
  get isContentEditable() {
    const v = this.getAttribute("contenteditable");
    return v === "" || v === "true";
  }
  get form() {
    return FIELDS.includes(this.tagName) ? this.closest("form") : null;
  }
  get elements() {
    return this.querySelectorAll("input, textarea, select, button");
  }
  matches(sel) {
    return selector(sel)(this);
  }
  closest(sel) {
    const test = selector(sel);
    for (let x = this; x && x.nodeType === 1; x = x.parentNode) if (test(x)) return x;
    return null;
  }
  getElementById(id) {
    return this.descendants().find((e) => e.id === id) || null;
  }
  focus() {
    moveFocus(this);
  }
  blur() {
    if (this.ownerDocument.activeElement === this) moveFocus(this.ownerDocument.body);
  }
  reset() {
    this.dispatchEvent(new EventStub("reset", { bubbles: true, cancelable: true }));
  }
  requestSubmit(submitter) {
    this.dispatchEvent(new EventStub("submit", { bubbles: true, cancelable: true, submitter }));
  }
  getBoundingClientRect() {
    return { top: 0, left: 0, right: 0, bottom: 0, width: 0, height: 0 };
  }
  scrollIntoView() {}
  cloneNode(deep) {
    const c = new ElementStub(this.ownerDocument, this.tagName);
    this.attrs.forEach((v, k) => c.attrs.set(k, v));
    if (deep) this.childNodes.forEach((n) => c.appendChild(n.cloneNode(true)));
    return c;
  }
}

function dataName(key) {
  return "data-" + key.replace(/[A-Z]/g, (c) => "-" + c.toLowerCase());
}

class DocumentStub extends NodeStub {
  constructor() {
    super(null);
    this.ownerDocument = this;
    this.nodeType = 9;
    this.hidden = false;
    this.readyState = "complete";
    this._active = null;
  }
  get visibilityState() {
    return this.hidden ? "hidden" : "visible";
  }
  get documentElement() {
    return this.children[0] || null;
  }
  get head() {
    return this.querySelector("head");
  }
  get body() {
    return this.querySelector("body");
  }
  get title() {
    const t = this.querySelector("title");
    return t ? t.textContent : "";
  }
  set title(v) {
    const t = this.querySelector("title");
    if (t) t.textContent = v;
  }
  get activeElement() {
    return this._active && this._active.isConnected ? this._active : this.body;
  }
  createElement(tag) {
    return new ElementStub(this, tag);
  }
  createTextNode(text) {
    return new TextStub(this, text);
  }
  createDocumentFragment() {
    const f = new NodeStub(this);
    f.nodeType = 11;
    return f;
  }
  importNode(n, deep) {
    return reown(n.cloneNode(deep), this);
  }
  adoptNode(n) {
    if (n.parentNode) n.parentNode.detach(n);
    return reown(n, this);
  }
  getElementById(id) {
    return this.descendants().find((e) => e.id === id) || null;
  }
  hasFocus() {
    return true;
  }
}

function reown(n, doc) {
  n.ownerDocument = doc;
  n.childNodes.forEach((c) => reown(c, doc));
  return n;
}

function moveFocus(to) {
  const doc = to.ownerDocument;
  const from = doc.activeElement;
  if (from === to) return;
  if (from && from !== doc.body) {
    from.dispatchEvent(new EventStub("blur", { relatedTarget: to }));
    from.dispatchEvent(new EventStub("focusout", { bubbles: true, relatedTarget: to }));
  }
  doc._active = to;
  if (to !== doc.body) {
    to.dispatchEvent(new EventStub("focus", { relatedTarget: from }));
    to.dispatchEvent(new EventStub("focusin", { bubbles: true, relatedTarget: from }));
  }
}

// ---- the page and the server ---------------------------------------------------------------------

// h("tag#id.class", {attr: value}, ...children)
function h(doc, spec, attrs, ...kids) {
  const m = /^([a-z0-9]+)(?:#([\w-]+))?((?:\.[\w-]+)*)$/.exec(spec);
  const el = doc.createElement(m[1]);
  if (m[2]) el.setAttribute("id", m[2]);
  if (m[3]) el.setAttribute("class", m[3].slice(1).replace(/\./g, " "));
  Object.entries(attrs || {}).forEach(([k, v]) => el.setAttribute(k, v));
  kids.forEach((k) => el.appendChild(typeof k === "string" ? doc.createTextNode(k) : k));
  return el;
}

// The page at version `n`. The link `#gone` is there only at version 1.
function page(doc, n) {
  const e = (...a) => h(doc, ...a);
  const main = e(
    "main#main",
    { "data-live": "", tabindex: "-1" },
    e("h1", null, "v" + n),
    e(
      "form#f",
      { method: "post", action: "/save" },
      e("input#q", { name: "q", type: "text" }),
      e("button", { name: "act", value: "go", type: "submit" }, "Act")
    ),
    e("a#go", { href: "/x" }, "Go"),
    e("a", { href: "/y" }, "Y"),
    ...(String(n) === "1" ? [e("a#gone", { href: "/gone" }, "Gone")] : [])
  );
  const html = e(
    "html",
    null,
    e("head", null, e("title", null, "Page v" + n)),
    e(
      "body",
      null,
      e("header.bar", null, e("a#home", { href: "/" }, "Home")),
      main,
      e("div#toasts.toasts", { "aria-live": "polite" })
    )
  );
  doc.appendChild(html);
  return doc;
}

const world = {};

function build() {
  const document = new DocumentStub();
  page(document, "1");
  const window = new Listening();
  world.window = window;
  world.document = document;
  world.location = { href: "http://127.0.0.1:3000/task/1", origin: "http://127.0.0.1:3000", pathname: "/task/1" };
  world.server = { version: "1" };
  world.fetches = [];
  world.sources = [];
  world.toasts = [];

  class EventSourceStub extends Listening {
    constructor(url) {
      super();
      this.url = url;
      this.readyState = 0;
      this.closed = false;
      this.onmessage = null;
      this.onerror = null;
      this.onopen = null;
      world.sources.push(this);
    }
    close() {
      this.readyState = 2;
      this.closed = true;
    }
  }
  EventSourceStub.CONNECTING = 0;
  EventSourceStub.OPEN = 1;
  EventSourceStub.CLOSED = 2;

  class DOMParserStub {
    parseFromString(text) {
      const m = /^page:(.*)$/.exec(text);
      if (!m) throw new Error("the stub server only serves pages: " + text);
      return page(new DocumentStub(), m[1]);
    }
  }

  function fetch(url, init) {
    world.fetches.push({ url: String(url), init });
    const version = world.server.version;
    return Promise.resolve({
      ok: true,
      status: 200,
      redirected: false,
      url: String(url),
      headers: { get: () => "text/html" },
      text: () => Promise.resolve("page:" + version),
    });
  }

  class MutationObserverStub {
    constructor(fn) {
      this.fn = fn;
      this.on = false;
      observers.push(this);
    }
    observe() {
      this.on = true;
    }
    disconnect() {
      this.on = false;
    }
    takeRecords() {
      return [];
    }
  }

  Object.assign(window, {
    window,
    self: window,
    document,
    location: world.location,
    navigator: { userAgent: "node" },
    EventSource: EventSourceStub,
    DOMParser: DOMParserStub,
    MutationObserver: MutationObserverStub,
    fetch,
    Event: EventStub,
    CustomEvent: CustomEventStub,
    FocusEvent: EventStub,
    KeyboardEvent: EventStub,
    CSS: { escape: (s) => String(s).replace(/["\\#.:\[\]()]/g, "\\$&") },
    setTimeout: (fn, ms) => setTimer(fn, ms, false),
    setInterval: (fn, ms) => setTimer(fn, ms, true),
    clearTimeout: clearTimer,
    clearInterval: clearTimer,
    requestAnimationFrame: (fn) => setTimer(() => fn(clock.now), 16, false),
    cancelAnimationFrame: clearTimer,
    queueMicrotask,
    console,
    iterToast: (kind, text) => world.toasts.push({ kind, text }),
  });
  window.top = window;
  window.parent = window;
  // the page's own toast region (ui/toast.js) as well, for a script that writes there
  return window;
}

// ---- driving it ----------------------------------------------------------------------------------

function source() {
  const open = world.sources.filter((s) => !s.closed);
  if (open.length !== 1) throw new Error("expected one open stream, found " + open.length + " of " + world.sources.length);
  return open[0];
}

function emit(src, type, data) {
  if (type === "message" && src.readyState !== 1) {
    src.readyState = 1;
    src.dispatchEvent(new EventStub("open"));
  }
  src.dispatchEvent(new EventStub(type, { data }));
}

// The server moves to `version` and says so on the open stream.
async function serverAt(version) {
  world.server.version = String(version);
  emit(source(), "message", String(version));
  await advance(50);
}

function heading() {
  const main = world.document.getElementById("main");
  const h1 = main && main.querySelector("h1");
  return h1 ? h1.textContent : "(no heading)";
}

function check(ok, why) {
  if (!ok) throw new Error(why);
}

function at(version, why) {
  check(heading() === "v" + version, why + " (the page shows " + heading() + ", the server is at v" + world.server.version + ")");
}

const $ = (sel) => world.document.querySelector(sel);

function typeInto(field) {
  field.focus();
  field.value = "draft";
  field.dispatchEvent(new EventStub("input", { bubbles: true }));
}

// ---- scenarios -----------------------------------------------------------------------------------

const scenarios = {
  // Guard: with nothing held, a new version is swapped in.
  async the_page_follows_a_new_version() {
    await serverAt(1);
    at(1, "the first version is only remembered");
    await serverAt(2);
    at(2, "a new version is swapped in");
    check(world.toasts.length === 0, "no toast when the swap went through: " + JSON.stringify(world.toasts));
  },

  // R2e: a swap held while a field had focus is applied once focus leaves it.
  async held_by_a_field_applies_when_focus_leaves() {
    $("#q").focus();
    await serverAt(1);
    await serverAt(2);
    at(1, "the swap waits while a field in the page has focus");
    world.document.body.focus();
    await advance(1500);
    at(2, "the held swap is applied on its own once focus left the field, without another change");
  },

  // R2e: a swap held for an edited form is applied once the form is reset.
  async held_by_an_edited_form_applies_on_reset() {
    typeInto($("#q"));
    world.document.body.focus();
    await serverAt(1);
    await serverAt(2);
    at(1, "the swap waits while a form in the page has been edited");
    $("#f").reset();
    await advance(1500);
    at(2, "the held swap is applied on its own once the edited form was reset");
  },

  // R2e: a swap held for an edited form is applied once the form is submitted (and the page
  // stays, the submit handled by a script).
  async held_by_an_edited_form_applies_on_submit() {
    typeInto($("#q"));
    world.document.body.focus();
    $("#f").addEventListener("submit", (e) => e.preventDefault());
    await serverAt(1);
    await serverAt(2);
    at(1, "the swap waits while a form in the page has been edited");
    $("#f").requestSubmit();
    await advance(1500);
    at(2, "the held swap is applied on its own once the edited form was submitted");
  },

  // R2e: a swap held while a pop-up was open is applied once it closes.
  async held_by_a_pop_up_applies_when_it_closes() {
    const modal = h(world.document, "div.modal-box", { role: "dialog" }, "Pop-up");
    world.document.body.appendChild(modal);
    await serverAt(1);
    await serverAt(2);
    at(1, "the swap waits while a pop-up is open");
    modal.remove();
    world.document.dispatchEvent(new CustomEventStub("iter:modal-closed"));
    await advance(1500);
    at(2, "the held swap is applied on its own once the pop-up closed");
  },

  // R2e: the toast for a held swap says the page will update, not to reload it.
  async the_toast_does_not_ask_for_a_reload() {
    $("#q").focus();
    await serverAt(1);
    await serverAt(2);
    at(1, "the swap waits while a field has focus");
    check(world.toasts.length > 0, "a toast says the page changed");
    const asking = world.toasts.filter((t) => /reload/i.test(t.text));
    check(asking.length === 0, "the toast does not ask for a reload: " + JSON.stringify(world.toasts));
  },

  // R2f: focus on a link does not hold the swap, and comes back to the same link (by id).
  async focus_on_a_link_does_not_hold_and_comes_back_by_id() {
    const old = $("#go");
    old.focus();
    await serverAt(1);
    await serverAt(2);
    at(2, "focus on a link (not a field) does not hold the swap");
    const now = world.document.activeElement;
    check(now !== old && now.isConnected, "focus is on an element of the fresh page, not the old link");
    check(now.id === "go", "focus came back to the link with the same id, not " + describe(now));
  },

  // R2f: without an id, focus comes back to the link with the same href.
  async focus_comes_back_by_href() {
    const old = world.document.querySelector('a[href="/y"]');
    old.focus();
    await serverAt(1);
    await serverAt(2);
    at(2, "focus on a link does not hold the swap");
    const now = world.document.activeElement;
    check(now !== old && now.isConnected, "focus is on an element of the fresh page");
    check(now.getAttribute("href") === "/y", "focus came back to the link with the same href, not " + describe(now));
  },

  // R2f: a button without an id gets focus back by its name and value.
  async focus_comes_back_by_name_and_value() {
    const old = world.document.querySelector("button[name=act]");
    old.focus();
    await serverAt(1);
    await serverAt(2);
    at(2, "focus on a button does not hold the swap");
    const now = world.document.activeElement;
    check(now !== old && now.isConnected, "focus is on an element of the fresh page");
    check(now.tagName === "BUTTON" && now.name === "act" && now.value === "go", "focus came back to the same button, not " + describe(now));
  },

  // R2f: when the focused element is gone from the fresh page, focus goes to the region.
  async focus_falls_back_to_the_region() {
    $("#gone").focus();
    await serverAt(1);
    await serverAt(2);
    at(2, "focus on a link does not hold the swap");
    const now = world.document.activeElement;
    check(now === world.document.getElementById("main"), "focus went to the region, not " + describe(now));
  },

  // R2g: a stream that closed (the server went away) is opened again, and a version that
  // differs from the last one seen brings the page up to date; the same version leaves it.
  async a_dropped_stream_reconnects_and_catches_up() {
    await serverAt(1);
    const first = source();
    first.readyState = 2;
    emit(first, "error");
    await advance(60000);
    const open = world.sources.filter((s) => !s.closed && s !== first && s.readyState !== 2);
    check(open.length === 1, "one new stream is opened after the old one closed (found " + open.length + ")");
    const fetched = world.fetches.length;
    emit(open[0], "message", "1");
    await advance(50);
    check(world.fetches.length === fetched, "the same version after reconnecting does not refresh");
    at(1, "the page stays at the version it had");
    world.server.version = "2";
    emit(open[0], "message", "2");
    await advance(50);
    at(2, "a version that moved while the stream was down is caught up");
  },

  // R2g: a stream the browser is reconnecting itself (readyState CONNECTING) is left to it, and
  // the next version still refreshes.
  async a_reconnecting_stream_catches_up() {
    await serverAt(1);
    const first = source();
    first.readyState = 0;
    emit(first, "error");
    await advance(60000);
    world.server.version = "2";
    const open = world.sources.filter((s) => !s.closed);
    check(open.length === 1, "one stream is open (found " + open.length + ")");
    emit(open[0], "message", "2");
    await advance(50);
    at(2, "the version after the reconnect is caught up");
  },

  // R2g: a hidden tab closes its stream and catches up when shown again.
  async a_hidden_tab_closes_its_stream_and_catches_up_when_shown() {
    await serverAt(1);
    const first = source();
    world.document.hidden = true;
    world.document.dispatchEvent(new EventStub("visibilitychange"));
    await advance(50);
    check(first.closed, "the hidden tab closed its stream");
    check(world.sources.every((s) => s.closed), "no stream is open while hidden");
    world.server.version = "2";
    world.document.hidden = false;
    world.document.dispatchEvent(new EventStub("visibilitychange"));
    await advance(50);
    emit(source(), "message", "2");
    await advance(50);
    at(2, "the tab caught up once shown");
  },
};

function describe(el) {
  if (!el) return "nothing";
  return "<" + el.tagName.toLowerCase() + (el.id ? "#" + el.id : "") + (el.getAttribute && el.getAttribute("href") ? " href=" + el.getAttribute("href") : "") + ">";
}

async function main() {
  const [script, name] = process.argv.slice(2);
  if (name === "--list") {
    console.log(Object.keys(scenarios).join("\n"));
    return;
  }
  const scenario = scenarios[name];
  if (!script || !scenario) {
    console.error("usage: node harness.js <live.js> <scenario>; scenarios: " + Object.keys(scenarios).join(", "));
    process.exit(2);
  }
  const window = build();
  const context = vm.createContext(window);
  vm.runInContext(fs.readFileSync(script, "utf8"), context, { filename: script });
  await advance(10);
  await scenario();
  console.log("ok");
}

main().catch((e) => {
  console.log("FAILED: " + (e && e.message ? e.message : e));
  if (e && e.stack && !/^Error: /.test(String(e.message))) console.log(e.stack);
  process.exit(1);
});
