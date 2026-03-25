package theme

const highlightJS = `(function() {
  var params = new URLSearchParams(window.location.search);
  var term = params.get('highlight');
  if (!term) return;

  var article = document.querySelector('article .prose');
  if (!article) return;

  var walker = document.createTreeWalker(article, NodeFilter.SHOW_TEXT, null);
  var nodes = [];
  while (walker.nextNode()) {
    if (walker.currentNode.parentElement.tagName !== 'SCRIPT' &&
        walker.currentNode.parentElement.tagName !== 'STYLE' &&
        walker.currentNode.parentElement.tagName !== 'CODE') {
      nodes.push(walker.currentNode);
    }
  }

  var regex = new RegExp('(' + term.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + ')', 'gi');
  nodes.forEach(function(node) {
    if (!regex.test(node.textContent)) return;
    var span = document.createElement('span');
    span.innerHTML = node.textContent.replace(regex, '<mark class="bg-yellow-200 dark:bg-yellow-800 rounded px-0.5">$1</mark>');
    node.parentNode.replaceChild(span, node);
  });
})();`
