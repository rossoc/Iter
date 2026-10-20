// The pop-up's keyboard manners (modal.rs). An enhancement: without it the
// pop-up still closes by its links (the ✕, Cancel, the backdrop) and the page
// behind is still inert (frame.rs). With it, for the pop-up on the page:
//   - the site footer is inert too, so Tab cannot leave the pop-up for it
//     (the footer is outside the frame, which only marks the bar and main);
//   - Escape closes the pop-up: it goes to the ✕'s link, the page without it;
//   - Tab and Shift+Tab stay in the pop-up: from its last visible focusable
//     element to the first, and from the first back to the last.
(function () {
  var box = document.querySelector(".modal-box[role=dialog]");
  if (!box) return;

  var foot = document.querySelector("footer.site-foot");
  if (foot) foot.setAttribute("inert", "");

  var closeLink = box.querySelector(".modal-x");
  document.addEventListener("keydown", function (e) {
    if (e.key === "Escape" && closeLink) location.href = closeLink.href;
  });

  var SELECTOR = 'a[href], button:not([disabled]), input:not([disabled]):not([type=hidden]), select, textarea, [tabindex]:not([tabindex="-1"])';
  var focusables = function () {
    return Array.prototype.filter.call(box.querySelectorAll(SELECTOR), function (el) {
      return el.getClientRects().length > 0;
    });
  };
  document.addEventListener("keydown", function (e) {
    if (e.key !== "Tab") return;
    var items = focusables();
    if (items.length === 0) return;
    var first = items[0], last = items[items.length - 1];
    var outside = !box.contains(document.activeElement);
    if (e.shiftKey && (outside || document.activeElement === first)) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && (outside || document.activeElement === last)) {
      e.preventDefault();
      first.focus();
    }
  });
})();
