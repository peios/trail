package theme

const dictionaryJS = `(function() {
  var input = document.getElementById('dict-search');
  if (!input) return;

  var entries = document.querySelectorAll('[data-dict-entry]');
  var groups = document.querySelectorAll('[data-dict-group]');

  input.addEventListener('input', function() {
    var q = input.value.toLowerCase().trim();

    entries.forEach(function(el) {
      var text = el.getAttribute('data-dict-entry').toLowerCase();
      el.style.display = (q === '' || text.indexOf(q) !== -1) ? '' : 'none';
    });

    // Hide empty groups
    groups.forEach(function(g) {
      var visible = g.querySelectorAll('[data-dict-entry]:not([style*="display: none"])');
      g.style.display = visible.length > 0 ? '' : 'none';
    });
  });
})();`

const dictionaryTemplate = `{{define "title"}}Dictionary — {{.Site.Title}}{{end}}
{{define "content"}}
<div class="max-w-5xl mx-auto px-4 sm:px-6 lg:px-8 py-8 sm:py-12">
  <nav class="text-sm mb-6">
    <a href="{{.Site.BasePath}}" class="text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200">Home</a>
    <span class="text-gray-400 dark:text-gray-600 mx-2">/</span>
    <span class="text-gray-900 dark:text-gray-100">Dictionary</span>
  </nav>

  <div class="mb-8">
    <h1 class="text-3xl font-bold text-gray-900 dark:text-gray-100 mb-2">Dictionary</h1>
    <p class="text-gray-500 dark:text-gray-400">{{len .Terms}} terms across Peios subsystems.</p>
  </div>

  <!-- Search -->
  <div class="mb-6">
    <input id="dict-search" type="text" placeholder="Filter terms..." class="w-full sm:w-80 px-4 py-2 border border-gray-300 dark:border-gray-700 rounded-lg bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100 text-sm focus:outline-none focus:ring-2 focus:ring-brand-500 focus:border-brand-500">
  </div>

  <!-- Tabs -->
  <div data-tab-group="dict-views">
    <div class="flex border-b border-gray-200 dark:border-gray-700 mb-6">
      <button class="px-4 py-2 text-sm font-medium border-b-2 -mb-px transition-colors tab-active" data-tab="dict-views-az">A–Z</button>
      <button class="px-4 py-2 text-sm font-medium border-b-2 -mb-px transition-colors" data-tab="dict-views-cat">By Category</button>
      <button class="px-4 py-2 text-sm font-medium border-b-2 -mb-px transition-colors" data-tab="dict-views-usage">By Usage</button>
    </div>

    <!-- A-Z View -->
    <div data-tab-panel="dict-views-az">
      {{if .Letters}}
      <div class="flex flex-wrap gap-1 mb-6">
        {{range .Letters}}<a href="#letter-{{.}}" class="px-2 py-1 text-xs font-medium text-brand-600 dark:text-brand-400 hover:bg-brand-50 dark:hover:bg-brand-900/30 rounded">{{.}}</a>{{end}}
      </div>
      {{end}}
      {{range .ByLetter}}
      <div id="letter-{{.Letter}}" data-dict-group>
        <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-3 mt-6 border-b border-gray-200 dark:border-gray-700 pb-2">{{.Letter}}</h2>
        {{range .Terms}}
        {{template "dict-entry" .}}
        {{end}}
      </div>
      {{end}}
    </div>

    <!-- Category View -->
    <div class="hidden" data-tab-panel="dict-views-cat">
      {{range .ByCategory}}
      <div data-dict-group>
        <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-3 mt-6 border-b border-gray-200 dark:border-gray-700 pb-2">{{.Category}}</h2>
        {{range .Terms}}
        {{template "dict-entry" .}}
        {{end}}
      </div>
      {{end}}
    </div>

    <!-- Usage View -->
    <div class="hidden" data-tab-panel="dict-views-usage">
      {{range .Terms}}
      {{if .AppearsOn}}
      <div data-dict-entry="{{.Term}} {{.Abbr}}" class="mb-4 p-4 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-800 rounded-lg">
        <div class="font-medium text-gray-900 dark:text-gray-100 mb-2">{{.Term}}{{if .Abbr}} <span class="text-gray-400 dark:text-gray-500">({{.Abbr}})</span>{{end}}</div>
        <ul class="space-y-1">
          {{range .AppearsOn}}
          <li><a href="{{.URL}}" class="text-sm text-brand-600 dark:text-brand-400 hover:text-brand-800 dark:hover:text-brand-200">{{.Title}}</a></li>
          {{end}}
        </ul>
      </div>
      {{end}}
      {{end}}
    </div>
  </div>
</div>
<script src="{{.Site.BasePath}}assets/dictionary.js"></script>
{{end}}

{{define "dict-entry"}}
<div id="term-{{.Slug}}" data-dict-entry="{{.Term}} {{.Abbr}} {{.AliasText}}" class="mb-4 p-4 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-800 rounded-lg">
  <div class="flex items-start justify-between gap-4 mb-2">
    <div>
      <span class="font-semibold text-gray-900 dark:text-gray-100">{{.Term}}</span>
      {{if .Abbr}}<span class="ml-2 text-sm font-mono text-brand-600 dark:text-brand-400 bg-brand-50 dark:bg-brand-900/30 px-1.5 py-0.5 rounded">{{.Abbr}}</span>{{end}}
    </div>
    {{if .Category}}<span class="text-xs font-medium text-gray-500 dark:text-gray-400 bg-gray-100 dark:bg-gray-800 px-2 py-0.5 rounded whitespace-nowrap">{{.Category}}</span>{{end}}
  </div>
  <p class="text-sm text-gray-700 dark:text-gray-300 mb-2">{{.Definition}}</p>
  {{if .Aliases}}<div class="text-xs text-gray-400 dark:text-gray-500 mb-2">Also: {{.AliasText}}</div>{{end}}
  {{if .Etymology}}<div class="text-xs text-gray-400 dark:text-gray-500 mb-2 italic">{{.Etymology}}</div>{{end}}
  {{if .Refs}}<div class="flex flex-wrap gap-2 mt-2">{{range .Refs}}<a href="{{.URL}}" class="text-xs text-brand-600 dark:text-brand-400 hover:text-brand-800 dark:hover:text-brand-200">{{.Label}} &rarr;</a>{{end}}</div>{{end}}
</div>
{{end}}`
