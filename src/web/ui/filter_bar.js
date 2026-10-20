// The syntax tip of the filter bar ("?", filter_bar.rs) closes with Escape
// (WCAG 1.4.13): the tip shows on hover and focus, and Escape hides it until
// the "?" is focused or hovered again (`.dismissed`, filter_bar.css). An
// enhancement: without it the tip shows as the CSS alone shows it.
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
