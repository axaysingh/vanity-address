// Loaded before the stylesheet paints so the saved theme applies without a flash.
(function () {
  var saved = null;
  try {
    saved = localStorage.getItem("vanity.theme");
  } catch (e) {}
  var system = window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
  document.documentElement.dataset.theme = saved === "light" || saved === "dark" ? saved : system;
})();
