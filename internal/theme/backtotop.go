package theme

const backToTopJS = `(function() {
  var btn = document.getElementById('back-to-top');
  if (!btn) return;

  var media = window.matchMedia('(min-width: 1024px)');
  var scrollRoot = document.querySelector('[data-scroll-root]') || document.querySelector('main');

  function currentScrollTop() {
    if (scrollRoot && media.matches) return scrollRoot.scrollTop;
    return window.scrollY;
  }

  function sync() {
    if (currentScrollTop() > 400) {
      btn.classList.remove('hidden');
    } else {
      btn.classList.add('hidden');
    }
  }

  window.addEventListener('scroll', sync, { passive: true });
  if (scrollRoot) scrollRoot.addEventListener('scroll', sync, { passive: true });

  btn.addEventListener('click', function() {
    if (scrollRoot && media.matches) {
      scrollRoot.scrollTo({ top: 0, behavior: 'smooth' });
      return;
    }
    window.scrollTo({ top: 0, behavior: 'smooth' });
  });

  sync();
})();`
