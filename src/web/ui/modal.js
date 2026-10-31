// The pop-up's keyboard manners (modal.rs). Without it the pop-up still closes by its
// links and the page behind is still inert (frame.rs). With it:
//   - the site footer is inert too (it is outside the frame, which marks only bar and main);
//   - Escape follows the close link (the ✕);
//   - Tab and Shift+Tab wrap between the first and last focusable element.
(function () {
  var box = document.querySelector(".modal-box[role=dialog]");
  if (!box) return;

  var foot = document.querySelector("footer.site-foot");
  if (foot) foot.setAttribute("inert", "");

  // The instruction field: picking another instruction fetches its prompt (and model)
  // into the form, unless the prompt was edited since load (differs from `data-loaded`).
  // Without script, Enter in the field submits the small GET form that reloads the pop-up.
  var post = box.querySelector("form[method=post]");
  var promptArea = post && post.querySelector("textarea[name=prompt]");
  var model = post && post.querySelector("input[name=model]");
  var chosen = post && post.querySelector("input[type=hidden][name=instruction]");
  var modelTouched = false;
  if (model) {
    ["input", "change"].forEach(function (ev) {
      model.addEventListener(ev, function () { modelTouched = true; });
    });
  }
  Array.prototype.forEach.call(box.querySelectorAll("input[data-autoload]"), function (input) {
    var asked = input.value;
    var load = function () {
      var name = input.value.trim();
      if (!name || name === asked || !post || !promptArea || !chosen) return;
      asked = name;
      var url = post.getAttribute("action").replace(/\/runs$/, "/prompt") +
        "?instruction=" + encodeURIComponent(name);
      fetch(url, { headers: { Accept: "application/json" } })
        .then(function (r) { if (!r.ok) throw new Error(r.status); return r.json(); })
        .then(function (d) {
          chosen.value = name;
          if (promptArea.value === promptArea.dataset.loaded) {
            promptArea.value = d.prompt;
            promptArea.dataset.loaded = d.prompt;
          }
          if (d.model && model && !modelTouched) model.value = d.model;
        })
        .catch(function () { asked = chosen.value; });
    };
    input.addEventListener("change", load);
    input.addEventListener("keydown", function (e) {
      if (e.key === "Enter") { e.preventDefault(); load(); }
    });
  });

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
