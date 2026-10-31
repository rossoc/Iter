// Escape closes the syntax tip of the filter bar (WCAG 1.4.13) until the "?" is focused
// or hovered again (`.dismissed`). Without it the tip shows as CSS alone shows it.
(function () {
  document.querySelectorAll(".filter-help").forEach(function (help) {
    var btn = help.querySelector(".filter-help-btn");
    help.addEventListener("keydown", function (e) {
      if (e.key === "Escape") help.classList.add("dismissed");
    });
    var reopen = function () { help.classList.remove("dismissed"); };
    btn.addEventListener("focusin", reopen);
    btn.addEventListener("mouseenter", reopen);
  });
})();
