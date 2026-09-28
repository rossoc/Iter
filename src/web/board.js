// Drag-and-drop for the board pages. Cards carry `data-task`; every drop
// zone carries `data-target`; the board root carries `data-post`, the URL a
// drop is sent to. The page reloads afterwards, so the server stays the
// only place that decides where a card belongs.
(function () {
  var board = document.querySelector(".board");
  if (!board) return;

  document.addEventListener("dragstart", function (e) {
    var card = e.target.closest && e.target.closest(".card");
    if (!card) return;
    e.dataTransfer.setData("text/plain", card.dataset.task);
    e.dataTransfer.effectAllowed = "move";
  });

  function zoneOf(e) {
    return e.target.closest && e.target.closest(".zone");
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
    var body = new URLSearchParams({
      task_id: e.dataTransfer.getData("text/plain"),
      target: zone.dataset.target,
    });
    fetch(board.dataset.post, { method: "POST", body: body })
      .then(function (r) {
        if (!r.ok) console.error("drop failed", r.status);
        location.reload();
      })
      .catch(function (err) {
        console.error("drop failed", err);
      });
  });
})();
