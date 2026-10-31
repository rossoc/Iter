// Drag-and-drop for the board pages (agenda, matrix): an enhancement, as every
// page also moves a task with plain links and forms (the pick mode).
//
// The contract, all in the markup (the script knows no page):
//   [data-post]   the board's root; the URL a drop is POSTed to (task_id, target).
//                 The script adds .can-drag: grab cursor and dashed drop targets.
//   [data-task]   a draggable card (the task's id); the card's own id is
//                 card-<task>, what a link (#card-7) or a focus lands on.
//   [data-target] a drop zone; what is POSTed as `target`.
//   .dragging     on the card while dragged; .over on the zone above it.
//   .board-notes  (optional, a role=status region in the root) where a done move
//                 and a failed drop (as an alert) are said; made if missing.
// The server decides where a card belongs, so after a good drop the page reloads
// (?moved=<task>, as a pick form's redirect): the keyboard lands on #card-<task>
// and the status says "Moved <task> to <place>". A failed drop says so and stays.
// Agenda extra, only when the page has the pieces: the day title opens the date
// picker; a typed or stepped date goes on Enter or when focus leaves the form, not per
// keystroke (Chrome reports a change after every digit of a year); a year from
// 1970 counts. Without script the date is a plain field with a hidden submit button.
(function () {
  var input = document.getElementById("day-input");
  var label = document.querySelector(".day-label");
  if (input && label && input.form) {
    var form = input.form;
    var typing = false;
    var ready = function (v) { return /^\d{4}-\d\d-\d\d$/.test(v) && +v.slice(0, 4) >= 1970; };
    var go = function () {
      typing = false;
      if (form.requestSubmit) form.requestSubmit();
      else form.submit();
    };
    label.addEventListener("click", function (e) {
      typing = false;
      if (typeof input.showPicker !== "function") return;
      e.preventDefault();
      try {
        input.showPicker();
      } catch (_) {
        input.focus();
      }
    });
    input.addEventListener("keydown", function (e) { if (e.key !== "Enter" && e.key !== "Tab") typing = true; });
    input.addEventListener("change", function () { if (!typing && ready(input.value)) go(); });
    form.addEventListener("focusout", function (e) {
      if (!typing || form.contains(e.relatedTarget)) return;
      if (ready(input.value)) go();
      else typing = false;
    });
  }
})();

(function () {
  var board = document.querySelector("[data-post]");
  if (!board) return;

  board.classList.add("can-drag");
  Array.prototype.forEach.call(board.querySelectorAll("[data-task]"), function (card) {
    card.setAttribute("draggable", "true");
  });

  // A live region reads a change, not what was there on load: write the note again once the page is up.
  var notes = board.querySelector(".board-notes");
  if (notes && notes.textContent.trim()) {
    var said = notes.firstElementChild;
    var text = said.textContent;
    said.textContent = "";
    setTimeout(function () {
      said.textContent = text;
    }, 100);
  }

  // The note is for that one load: drop `moved` from the URL.
  var here = new URL(location.href);
  if (here.searchParams.has("moved")) {
    here.searchParams.delete("moved");
    history.replaceState(null, "", here);
  }

  // Scroll across a board action: each pick step, posting form and drop reloads the
  // page at #card-<task>, which the browser scrolls to the window top. So the scroll is
  // kept in sessionStorage before leaving and restored on the next load of the same
  // board and day (key: path and ?date=), after the browser's fragment scroll; the
  // card is focused without scrolling. Blocked storage just loads at the card.
  var SCROLL = "board-scroll";

  function key(url) {
    url = new URL(url.href);
    return url.pathname + "?" + (url.searchParams.get("date") || "");
  }

  function saveScroll() {
    try {
      sessionStorage.setItem(SCROLL, JSON.stringify({ key: key(location), y: window.scrollY }));
    } catch (_) {
    }
  }

  var saved = null;
  try {
    saved = JSON.parse(sessionStorage.getItem(SCROLL));
    sessionStorage.removeItem(SCROLL);
  } catch (_) {
  }
  if (saved && saved.key === key(location)) {
    history.scrollRestoration = "manual";
    var restore = function () {
      requestAnimationFrame(function () {
        window.scrollTo(0, saved.y);
        var id = location.hash.slice(1);
        var card = id.indexOf("card-") === 0 && document.getElementById(id);
        if (card) card.focus({ preventScroll: true });
      });
    };
    if (document.readyState === "complete") restore();
    else window.addEventListener("load", restore);
  }

  // A pick-step link or a posting form (pick-here, unschedule, unplace) is a board action.
  document.addEventListener("click", function (e) {
    var a = e.target.closest && e.target.closest("a[href]");
    if (!a) return;
    var to = new URL(a.href);
    if (to.pathname !== location.pathname) return;
    if (to.hash.indexOf("#card-") === 0 || to.searchParams.has("pick")) saveScroll();
  });

  document.addEventListener("submit", function (e) {
    var form = e.target;
    if (form.method !== "post") return;
    if (board.contains(form) || (form.closest && form.closest(".pickbar"))) saveScroll();
  });

  function fail(what) {
    var notes = board.querySelector(".board-notes");
    if (!notes) {
      notes = document.createElement("div");
      notes.className = "board-notes";
      board.insertBefore(notes, board.firstChild);
    }
    var note = document.createElement("p");
    note.className = "notice error";
    note.setAttribute("role", "alert");
    note.textContent = "The task could not be moved (" + what + "). Nothing changed.";
    notes.replaceChildren(note);
  }

  document.addEventListener("dragstart", function (e) {
    var card = e.target.closest && e.target.closest("[data-task]");
    if (!card) return;
    e.dataTransfer.setData("text/plain", card.dataset.task);
    e.dataTransfer.effectAllowed = "move";
    card.classList.add("dragging");
  });

  document.addEventListener("dragend", function (e) {
    var card = e.target.closest && e.target.closest("[data-task]");
    if (card) card.classList.remove("dragging");
  });

  function zoneOf(e) {
    return e.target.closest && e.target.closest("[data-target]");
  }

  document.addEventListener("dragover", function (e) {
    var zone = zoneOf(e);
    if (!zone) return;
    e.preventDefault();
    zone.classList.add("over");
  });

  document.addEventListener("dragleave", function (e) {
    var zone = zoneOf(e);
    if (zone && !zone.contains(e.relatedTarget)) zone.classList.remove("over");
  });

  document.addEventListener("drop", function (e) {
    var zone = zoneOf(e);
    if (!zone) return;
    e.preventDefault();
    zone.classList.remove("over");
    var task = e.dataTransfer.getData("text/plain");
    var body = new URLSearchParams({ task_id: task, target: zone.dataset.target });
    fetch(board.dataset.post, { method: "POST", body: body })
      .then(function (r) {
        if (!r.ok) return fail("error " + r.status);
        // Back to the same page on the moved card, as a pick form's redirect does.
        var next = new URL(location.href);
        next.searchParams.set("moved", task);
        next.hash = "card-" + task;
        saveScroll();
        location.replace(next);
      })
      .catch(function () {
        fail("no answer from the server");
      });
  });
})();
