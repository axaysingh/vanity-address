// Loaded before the stylesheet paints so the saved theme applies without a flash.
(function () {
  var saved = null;
  try {
    saved = localStorage.getItem("vanity.theme");
  } catch (e) {}
  document.documentElement.dataset.theme = saved === "dark" ? "dark" : "light";
})();
