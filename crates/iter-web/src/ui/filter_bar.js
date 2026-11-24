// Escape closes the syntax tip of the filter bar (WCAG 1.4.13) until the "?" is focused
// or hovered again (`.dismissed`). Without it the tip shows as CSS alone shows it.
// A region swapped in by live.js (`iter:live`) brings a fresh bar, armed the same way.
(function () {
  function armAll() {
    document.querySelectorAll(".filter-help").forEach(function (help) {
      if (help.dataset.armed) return;
      help.dataset.armed = "1";
      var btn = help.querySelector(".filter-help-btn");
      help.addEventListener("keydown", function (e) {
        if (e.key === "Escape") help.classList.add("dismissed");
      });
      var reopen = function () { help.classList.remove("dismissed"); };
      btn.addEventListener("focusin", reopen);
      btn.addEventListener("mouseenter", reopen);
    });
  }
  armAll();
  document.addEventListener("iter:live", armAll);
})();
