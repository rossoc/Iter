// The activities suggested for a time platform project (platform_pick.rs).
// An enhancement: the lists are filled by the server for the project stored;
// when the project field changes, the activities of the new one are fetched
// and put in the activities' <datalist>. Without script, or when the platform
// does not answer, the fields are plain text.
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
