// Activities suggested for a time platform project (platform_pick.rs): when the
// project field changes, the new project's activities are fetched into the
// <datalist>. Without script or an answer the fields are plain text.
(function () {
  var list = document.getElementById("platform-activities");
  // the combobox (combobox.js) takes `list` off the field, so it is found by id
  var project = document.getElementById("f-organization") || document.getElementById("f-platform_project");
  if (!list || !project || !list.dataset.url) return;
  var asked = project.value, timer;
  var load = function () {
    var name = project.value.trim();
    if (name === asked) return;
    asked = name;
    fetch(list.dataset.url + "?project=" + encodeURIComponent(name), { headers: { Accept: "application/json" } })
      .then(function (r) { if (!r.ok) throw new Error(r.status); return r.json(); })
      .then(function (d) {
        if (project.value.trim() !== name) return;
        list.textContent = "";
        d.activities.forEach(function (a) {
          var o = document.createElement("option");
          o.value = a;
          list.appendChild(o);
        });
      })
      .catch(function () {});
  };
  ["change", "input"].forEach(function (ev) {
    project.addEventListener(ev, function () { clearTimeout(timer); timer = setTimeout(load, 300); });
  });
})();
