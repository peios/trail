package theme

const copyCodeJS = `(function() {
  document.querySelectorAll('.prose pre').forEach(function(pre) {
    var btn = document.createElement('button');
    btn.textContent = 'Copy';
    btn.className = 'absolute top-2 right-2 px-2 py-1 text-xs rounded bg-gray-700 hover:bg-gray-600 text-gray-300 opacity-0 transition-opacity';
    btn.style.cssText = 'position:absolute;top:0.5rem;right:0.5rem;';
    pre.style.position = 'relative';
    pre.appendChild(btn);

    pre.addEventListener('mouseenter', function() { btn.style.opacity = '1'; });
    pre.addEventListener('mouseleave', function() { btn.style.opacity = '0'; });

    btn.addEventListener('click', function() {
      var code = pre.querySelector('code');
      var text = code ? code.textContent : pre.textContent;
      navigator.clipboard.writeText(text).then(function() {
        btn.textContent = 'Copied!';
        setTimeout(function() { btn.textContent = 'Copy'; }, 1500);
      });
    });
  });
})();`
