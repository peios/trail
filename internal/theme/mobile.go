package theme

const mobileJS = `(function() {
  var toggle = document.getElementById('mobile-menu-toggle');
  var menu = document.getElementById('mobile-menu');
  var iconOpen = document.getElementById('mobile-menu-open');
  var iconClose = document.getElementById('mobile-menu-close');

  toggle.addEventListener('click', function() {
    var isHidden = menu.classList.contains('hidden');
    menu.classList.toggle('hidden');
    iconOpen.classList.toggle('hidden', !isHidden);
    iconClose.classList.toggle('hidden', isHidden);
  });
})();`
