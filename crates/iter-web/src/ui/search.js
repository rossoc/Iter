// The search palette (search.rs): `/` or the bar's button opens it; the query goes to
// GET /search?q= and the selected page shows in the frame beside the list.
//   - Up/Down (or Ctrl-N/Ctrl-P) move, Enter opens (Shift/Ctrl/Cmd+Enter in a new tab), Esc closes;
//   - Tab completes a kind word (`pr` -> `project `), else moves into the interactive preview.
// Loaded first: a page that is itself the preview must hide its chrome before it paints (`.in-preview`).
(function () {
  if (window.top !== window.self) {
    document.documentElement.classList.add("in-preview");
    return;
  }

  var KINDS = ["board","org","project","task","note"];

  function ready() {
    var root = document.getElementById("palette");
    if (!root) return;
    var input = root.querySelector(".palette-input");
    var list = root.querySelector(".palette-list");
    var frame = root.querySelector(".palette-frame");
    var status = document.getElementById("palette-status");
    var chrome = document.querySelectorAll("header.bar, main#main, footer.site-foot");
    var opener = null, hits = [], current = -1, seq = 0, timer = 0, previewTimer = 0;

    document.querySelectorAll("[data-palette-open]").forEach(function (b) {
      b.hidden = false;
      b.addEventListener("click", open);
    });
    root.querySelector("[data-palette-close]").addEventListener("click", close);

    function editable(el) {
      return el && (el.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName));
    }

    function open() {
      if (!root.hidden) return;
      opener = document.activeElement;
      root.hidden = false;
      document.documentElement.classList.add("palette-open");
      chrome.forEach(function (el) { el.setAttribute("inert", ""); });
      input.focus();
      input.select();
      query();
    }

    function close() {
      if (root.hidden) return;
      root.hidden = true;
      clearTimeout(timer);
      clearTimeout(previewTimer);
      frame.src = "about:blank";
      document.documentElement.classList.remove("palette-open");
      chrome.forEach(function (el) { el.removeAttribute("inert"); });
      if (opener && opener.focus) opener.focus();
    }

    function query() {
      var n = ++seq;
      fetch("/search?q=" + encodeURIComponent(input.value))
        .then(function (r) { return r.json(); })
        .then(function (data) { if (n === seq) show(data); })
        .catch(function () { if (n === seq) { list.textContent = ""; message("Search failed."); } });
    }

    function message(text) {
      var li = document.createElement("li");
      li.className = "palette-none";
      li.textContent = text;
      list.appendChild(li);
      status.textContent = text;
    }

    function marked(title, marks) {
      var span = document.createElement("span");
      span.className = "palette-title";
      var set = {};
      marks.forEach(function (m) { set[m] = true; });
      var chars = Array.from(title), run = "", on = false;
      function flush() {
        if (!run) return;
        if (on) {
          var m = document.createElement("mark");
          m.textContent = run;
          span.appendChild(m);
        } else {
          span.appendChild(document.createTextNode(run));
        }
        run = "";
      }
      chars.forEach(function (c, i) {
        var hit = !!set[i];
        if (hit !== on) { flush(); on = hit; }
        run += c;
      });
      flush();
      return span;
    }

    function show(data) {
      hits = data;
      list.textContent = "";
      current = -1;
      if (!hits.length) {
        message("No pages match.");
        frame.src = "about:blank";
        return;
      }
      hits.forEach(function (h, i) {
        var li = document.createElement("li");
        li.id = "palette-opt-" + i;
        li.setAttribute("role", "option");
        var a = document.createElement("a");
        a.href = h.url;
        var kind = document.createElement("span");
        kind.className = "palette-kind";
        kind.title = h.kind;
        var tpl = root.querySelector('template[data-kind="' + h.kind + '"]');
        if (tpl) kind.appendChild(tpl.content.cloneNode(true));
        var label = document.createElement("span");
        label.className = "sr";
        label.textContent = h.kind + ": ";
        kind.appendChild(label);
        var sub = document.createElement("span");
        sub.className = "palette-sub";
        sub.textContent = [h.path, h.status].filter(Boolean).join(" · ");
        a.append(kind, marked(h.title, h.marks), sub);
        li.appendChild(a);
        li.addEventListener("mousemove", function () { if (current !== i) select(i, true); });
        list.appendChild(li);
      });
      status.textContent = hits.length + (hits.length === 1 ? " result" : " results");
      select(0, true);
    }

    function select(i, scroll) {
      if (!hits.length) return;
      i = (i + hits.length) % hits.length;
      var prev = list.children[current];
      if (prev) prev.removeAttribute("aria-selected");
      current = i;
      var li = list.children[i];
      li.setAttribute("aria-selected", "true");
      input.setAttribute("aria-activedescendant", li.id);
      if (scroll) li.scrollIntoView({ block: "nearest" });
      clearTimeout(previewTimer);
      previewTimer = setTimeout(preview, 120);
    }

    function preview() {
      var h = hits[current];
      if (!h) return;
      if (frame.getAttribute("src") === h.url) mark();
      else frame.src = h.url;
    }

    function words() {
      var seg = input.value.split("/").pop().trim().split(/\s+/);
      var first = seg[0] ? seg[0].toLowerCase() : "";
      if (seg.length > 1 && first && KINDS.concat(["organization", "notes"]).some(function (k) { return k.indexOf(first) === 0; })) seg.shift();
      return seg.filter(function (w) { return w && w.indexOf("is:") !== 0; }).map(function (w) { return w.toLowerCase(); });
    }

    function find(doc, root, word) {
      var walker = doc.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
        acceptNode: function (n) {
          return /^(SCRIPT|STYLE)$/.test(n.parentNode.nodeName) ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT;
        }
      });
      for (var n = walker.nextNode(); n; n = walker.nextNode()) {
        var at = n.nodeValue.toLowerCase().indexOf(word);
        if (at < 0) continue;
        var range = doc.createRange();
        range.setStart(n, at);
        range.setEnd(n, at + word.length);
        return range;
      }
      return null;
    }

    function mark() {
      var doc;
      try { doc = frame.contentDocument; } catch (err) { return; }
      if (!doc || !doc.body) return;
      var sel = doc.getSelection();
      sel.removeAllRanges();
      var main = doc.querySelector("main") || doc.body;
      var h1 = main.querySelector("h1");
      var range = null;
      words().some(function (w) {
        range = (h1 && find(doc, h1, w)) || find(doc, main, w);
        return range;
      });
      if (!range) return;
      sel.addRange(range);
      // scroll the frame only: scrollIntoView would also scroll the palette's own box
      var win = frame.contentWindow;
      var top = range.getBoundingClientRect().top + win.scrollY;
      win.scrollTo({ top: Math.max(0, top - win.innerHeight / 2) });
    }

    function complete() {
      var v = input.value, at = v.lastIndexOf("/") + 1;
      var word = v.slice(at).trim().toLowerCase();
      if (!/^[a-z]+$/.test(word)) return false;
      var match = KINDS.filter(function (k) { return k.indexOf(word) === 0; });
      if (word === "organization".slice(0, word.length)) match = ["org"];
      if (match.length !== 1) return false;
      input.value = v.slice(0, at) + match[0] + " ";
      return true;
    }

    input.addEventListener("input", function () {
      clearTimeout(timer);
      timer = setTimeout(query, 60);
    });

    frame.addEventListener("load", function () {
      if (frame.getAttribute("src") !== "about:blank") mark();
      try {
        frame.contentWindow.document.addEventListener("keydown", function (e) {
          if (e.key === "Escape") { close(); }
        });
      } catch (err) {}
    });

    document.addEventListener("keydown", function (e) {
      if (root.hidden) {
        if (e.key === "/" && !e.ctrlKey && !e.metaKey && !e.altKey && !editable(e.target)) {
          e.preventDefault();
          open();
        }
        return;
      }
      var k = e.key;
      if (k === "Escape") { e.preventDefault(); close(); }
      else if (k === "ArrowDown" || (e.ctrlKey && k === "n")) { e.preventDefault(); select(current + 1, true); }
      else if (k === "ArrowUp" || (e.ctrlKey && k === "p")) { e.preventDefault(); select(current - 1, true); }
      else if (k === "Enter" && hits[current]) {
        e.preventDefault();
        if (e.shiftKey || e.ctrlKey || e.metaKey) window.open(hits[current].url, "_blank");
        else location.href = hits[current].url;
      } else if (k === "Tab" && e.target === input) {
        if (e.shiftKey) { e.preventDefault(); return; }
        e.preventDefault();
        if (complete()) { query(); } else if (frame.contentWindow) { frame.contentWindow.focus(); }
      } else if (k === "Tab" && !e.shiftKey && !root.contains(e.target)) {
        e.preventDefault();
        input.focus();
      }
    });
  }

  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", ready);
  else ready();
})();
