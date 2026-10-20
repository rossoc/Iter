// Drag-and-drop for the board pages (the agenda and the matrix). It is
// an enhancement: every page also moves a task with plain links and forms
// (the pick mode), so nothing here is needed to use the page.
//
// The contract, all in the markup (the script knows no page):
//   [data-post]   the board's root; the URL a drop is POSTed to, as a form
//                 (task_id, target). The script adds .can-drag to it: the
//                 grab cursor and the dashed drop targets show only then.
//   [data-task]   a card that can be dragged (the task's id); the script
//                 makes it draggable. It is dragged by its id: the card's own
//                 id is card-<task>, what a link (#card-7) or a focus lands on.
//   [data-target] a drop zone; what is POSTed as `target` (an hour's start,
//                 empty for the lists, a quadrant's name).
//   .dragging     set on the card while it is dragged; .over on the zone
//                 above which it is.
//   .board-notes  (optional, inside the root; a role=status region) where the
//                 page says a move was done and where a failed drop is said,
//                 as an alert; made when the page has none.
// The server decides where a card belongs, so after a drop that worked the
// page loads again (with ?moved=<task>, as a pick form's redirect does): the
// keyboard lands on the moved card (the #card-<task> fragment) and the
// status region says "Moved <task> to <place>". One that failed says so and
// leaves the page as it is.
// The agenda's convenience, only when the page has the piece: the day's
// title opens the browser's date picker (a date picked there goes to that
// day at once). A date typed into the field, or stepped with the arrow keys,
// goes when the typing ends: on Enter (the form's hidden submit button), or
// when focus leaves the form, not on each keystroke, since Chrome reports a
// change after every digit of a year ("0002", "0020", ...). Only a complete
// date with a four-digit year from 1970 counts. Without script the date is a
// plain field with a hidden submit button. (The Not scheduled column folds
// with a link, ?side=off: the server draws it.)
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

  // The page says a move was done ("Moved Task to Plan") in the status region.
  // A live region reads a change, not what was there when the page loaded, so
  // the note is written again once the page is up.
  var notes = board.querySelector(".board-notes");
  if (notes && notes.textContent.trim()) {
    var said = notes.firstElementChild;
    var text = said.textContent;
    said.textContent = "";
    setTimeout(function () {
      said.textContent = text;
    }, 100);
  }

  // The note is for that one load: leave `moved` out of the URL (a reload
  // or a copied link does not say it again).
  var here = new URL(location.href);
  if (here.searchParams.has("moved")) {
    here.searchParams.delete("moved");
    history.replaceState(null, "", here);
  }

  // The scroll position across a board action. Each pick step (Schedule, Move,
  // Place, Cancel), each form that posts and each drop leaves the page by a
  // full load whose URL ends in #card-<task>, and the browser scrolls that card
  // to the top of the window on every one. So before a board action leaves the
  // page its scroll is kept in sessionStorage, and the next load of the same
  // board on the same day (the key: path and ?date=) puts it back, once the
  // browser has done its own fragment scroll. The #card-<task> hash stays, so
  // the keyboard still lands on the card: it is focused without scrolling. A
  // different board or day drops the entry; storage can be blocked, and then
  // the page loads at the card as it did before.
  var SCROLL = "board-scroll";

  function key(url) {
    url = new URL(url.href);
    return url.pathname + "?" + (url.searchParams.get("date") || "");
  }

  function saveScroll() {
    try {
      sessionStorage.setItem(SCROLL, JSON.stringify({ key: key(location), y: window.scrollY }));
    } catch (_) {
      // Storage is blocked or full: the page still works without it.
    }
  }

  var saved = null;
  try {
    saved = JSON.parse(sessionStorage.getItem(SCROLL));
    sessionStorage.removeItem(SCROLL);
  } catch (_) {
    // Nothing saved, or storage is blocked.
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

  // A link to a pick step (its #card- fragment or its pick= parameter) or a
  // form that posts (the pick-here, unschedule and unplace forms, in the board
  // or the pick bar) is a board action.
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
        // Back to the same page, on the moved card (the keyboard lands on it),
        // which says it was moved: what a pick form's redirect does too.
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
