package theme

const themeJS = `(function() {
  var toggle = document.getElementById('theme-toggle');
  var iconLight = document.getElementById('theme-icon-light');
  var iconDark = document.getElementById('theme-icon-dark');

  function updateIcons() {
    var isDark = document.documentElement.classList.contains('dark');
    // Show sun icon in dark mode (click to go light), moon icon in light mode (click to go dark)
    iconLight.classList.toggle('hidden', !isDark);
    iconDark.classList.toggle('hidden', isDark);
  }

  updateIcons();

  toggle.addEventListener('click', function() {
    document.documentElement.classList.toggle('dark');
    var isDark = document.documentElement.classList.contains('dark');
    localStorage.setItem('theme', isDark ? 'dark' : 'light');
    updateIcons();
  });
})();`
