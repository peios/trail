package theme

const scrollSpyJS = `(function() {
  var outline = document.querySelector('[data-outline-nav]');
  if (!outline) return;

  var links = outline.querySelectorAll('a[href^="#"]');
  if (links.length === 0) return;

  var scrollRoot = document.querySelector('[data-scroll-root]');
  var media = window.matchMedia('(min-width: 1024px)');
  var observerRoot = scrollRoot && media.matches ? scrollRoot : null;
  var headingIds = [];
  links.forEach(function(link) {
    headingIds.push(link.getAttribute('href').slice(1));
  });

  var activeLink = null;

  function setActive(id) {
    var link = outline.querySelector('a[href="#' + id + '"]');
    if (!link || link === activeLink) return;

    if (activeLink) {
      activeLink.classList.remove('text-brand-600', 'dark:text-brand-400', 'font-medium');
      activeLink.classList.add('text-gray-600', 'dark:text-gray-400');
    }
    link.classList.remove('text-gray-600', 'dark:text-gray-400');
    link.classList.add('text-brand-600', 'dark:text-brand-400', 'font-medium');
    activeLink = link;

    if (typeof link.scrollIntoView === 'function') {
      link.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    }
  }

  var observer = new IntersectionObserver(function(entries) {
    entries.forEach(function(entry) {
      if (entry.isIntersecting) {
        setActive(entry.target.id);
      }
    });
  }, {
    root: observerRoot,
    rootMargin: '0px 0px -70% 0px',
    threshold: 0
  });

  headingIds.forEach(function(id) {
    var el = document.getElementById(id);
    if (el) observer.observe(el);
  });

  // Set initial active heading on page load
  if (window.location.hash) {
    setActive(window.location.hash.slice(1));
  } else {
    var rootTop = observerRoot ? observerRoot.getBoundingClientRect().top : 0;
    // Find the first heading above or at the current scroll position
    for (var i = headingIds.length - 1; i >= 0; i--) {
      var el = document.getElementById(headingIds[i]);
      if (el && el.getBoundingClientRect().top - rootTop <= 100) {
        setActive(headingIds[i]);
        break;
      }
    }
    // If nothing found (at top of page), highlight the first heading
    if (!activeLink && headingIds.length > 0) {
      setActive(headingIds[0]);
    }
  }
})();`
