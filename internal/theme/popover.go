package theme

const popoverJS = `(function() {
  var popover = null;
  var dictMap = null;
  var loading = false;
  var hideTimeout = null;
  var basePath = window.__basePath || '/';
  var hasHover = window.matchMedia('(hover: hover)').matches;

  function loadDict(cb) {
    if (dictMap) { cb(); return; }
    if (loading) return;
    loading = true;
    fetch(basePath + 'dictionary.json')
      .then(function(r) { return r.json(); })
      .then(function(data) {
        dictMap = {};
        data.forEach(function(e) { dictMap[e.term.toLowerCase()] = e; });
        loading = false;
        cb();
      })
      .catch(function() { loading = false; });
  }

  function esc(s) {
    var d = document.createElement('div');
    d.textContent = s;
    return d.innerHTML;
  }

  function getPopover() {
    if (popover) return popover;
    popover = document.createElement('div');
    popover.id = 'dict-popover';
    popover.style.cssText = 'display:none;position:absolute;z-index:9999;max-width:320px;';
    popover.className = 'p-3 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg';
    document.body.appendChild(popover);

    if (hasHover) {
      popover.addEventListener('mouseenter', cancelHide);
      popover.addEventListener('mouseleave', scheduleHide);
    }
    return popover;
  }

  function show(target, entry) {
    var p = getPopover();
    var slug = entry.term.toLowerCase().replace(/ /g, '-');

    var h = '<div class="font-semibold text-sm text-gray-900 dark:text-gray-100">' + esc(entry.term);
    if (entry.abbr) h += ' <span class="text-xs font-mono text-brand-600 dark:text-brand-400 bg-brand-50 dark:bg-brand-900/30 px-1 py-0.5 rounded">' + esc(entry.abbr) + '</span>';
    h += '</div>';
    h += '<p class="text-xs text-gray-600 dark:text-gray-300 mt-1.5 mb-2 leading-relaxed">' + esc(entry.definition) + '</p>';
    h += '<div class="flex items-center gap-2">';
    if (entry.category) h += '<span class="text-xs text-gray-400 dark:text-gray-500 bg-gray-100 dark:bg-gray-800 px-1.5 py-0.5 rounded">' + esc(entry.category) + '</span>';
    h += '<a href="' + basePath + 'dictionary/#term-' + slug + '" class="text-xs text-brand-600 dark:text-brand-400 hover:text-brand-800 dark:hover:text-brand-200 no-underline">Full entry &rarr;</a>';
    h += '</div>';

    p.innerHTML = h;
    p.style.display = 'block';

    // Position below the term, centered.
    var rect = target.getBoundingClientRect();
    var pw = p.offsetWidth;
    var ph = p.offsetHeight;
    var left = rect.left + (rect.width / 2) - (pw / 2) + window.scrollX;
    var top = rect.bottom + 6 + window.scrollY;

    // Keep within viewport.
    if (left < 8) left = 8;
    if (left + pw > window.innerWidth - 8) left = window.innerWidth - pw - 8;

    // Flip above if no room below.
    if (rect.bottom + ph + 14 > window.innerHeight) {
      top = rect.top - ph - 6 + window.scrollY;
    }

    p.style.left = left + 'px';
    p.style.top = top + 'px';
  }

  function hide() {
    if (popover) popover.style.display = 'none';
  }

  function scheduleHide() {
    hideTimeout = setTimeout(hide, 150);
  }

  function cancelHide() {
    if (hideTimeout) { clearTimeout(hideTimeout); hideTimeout = null; }
  }

  // Hover mode (desktop).
  if (hasHover) {
    document.addEventListener('mouseover', function(e) {
      var t = e.target.closest('.dict-term');
      if (!t) return;
      cancelHide();
      var term = t.getAttribute('data-dict-term');
      if (!term) return;
      loadDict(function() {
        var entry = dictMap[term.toLowerCase()];
        if (entry) show(t, entry);
      });
    });
    document.addEventListener('mouseout', function(e) {
      if (e.target.closest('.dict-term')) scheduleHide();
    });
  }

  // Click mode (mobile, or desktop click-through).
  document.addEventListener('click', function(e) {
    var t = e.target.closest('.dict-term');
    if (t) {
      e.preventDefault();
      var term = t.getAttribute('data-dict-term');
      if (!term) return;
      loadDict(function() {
        var entry = dictMap[term.toLowerCase()];
        if (entry) show(t, entry);
      });
      return;
    }
    // Click outside popover dismisses it.
    if (!e.target.closest('#dict-popover')) hide();
  });

  // Escape dismisses.
  document.addEventListener('keydown', function(e) {
    if (e.key === 'Escape') hide();
  });
})();`
