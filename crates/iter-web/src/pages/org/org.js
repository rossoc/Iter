// Report tab: applies the From/To dates as they change and wires the Share form.
// A typed date applies on Enter or when focus leaves both fields, not per keystroke
// (Chrome reports a change after every digit of a year); only a year from 1970 counts.
(function () {
  var form = document.querySelector(".range");
  if (!form) return;
  var timer, typing = false;
  var ready = function (v) { return v === "" || (/^\d{4}-\d\d-\d\d$/.test(v) && +v.slice(0, 4) >= 1970); };
  var go = function () { clearTimeout(timer); timer = setTimeout(function () { form.requestSubmit(); }, 250); };
  form.addEventListener("keydown", function (e) { if (e.key !== "Enter" && e.key !== "Tab") typing = true; });
  form.addEventListener("change", function (e) {
    clearTimeout(timer);
    if (!typing && ready(e.target.value)) go();
  });
  // Over the calendar icon a click picks the whole date, so the whole date
  // lights up (.whole, ui/control.css). The icon is the last 14px before the right
  // padding and border; 8px of its left margin count too.
  form.querySelectorAll("input[type=date]").forEach(function (input) {
    input.addEventListener("mousemove", function (e) {
      var r = input.getBoundingClientRect(), cs = getComputedStyle(input);
      var edge = r.right - parseFloat(cs.paddingRight) - parseFloat(cs.borderRightWidth);
      input.classList.toggle("whole", e.clientX >= edge - 14 - 8 && e.clientX <= edge);
    });
    input.addEventListener("mouseleave", function () { input.classList.remove("whole"); });
  });
  form.addEventListener("focusout", function (e) {
    if (!typing || form.contains(e.relatedTarget)) return;
    typing = false;
    var dates = form.querySelectorAll("input[type=date]");
    if (ready(dates[0].value) && ready(dates[1].value)) go();
  });
})();

// The Share pop-up (org_share.rs `share_modal`): picks which settings and button show
// (both show without script); a copy goes to the clipboard, a push is posted. Without a
// clipboard the form posts and the server answers with the text.
(function () {
  var form = document.querySelector("form.sharing");
  if (!form) return;
  var modes = form.querySelectorAll("input[name=mode]");
  var picker = form.querySelector(".share-modes");
  if (picker) picker.hidden = false; // the way to share is chosen here; without script both show
  var status = document.getElementById("copy-status");
  var timer;
  var mode = function () {
    var checked = form.querySelector("input[name=mode]:checked");
    return checked ? checked.value : "copy";
  };
  // a hidden control is disabled too, so only the chosen way is submitted: a hidden but
  // enabled Copy button would be the form's default and Enter in a push field would copy
  var sync = function () {
    var now = mode();
    form.querySelectorAll("[data-for]").forEach(function (el) {
      var on = el.dataset["for"] === now;
      el.hidden = !on;
      if (el.matches("button")) el.disabled = !on;
      el.querySelectorAll("input, select, button").forEach(function (c) { c.disabled = !on; });
    });
  };
  modes.forEach(function (m) { m.addEventListener("change", sync); });
  sync();
  form.addEventListener("submit", function (e) {
    var copy = e.submitter && e.submitter.value === "copy";
    if (!copy || !navigator.clipboard) return;
    e.preventDefault();
    var button = e.submitter;
    var format = form.elements.format;
    navigator.clipboard.writeText(form.dataset[format.value] || "").then(function () {
      button.classList.add("copied");
      status.textContent = "Copied to the clipboard";
      clearTimeout(timer);
      timer = setTimeout(function () {
        button.classList.remove("copied");
        status.textContent = "";
      }, 2000);
    }, function () {
      status.textContent = "Could not copy";
    });
  });
})();
