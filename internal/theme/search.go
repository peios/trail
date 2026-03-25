package theme

const searchJS = `(function() {
  var input = document.getElementById('search-input');
  var results = document.getElementById('search-results');
  var fuse;
  var activeIdx = -1;

  fetch('/search-index.json')
    .then(function(r) { return r.json(); })
    .then(function(data) {
      fuse = new Fuse(data, {
        keys: [
          { name: 'title', weight: 2 },
          { name: 'content', weight: 1 }
        ],
        threshold: 0.3,
        includeMatches: true,
        minMatchCharLength: 2
      });
    });

  function getItems() {
    return results.querySelectorAll('a[data-search-item]');
  }

  function setActive(idx) {
    var items = getItems();
    if (items.length === 0) return;

    // Clamp
    if (idx < 0) idx = 0;
    if (idx >= items.length) idx = items.length - 1;

    // Remove previous highlight
    items.forEach(function(el) {
      el.classList.remove('bg-gray-100', 'dark:bg-gray-800');
    });

    activeIdx = idx;
    items[idx].classList.add('bg-gray-100', 'dark:bg-gray-800');
    items[idx].scrollIntoView({ block: 'nearest' });
  }

  function clearActive() {
    activeIdx = -1;
    getItems().forEach(function(el) {
      el.classList.remove('bg-gray-100', 'dark:bg-gray-800');
    });
  }

  input.addEventListener('input', function() {
    var query = input.value.trim();
    activeIdx = -1;

    if (!query || !fuse) {
      results.classList.add('hidden');
      results.innerHTML = '';
      return;
    }

    var matches = fuse.search(query, { limit: 8 });
    if (matches.length === 0) {
      results.innerHTML = '<div class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400">No results found</div>';
      results.classList.remove('hidden');
      return;
    }

    results.innerHTML = matches.map(function(m) {
      var item = m.item;
      var typeLabel = item.type === 'how-to' ? 'How-to' : item.type === 'concept' ? 'Concept' : '';
      return '<a href="/' + item.slug + '/?highlight=' + encodeURIComponent(query) + '" data-search-item class="block px-4 py-3 border-b border-gray-100 dark:border-gray-800 last:border-0 outline-none">' +
        '<div class="text-sm font-medium text-gray-900 dark:text-gray-100">' + escapeHtml(item.title) + '</div>' +
        (item.description ? '<div class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">' + escapeHtml(item.description) + '</div>' : '') +
        '<div class="flex gap-2 mt-0.5">' +
          (typeLabel ? '<span class="text-xs text-brand-600 dark:text-brand-400">' + typeLabel + '</span>' : '') +
          '<span class="text-xs text-gray-400 dark:text-gray-500">' + escapeHtml(item.category) + '</span>' +
        '</div>' +
      '</a>';
    }).join('');
    results.classList.remove('hidden');
  });

  input.addEventListener('keydown', function(e) {
    var items = getItems();
    if (items.length === 0 && e.key !== 'Escape') return;

    if (e.key === 'ArrowDown' || (e.key === 'Tab' && !e.shiftKey)) {
      e.preventDefault();
      setActive(activeIdx + 1);
    } else if (e.key === 'ArrowUp' || (e.key === 'Tab' && e.shiftKey)) {
      e.preventDefault();
      if (activeIdx <= 0) {
        clearActive();
        activeIdx = -1;
      } else {
        setActive(activeIdx - 1);
      }
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (activeIdx >= 0 && items[activeIdx]) {
        items[activeIdx].click();
      }
    } else if (e.key === 'Escape') {
      results.classList.add('hidden');
      clearActive();
      input.blur();
    }
  });

  // Close results on click outside
  document.addEventListener('click', function(e) {
    if (!input.contains(e.target) && !results.contains(e.target)) {
      results.classList.add('hidden');
      clearActive();
    }
  });

  // Keyboard shortcut: / to focus search
  document.addEventListener('keydown', function(e) {
    if (e.key === '/' && document.activeElement !== input && document.activeElement.tagName !== 'INPUT') {
      e.preventDefault();
      input.focus();
    }
  });

  function escapeHtml(s) {
    var div = document.createElement('div');
    div.textContent = s;
    return div.innerHTML;
  }
})();`
