// The folder button of a path field (field.rs `path_field`). An enhancement:
// the button is `hidden` until this runs, and the path can always be typed.
// A click asks the server to open the system's folder chooser at the typed
// path (POST /folders/pick, a same-origin fetch like the other scripts'),
// waits while it is open, and puts the folder chosen in the input.
(function () {
  document.querySelectorAll("button[data-path-pick]").forEach(function (button) {
    var input = document.getElementById(button.dataset.pathPick);
    if (!input) return;
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
  });
})();
