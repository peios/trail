package theme

const backToTopJS = `(function() {
  var btn = document.getElementById('back-to-top');
  if (!btn) return;

  window.addEventListener('scroll', function() {
    if (window.scrollY > 400) {
      btn.classList.remove('hidden');
    } else {
      btn.classList.add('hidden');
    }
  });

  btn.addEventListener('click', function() {
    window.scrollTo({ top: 0, behavior: 'smooth' });
  });
})();`
