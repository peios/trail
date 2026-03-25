package theme

const tabsJS = `(function() {
  document.querySelectorAll('[data-tab-group]').forEach(function(group) {
    var buttons = group.querySelectorAll('[data-tab]');
    var panels = group.querySelectorAll('[data-tab-panel]');

    buttons.forEach(function(btn) {
      btn.addEventListener('click', function() {
        var target = btn.getAttribute('data-tab');

        buttons.forEach(function(b) { b.classList.remove('tab-active'); });
        btn.classList.add('tab-active');

        panels.forEach(function(p) {
          if (p.getAttribute('data-tab-panel') === target) {
            p.classList.remove('hidden');
          } else {
            p.classList.add('hidden');
          }
        });
      });
    });
  });
})();`
