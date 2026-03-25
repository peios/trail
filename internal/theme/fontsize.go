package theme

const fontSizeJS = `(function() {
  var sizes = ['text-sm', 'text-base', 'text-lg'];
  var labels = ['A', 'A', 'A'];
  var current = parseInt(localStorage.getItem('font-size') || '1', 10);

  var container = document.getElementById('font-size-controls');
  if (!container) return;

  function apply() {
    var prose = document.querySelectorAll('.prose');
    prose.forEach(function(el) {
      sizes.forEach(function(s) { el.classList.remove(s); });
      el.classList.add(sizes[current]);
    });
    container.querySelectorAll('button').forEach(function(btn, i) {
      if (i === current) {
        btn.classList.add('text-brand-600', 'dark:text-brand-400');
        btn.classList.remove('text-gray-400', 'dark:text-gray-500');
      } else {
        btn.classList.remove('text-brand-600', 'dark:text-brand-400');
        btn.classList.add('text-gray-400', 'dark:text-gray-500');
      }
    });
  }

  sizes.forEach(function(_, i) {
    var btn = document.createElement('button');
    btn.textContent = 'A';
    btn.className = 'font-serif hover:text-gray-700 dark:hover:text-gray-200';
    btn.style.fontSize = (12 + i * 4) + 'px';
    btn.addEventListener('click', function() {
      current = i;
      localStorage.setItem('font-size', String(i));
      apply();
    });
    container.appendChild(btn);
  });

  apply();
})();`
