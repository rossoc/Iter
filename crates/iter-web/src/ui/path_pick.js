// The folder button of a path field (field.rs `path_field`). The button is `hidden`
// until this runs; the path can always be typed. A click asks the server to open the
// system's folder chooser (POST /folders/pick) and puts the chosen folder in the input.
// A region swapped in by live.js (`iter:live`) brings fresh buttons, armed the same way.
(function () {
  function arm(button) {
    var input = document.getElementById(button.dataset.pathPick);
    if (!input || button.dataset.armed) return;
    button.dataset.armed = "1";
    button.hidden = false;
    var busy = false;
    button.addEventListener("click", function () {
      if (busy) return;
      busy = true;
      button.disabled = true;
      fetch("/folders/pick", {
        method: "POST",
        headers: { Accept: "application/json", "Content-Type": "application/x-www-form-urlencoded" },
        body: "start=" + encodeURIComponent(input.value)
      })
        .then(function (r) { if (!r.ok) throw new Error(r.status); return r.json(); })
        .then(function (d) {
          if (d.path) {
            input.value = d.path;
            input.dispatchEvent(new Event("input", { bubbles: true }));
            input.dispatchEvent(new Event("change", { bubbles: true }));
          } else if (d.unavailable) {
            button.hidden = true; // no chooser on this system: type the path
            input.focus();
          }
        })
        .catch(function () {})
        .then(function () { busy = false; button.disabled = false; input.focus(); });
    });
  }

  function armAll() {
    document.querySelectorAll("button[data-path-pick]").forEach(arm);
  }
  armAll();
  document.addEventListener("iter:live", armAll);
})();
