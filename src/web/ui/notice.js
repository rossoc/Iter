// A notice with `data-focus` (notice.rs) takes focus on load. Its `role="alert"` is
// dropped first, or the message would be read twice (once as an alert, once as focus).
document.querySelectorAll("[data-focus]").forEach(function (n) {
  n.removeAttribute("role");
  n.focus();
});
