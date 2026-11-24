// The page that updates itself (frame.rs, pages/site/live.rs). The server's `/live` event
// stream sends one number, the live version, at once and again whenever the database or a notes
// file changes. When it moves, this script fetches the page again and swaps each region marked
// `data-live` (by its id; `<main id="main">` is one) for the fresh one, so the page follows the
// data without a reload. The server stays the one place a page is rendered: no client model.
//
// It does not swap a region the user is busy in - a field in it has focus, a form in it has
// been edited, or a pop-up is open over the page - since that would throw their work away;
// it raises a "page changed" toast instead, and the next change (or a reload) brings it up to
// date. A hidden tab closes its stream (browsers allow few connections per server) and opens
// it again when shown, catching up if the version moved meanwhile. A framed page (the
// palette's preview) does not listen at all.
(function () {
  if (!window.EventSource || window.top !== window) return;
  var REGIONS = "[data-live]";
  var source = null;
  var known = null;
  var busy = false;
  var again = false;
  var told = false;

  // A form counts as edited from its first input until the page is swapped or reloaded.
  document.addEventListener("input", function (e) {
    var form = e.target && e.target.form;
    if (form) form.dataset.edited = "";
  });

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
      // any focused element (a field, or a link or button a keyboard user is on) would be
      // lost: the swap waits; the region itself (the skip link's target) does not count
      if (focus && focus !== here[i] && here[i].contains(focus)) {
        return typing(focus) ? "a field has focus" : "something in the page has focus";
      }
      if (here[i].querySelector("form[data-edited]")) return "a form was edited";
    }
    return null;
  }

  function tell() {
    if (told || !window.iterToast) return;
    told = true;
    window.iterToast("ok", "This page changed. Reload to see the latest.");
  }

  function swap(html) {
    var fresh = new DOMParser().parseFromString(html, "text/html");
    var title = fresh.querySelector("title");
    if (title) document.title = title.textContent;
    regions(document).forEach(function (old) {
      var next = fresh.getElementById(old.id);
      if (!next || !next.hasAttribute("data-live")) return;
      old.replaceChildren.apply(old, Array.prototype.slice.call(next.childNodes).map(function (n) {
        return document.importNode(n, true);
      }));
    });
    told = false;
    document.dispatchEvent(new CustomEvent("iter:live"));
  }

  function refresh() {
    if (held()) return tell();
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
        if (held()) return tell();
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
    source = new EventSource("/live");
    source.onmessage = function (e) {
      var version = e.data;
      if (known !== null && version !== known) refresh();
      known = version;
    };
  }

  function close() {
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
