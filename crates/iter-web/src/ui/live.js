// The page that updates itself (frame.rs, pages/site/live.rs). The server's `/live` event
// stream sends one number, the live version, at once and again whenever the database or a notes
// file changes. When it moves, this script fetches the page again and swaps each region marked
// `data-live` (by its id; `<main id="main">` is one) for the fresh one, so the page follows the
// data without a reload. The server stays the one place a page is rendered: no client model.
//
// It holds the swap while the user is busy - a field in a region has focus, a form in it has
// been edited, or a pop-up is open over the page - since that would throw their work away; a
// toast says the page will update, and the held swap is applied on its own as soon as the hold
// ends (focus leaves the field, the form is submitted or reset, the pop-up closes; checked on
// those events and every second). Focus on a link or button does not hold: after the swap it goes
// back to the matching element of the fresh region (same id, else href, else name and value), or
// to the region. A stream that closes (the server went away) is opened again after a backoff, and
// a version that differs from the last one seen catches the page up. A hidden tab closes its
// stream (browsers allow few connections per server) and opens it again when shown. A framed
// page (the palette's preview) does not listen at all.
(function () {
  if (!window.EventSource || window.top !== window) return;
  var REGIONS = "[data-live]";
  var source = null;
  var known = null;
  var busy = false;
  var again = false;
  var told = false;
  var pending = false;
  var retry = null;
  var backoff = 1000;
  var reopen = null;

  // A form counts as edited from its first input until it is submitted or reset, or the page is
  // swapped or reloaded.
  document.addEventListener("input", function (e) {
    var form = e.target && e.target.form;
    if (form) form.dataset.edited = "";
  });
  ["submit", "reset"].forEach(function (type) {
    document.addEventListener(type, function (e) {
      if (e.target && e.target.dataset) delete e.target.dataset.edited;
      soon();
    });
  });
  // focus has not moved yet when focusout fires: look once it has
  document.addEventListener("focusout", soon);

  function regions(doc) {
    return Array.prototype.filter.call(doc.querySelectorAll(REGIONS), function (el) {
      return el.id;
    });
  }

  function typing(el) {
    return el && (el.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName));
  }

  // Why the page cannot be swapped now, or null.
  function held() {
    // a pop-up (modal.rs) is in the page only while open; the search palette is always
    // there, hidden, and marks the document while open
    var palette = document.documentElement.classList.contains("palette-open");
    if (palette || document.querySelector(".modal-box[role=dialog], dialog[open], [role=listbox]:not([hidden]):not(.palette-list)")) {
      return "a pop-up is open";
    }
    if (document.querySelector(".dragging")) return "a card is being dragged";
    var here = regions(document);
    var focus = document.activeElement;
    for (var i = 0; i < here.length; i++) {
      // only a field holds: a link or button gets its focus back after the swap
      if (typing(focus) && here[i].contains(focus)) return "a field has focus";
      if (here[i].querySelector("form[data-edited]")) return "a form was edited";
    }
    return null;
  }

  function tell() {
    if (told || !window.iterToast) return;
    told = true;
    window.iterToast("ok", "This page changed. It will update when you are done here.");
  }

  // A held swap waits here; it is tried again on the events that end a hold and every second.
  function hold() {
    pending = true;
    tell();
    if (retry === null) retry = setInterval(tryHeld, 1000);
  }

  function release() {
    pending = false;
    if (retry !== null) clearInterval(retry);
    retry = null;
  }

  function tryHeld() {
    if (pending && !held()) {
      release();
      refresh();
    }
  }

  function soon() {
    if (pending) setTimeout(tryHeld, 0);
  }

  // Where focus is in a region, to find the same element in the fresh one.
  function focused() {
    var el = document.activeElement;
    var region = regions(document).filter(function (r) { return r !== el && r.contains(el); })[0];
    if (!region) return null;
    return {
      region: region.id,
      id: el.id,
      href: el.getAttribute("href"),
      tag: el.tagName,
      name: el.getAttribute("name"),
      value: el.getAttribute("value"),
    };
  }

  function refocus(was) {
    var region = document.getElementById(was.region);
    if (!region) return;
    var all = Array.prototype.slice.call(region.querySelectorAll("*"));
    var match = function (test) { return all.filter(test)[0]; };
    var next = (was.id && match(function (el) { return el.id === was.id; })) ||
      (was.href !== null && match(function (el) { return el.tagName === was.tag && el.getAttribute("href") === was.href; })) ||
      (was.name !== null && match(function (el) {
        return el.tagName === was.tag && el.getAttribute("name") === was.name && el.getAttribute("value") === was.value;
      })) ||
      region;
    next.focus({ preventScroll: true });
  }

  function swap(html) {
    var fresh = new DOMParser().parseFromString(html, "text/html");
    var title = fresh.querySelector("title");
    if (title) document.title = title.textContent;
    var was = focused();
    regions(document).forEach(function (old) {
      var next = fresh.getElementById(old.id);
      if (!next || !next.hasAttribute("data-live")) return;
      old.replaceChildren.apply(old, Array.prototype.slice.call(next.childNodes).map(function (n) {
        return document.importNode(n, true);
      }));
    });
    if (was) refocus(was);
    told = false;
    document.dispatchEvent(new CustomEvent("iter:live"));
  }

  function refresh() {
    if (held()) return hold();
    if (busy) {
      again = true;
      return;
    }
    busy = true;
    var asked = location.href;
    fetch(asked, { headers: { Accept: "text/html" }, credentials: "same-origin" })
      .then(function (r) {
        // a page that is gone now, or moved elsewhere, is left as it is
        if (!r.ok || r.redirected) return null;
        return r.text();
      })
      .then(function (html) {
        // the user may have started typing, or left, while it loaded
        if (html === null || location.href !== asked) return;
        if (held()) return hold();
        swap(html);
      })
      .catch(function () {})
      .then(function () {
        busy = false;
        if (again) {
          again = false;
          refresh();
        }
      });
  }

  function open() {
    if (source) return;
    clearTimeout(reopen);
    reopen = null;
    var here = new EventSource("/live");
    source = here;
    here.onmessage = function (e) {
      backoff = 1000;
      var version = e.data;
      if (known !== null && version !== known) refresh();
      known = version;
    };
    // a stream the browser is reconnecting (CONNECTING) is left to it; a closed one is opened
    // again after a backoff, up to 30s
    here.onerror = function () {
      if (here !== source || here.readyState !== 2) return;
      source = null;
      if (document.hidden || reopen !== null) return;
      reopen = setTimeout(function () {
        reopen = null;
        if (!document.hidden) open();
      }, backoff);
      backoff = Math.min(backoff * 2, 30000);
    };
  }

  function close() {
    clearTimeout(reopen);
    reopen = null;
    if (source) source.close();
    source = null;
  }

  document.addEventListener("visibilitychange", function () {
    if (document.hidden) close();
    else open();
  });
  window.addEventListener("pagehide", close);
  window.addEventListener("pageshow", function () {
    if (!document.hidden) open();
  });
  if (!document.hidden) open();
})();
