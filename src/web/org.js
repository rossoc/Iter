// The report tab's script: applies the From/To dates as they change, and wires
// the Copy button (shown only where the clipboard is available).
//
// Dates: a date picked from the calendar applies at once. A typed one applies
// on Enter (the form's Apply button) or when focus leaves both fields, not on
// each keystroke, since Chrome reports a change after every digit of a year
// ("0002", "0020", ...). Only a four-digit year from 1970 counts as typed.
// Copy: copies `data-copy` and says so for two seconds.
(function () {
  var form = document.querySelector(".v2 .range");
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
  // lights up (.whole, v2.css). The icon is the last 14px before the right
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

(function () {
  var button = document.querySelector(".v2 .copy");
  if (!button || !navigator.clipboard) return;
  var status = document.getElementById("copy-status");
  var timer;
  button.hidden = false;
  button.addEventListener("click", function () {
    navigator.clipboard.writeText(button.dataset.copy).then(function () {
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
