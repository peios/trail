package theme

const pathwayJS = `(function() {
  var params = new URLSearchParams(window.location.search);
  var pathwaySlug = params.get('pathway');
  if (!pathwaySlug) return;

  var path = window.location.pathname.replace(/^\/|\/$/g, '');

  fetch('/pathways.json')
    .then(function(r) { return r.json(); })
    .then(function(pathways) {
      var pathway = pathways.find(function(p) { return p.slug === pathwaySlug; });
      if (!pathway) return;

      var idx = -1;
      for (var i = 0; i < pathway.pages.length; i++) {
        if (pathway.pages[i].slug === path) { idx = i; break; }
      }
      if (idx === -1) return;

      // Replace sidebar with pathway page list
      var sidebar = document.querySelector('aside nav');
      if (sidebar) {
        var heading = sidebar.querySelector('h3');
        if (heading) heading.textContent = pathway.name;

        var list = sidebar.querySelector('ul');
        if (list) {
          list.innerHTML = pathway.pages.map(function(p) {
            var isCurrent = p.slug === path;
            var cls = isCurrent
              ? 'bg-brand-50 dark:bg-brand-900 text-brand-700 dark:text-brand-400 font-medium'
              : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100 hover:bg-gray-50 dark:hover:bg-gray-800';
            return '<li><a href="/' + p.slug + '/?pathway=' + pathwaySlug + '" class="block py-1.5 px-3 rounded text-sm ' + cls + '">' +
              escapeHtml(p.title) + '</a></li>';
          }).join('');
        }
      }

      // Show prev/next navigation
      var nav = document.getElementById('pathway-nav');
      var prevLink = document.getElementById('pathway-prev');
      var prevTitle = document.getElementById('pathway-prev-title');
      var nextLink = document.getElementById('pathway-next');
      var nextTitle = document.getElementById('pathway-next-title');
      nav.classList.remove('hidden');

      if (idx > 0) {
        var prev = pathway.pages[idx - 1];
        prevLink.href = '/' + prev.slug + '/?pathway=' + pathwaySlug;
        prevTitle.textContent = prev.title;
        prevLink.classList.remove('invisible');
      } else {
        prevLink.classList.add('invisible');
      }

      if (idx < pathway.pages.length - 1) {
        var next = pathway.pages[idx + 1];
        nextLink.href = '/' + next.slug + '/?pathway=' + pathwaySlug;
        nextTitle.textContent = next.title;
        nextLink.classList.remove('invisible');
      } else {
        nextLink.classList.add('invisible');
      }
    })
    .catch(function() {});

  function escapeHtml(s) {
    var div = document.createElement('div');
    div.textContent = s;
    return div.innerHTML;
  }
})();`
