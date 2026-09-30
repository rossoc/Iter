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
        location.replace(next);
      })
      .catch(function () {
        fail("no answer from the server");
      });
  });
})();
