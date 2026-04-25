package theme

const baseTemplate = `{{define "base"}}<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{{template "title" .}}</title>
  <meta property="og:title" content="{{template "title" .}}">
  <meta property="og:site_name" content="{{.Site.Title}}">
  {{block "meta_description" .}}{{if .Site.Description}}<meta property="og:description" content="{{.Site.Description}}">
  <meta name="description" content="{{.Site.Description}}">{{end}}{{end}}
  {{if .Site.BaseURL}}<meta property="og:url" content="{{.Site.BaseURL}}">{{end}}
  {{block "canonical" .}}{{end}}
  <meta property="og:type" content="article">
  {{if .Site.Favicon}}<link rel="icon" href="{{.Site.Favicon}}">{{end}}
  {{.Site.HeadExtra}}
  <script>
    // Apply dark mode immediately to prevent flash
    if (localStorage.getItem('theme') === 'dark' || (!localStorage.getItem('theme') && window.matchMedia('(prefers-color-scheme: dark)').matches)) {
      document.documentElement.classList.add('dark');
    }
  </script>
  <script src="https://cdn.tailwindcss.com"></script>
  <script>
    tailwind.config = {
      darkMode: 'class',
      theme: {
        extend: {
          colors: {
            brand: { 50: '#f0f5ff', 100: '#e0eaff', 200: '#c2d5ff', 400: '#6699ff', 500: '#3366cc', 600: '#2952a3', 700: '#1f3d7a', 800: '#152952', 900: '#0a1429' }
          }
        }
      }
    }
  </script>
  <style type="text/tailwindcss">
    .prose h1 { @apply text-3xl font-bold mb-6 text-gray-900 dark:text-gray-100; }
    .prose h2 { @apply text-2xl font-semibold mt-10 mb-4 text-gray-900 dark:text-gray-100; }
    .prose h3 { @apply text-xl font-semibold mt-8 mb-3 text-gray-800 dark:text-gray-200; }
    .prose h4 { @apply text-lg font-semibold mt-6 mb-2 text-gray-800 dark:text-gray-200; }
    .prose h5 { @apply text-base font-semibold mt-4 mb-2 text-gray-800 dark:text-gray-200; }
    .prose h6 { @apply text-sm font-semibold mt-4 mb-1 text-gray-700 dark:text-gray-300; }
    .prose { overflow-wrap: break-word; word-break: break-word; }
    .prose h1, .prose h2, .prose h3, .prose h4, .prose h5, .prose h6 { scroll-margin-top: 1.5rem; }
    .prose p { @apply mb-4 leading-relaxed text-gray-700 dark:text-gray-300; }
    .prose ul { @apply mb-4 ml-6 list-disc text-gray-700 dark:text-gray-300; }
    .prose ol { @apply mb-4 ml-6 list-decimal text-gray-700 dark:text-gray-300; }
    .prose li { @apply mb-1; }
    .prose .table-wrapper { @apply overflow-x-auto mb-6 -mx-4 px-4; }
    .prose table { @apply border-collapse; width: 100%; }
    .prose th { @apply text-left p-3 bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 font-semibold text-sm; white-space: nowrap; }
    .prose td { @apply p-3 border border-gray-200 dark:border-gray-700 text-sm; word-break: normal; }
    .prose code:not(pre code) { @apply bg-gray-100 dark:bg-gray-800 px-1.5 py-0.5 rounded text-sm font-mono; }
    .prose pre { @apply text-gray-100 p-4 rounded-lg mb-4 overflow-x-auto relative text-sm leading-relaxed; background: #282a36 !important; }
    .prose pre code { background: transparent !important; @apply p-0 text-inherit font-mono; }
    .prose pre code * { background: transparent !important; display: inline !important; }
    .prose a { @apply text-brand-500 dark:text-brand-400 underline hover:text-brand-700 dark:hover:text-brand-200; }
    .prose blockquote { @apply border-l-4 border-brand-200 dark:border-brand-700 pl-4 italic text-gray-600 dark:text-gray-400 my-4; }
    .prose strong { @apply font-semibold text-gray-900 dark:text-gray-100; }
    mark.search-highlight { all: unset; color: inherit; background: rgba(51, 102, 204, 0.15); border-bottom: 2px solid rgba(51, 102, 204, 0.5); border-radius: 2px; }
    :is(.dark mark.search-highlight) { color: inherit; background: rgba(102, 153, 255, 0.15); border-bottom-color: rgba(102, 153, 255, 0.4); }
    .tab-active { @apply border-brand-500 text-brand-600 dark:text-brand-400; }
    [data-tab-group] button:not(.tab-active) { @apply border-transparent text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200; }
    .prose details { @apply my-4 border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden; }
    .prose details summary { @apply cursor-pointer px-4 py-3 text-sm font-medium text-gray-900 dark:text-gray-100 bg-gray-50 dark:bg-gray-800 select-none; }
    .prose details[open] summary { @apply border-b border-gray-200 dark:border-gray-700; }
    .prose details > p { @apply px-4 pt-3 mb-0; }
    .prose details > p:last-child { @apply pb-3; }
    [data-tab-panel] > pre { @apply m-0 rounded-none; }
    .prose h2 .anchor, .prose h3 .anchor, .prose h4 .anchor, .prose h5 .anchor, .prose h6 .anchor { @apply invisible ml-2 text-gray-300 dark:text-gray-600 no-underline; }
    .prose h2:hover .anchor, .prose h3:hover .anchor, .prose h4:hover .anchor, .prose h5:hover .anchor, .prose h6:hover .anchor { @apply visible; }

    .section-num { @apply text-gray-400 dark:text-gray-500 font-normal mr-2; }
    h3 .section-num, h4 .section-num, h5 .section-num, h6 .section-num { @apply text-base; }
    .rfc-keyword { @apply font-semibold text-brand-700 dark:text-brand-400; }

    .dict-term { border-bottom: 1.5px dotted; border-color: rgba(99,102,241,0.5); cursor: help; }
    :is(.dark .dict-term) { border-color: rgba(129,140,248,0.45); }

    @media print {
      html, body { height: auto !important; overflow: visible !important; }
      body { display: block !important; }
      main { height: auto !important; overflow: visible !important; }
      header, footer, aside, #mobile-menu, #pathway-nav, #search-input, #search-results, #theme-toggle, #mobile-menu-toggle, #back-to-top, .anchor { display: none !important; }
      .print-toolbar { display: none !important; }
      body { background: white !important; color: black !important; }
      .prose a { color: inherit !important; text-decoration: underline !important; }
      .prose pre { border: 1px solid #ccc !important; }
      article { max-width: 100% !important; }
      .print-cover-page, .print-section-page { height: 100vh !important; min-height: auto !important; border: none !important; margin-bottom: 0 !important; page-break-after: always; break-after: page; }
      .print-page-break { page-break-before: always; break-before: page; }
    }
  </style>
</head>
<body class="min-h-screen lg:h-screen lg:overflow-hidden flex flex-col bg-white dark:bg-gray-950 text-gray-900 dark:text-gray-100 antialiased">
  <div id="site-chrome" class="flex-shrink-0">
  {{if .Site.Announcement}}<div id="announcement-bar" class="bg-brand-600 text-white text-sm text-center py-2 px-4">
    <span>{{.Site.Announcement}}</span>
    <button onclick="this.parentElement.remove();localStorage.setItem('announcement-dismissed','{{.Site.Announcement}}')" class="ml-3 text-brand-200 hover:text-white">&times;</button>
  </div>
  <script>if(localStorage.getItem('announcement-dismissed')==='{{.Site.Announcement}}')document.getElementById('announcement-bar').remove();</script>
  {{end}}
  <header class="border-b border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-950">
    <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
      <div class="flex items-center justify-between h-14">
        <a href="{{.Site.BasePath}}" class="text-lg font-semibold text-gray-900 dark:text-gray-100 hover:text-brand-600 dark:hover:text-brand-400">{{.Site.Title}}</a>
        <div class="flex items-center gap-4">
          <nav class="hidden md:flex items-center gap-6 text-sm">
            {{range .Site.Nav}}
            <a href="{{bp .URL}}" class="text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100">{{.Label}}</a>
            {{end}}
          </nav>
          <div class="relative hidden sm:block">
            <input id="search-input" type="text" placeholder="Search... (/)" class="w-40 md:w-48 lg:w-64 px-3 py-1.5 text-sm bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-md text-gray-900 dark:text-gray-100 placeholder-gray-400 dark:placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-brand-500 focus:border-transparent">
            <div id="search-results" class="hidden absolute top-full right-0 mt-1 w-80 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg overflow-hidden z-50 max-h-96 overflow-y-auto"></div>
          </div>
          <button id="theme-toggle" class="p-1.5 rounded-md text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800" aria-label="Toggle dark mode">
            <svg id="theme-icon-light" class="w-5 h-5 hidden" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z"/></svg>
            <svg id="theme-icon-dark" class="w-5 h-5 hidden" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z"/></svg>
          </button>
          <button id="mobile-menu-toggle" class="md:hidden p-1.5 rounded-md text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800" aria-label="Toggle menu">
            <svg id="mobile-menu-open" class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M4 6h16M4 12h16M4 18h16"/></svg>
            <svg id="mobile-menu-close" class="w-5 h-5 hidden" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"/></svg>
          </button>
        </div>
      </div>
    </div>
  </header>
  <div id="mobile-menu" class="hidden md:hidden border-b border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-950">
    <div class="max-w-7xl mx-auto px-4 py-3">
      <div class="relative sm:hidden mb-3">
        <input id="mobile-search-input" type="text" placeholder="Search..." class="w-full px-3 py-2 text-sm bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-md text-gray-900 dark:text-gray-100 placeholder-gray-400 dark:placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-brand-500 focus:border-transparent">
        <div id="mobile-search-results" class="hidden absolute top-full left-0 right-0 mt-1 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg overflow-hidden z-50 max-h-64 overflow-y-auto"></div>
      </div>
      <nav class="space-y-1">
        {{range .Site.Nav}}
        <a href="{{bp .URL}}" class="block py-2 px-3 rounded text-sm text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100 hover:bg-gray-50 dark:hover:bg-gray-800">{{.Label}}</a>
        {{end}}
      </nav>
    </div>
  </div>
  </div>
  <main class="flex-1 min-h-0 overflow-y-auto">
    {{template "content" .}}
  </main>
  <button id="back-to-top" class="hidden fixed bottom-6 right-6 p-2.5 rounded-full bg-gray-200 dark:bg-gray-800 text-gray-600 dark:text-gray-400 hover:bg-gray-300 dark:hover:bg-gray-700 shadow-lg transition-opacity z-40" aria-label="Back to top">
    <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M5 15l7-7 7 7"/></svg>
  </button>
  <footer class="flex-shrink-0 border-t border-gray-200 dark:border-gray-800 {{block "footer_class" .}}{{end}}">
    <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-10">
      <div class="flex flex-col sm:flex-row justify-between items-center gap-4 text-sm text-gray-400 dark:text-gray-500">
        <span>{{.Site.Title}}</span>
        <span>Built with <a href="https://github.com/peios/trail" class="hover:text-gray-600 dark:hover:text-gray-300">Trail</a></span>
      </div>
    </div>
  </footer>
  <script>window.__basePath='{{.Site.BasePath}}';</script>
  <script src="{{.Site.BasePath}}assets/livereload.js"></script>
  <script src="https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.min.js"></script>
  <script>
    document.querySelectorAll('.mermaid').forEach(function(el) {
      el.setAttribute('data-mermaid-src', el.textContent);
    });
    mermaid.initialize({ startOnLoad: true, theme: document.documentElement.classList.contains('dark') ? 'dark' : 'default' });
  </script>
  <script src="https://cdn.jsdelivr.net/npm/fuse.js@7.0.0/dist/fuse.min.js"></script>
  <script src="{{.Site.BasePath}}assets/pathway.js"></script>
  <script src="{{.Site.BasePath}}assets/theme.js"></script>
  <script src="{{.Site.BasePath}}assets/search.js"></script>
  <script src="{{.Site.BasePath}}assets/mobile.js"></script>
  <script src="{{.Site.BasePath}}assets/copycode.js"></script>
  <script src="{{.Site.BasePath}}assets/tabs.js"></script>
  <script src="{{.Site.BasePath}}assets/scrollspy.js"></script>
  <script src="{{.Site.BasePath}}assets/backtotop.js"></script>
  <script src="{{.Site.BasePath}}assets/highlight.js"></script>
  <script src="{{.Site.BasePath}}assets/fontsize.js"></script>
  <script src="{{.Site.BasePath}}assets/popover.js"></script>
</body>
</html>{{end}}`

const pageTemplate = `{{define "title"}}{{.Page.Title}} — {{.Site.Title}}{{end}}
{{define "meta_description"}}{{if .Page.Description}}<meta property="og:description" content="{{.Page.Description}}">
  <meta name="description" content="{{.Page.Description}}">{{else if .Site.Description}}<meta property="og:description" content="{{.Site.Description}}">
  <meta name="description" content="{{.Site.Description}}">{{end}}{{end}}
{{define "canonical"}}{{if .Site.BaseURL}}<link rel="canonical" href="{{.Site.BaseURL}}/{{.Page.Slug}}/">{{end}}{{end}}
{{define "footer_class"}}lg:hidden{{end}}
{{define "content"}}
<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8 lg:h-full lg:overflow-hidden lg:py-0">
  <div class="flex gap-8 lg:h-full lg:min-h-0">
    {{if .Category}}
    <aside class="hidden lg:block w-64 flex-shrink-0 lg:min-h-0 lg:py-8">
      <nav class="lg:h-full lg:overflow-y-auto lg:pr-2">
        <h3 class="font-semibold text-sm text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-3">{{.Category.Title}}</h3>
        <ul class="space-y-1">
          {{$currentSlug := .Page.Slug}}
          {{range .Category.Pages}}
          <li>
            <a href="{{$.Site.BasePath}}{{.Slug}}/"
               class="block py-1.5 px-3 rounded text-sm {{if eq .Slug $currentSlug}}bg-brand-50 dark:bg-brand-900 text-brand-700 dark:text-brand-400 font-medium{{else}}text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100 hover:bg-gray-50 dark:hover:bg-gray-800{{end}}">
              {{typeIcon .Type}} {{.Title}}
            </a>
          </li>
          {{end}}
        </ul>
      </nav>
    </aside>
    {{end}}
    <article class="flex-1 min-w-0 lg:min-h-0 lg:overflow-y-auto lg:py-8 overflow-x-hidden" data-scroll-root>
      <div id="page-top" class="max-w-3xl">
      <nav class="text-sm mb-4 flex flex-wrap items-center">
        <a href="{{.Site.BasePath}}" class="text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200">Home</a>
        {{if .Product}}<span class="text-gray-400 dark:text-gray-600 mx-2">/</span>
        <a href="{{$.Site.BasePath}}{{.Product.Slug}}/" class="text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200">{{.Product.Name}}</a>{{end}}
        {{if .Category}}<span class="text-gray-400 dark:text-gray-600 mx-2">/</span>
        <a href="{{catPath .Category.ProductSlug .Category.Name}}" class="text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200">{{.Category.Title}}</a>{{end}}
        <span class="text-gray-400 dark:text-gray-600 mx-2">/</span>
        <span class="text-gray-900 dark:text-gray-100">{{.Page.Title}}</span>
      </nav>
      <details class="lg:hidden mb-4 border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden not-prose">
        <summary class="cursor-pointer px-4 py-2.5 text-sm font-medium text-gray-700 dark:text-gray-300 bg-gray-50 dark:bg-gray-800 select-none">On this page</summary>
        <ul class="px-4 py-2 space-y-1">
          {{range .Page.Headings}}
          <li><a href="#{{.ID}}" class="block py-1 text-sm text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100{{headingIndent .Level}}">{{.Text}}</a></li>
          {{else}}
          <li><a href="#page-top" class="block py-1 text-sm text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100">Overview</a></li>
          {{end}}
        </ul>
      </details>
      <div class="mb-6">
        <div class="flex items-center gap-3 flex-wrap">
          {{if .Page.Type}}<span class="inline-block text-xs font-medium uppercase tracking-wide text-brand-600 dark:text-brand-400 bg-brand-50 dark:bg-brand-900 px-2 py-1 rounded">{{typeLabel .Page.Type}}</span>{{end}}
          {{if .Page.ReadingTime}}<span class="text-xs text-gray-400 dark:text-gray-500">{{.Page.ReadingTime}} min read</span>{{end}}
          {{if .Page.Updated}}<span class="text-xs text-gray-400 dark:text-gray-500">Updated {{.Page.Updated}}</span>{{end}}
        </div>
        <h1 class="text-3xl font-bold text-gray-900 dark:text-gray-100 mt-2">{{.Page.Title}}</h1>
      </div>
      <div class="prose">
        {{.Page.HTML}}
      </div>
      {{if .Page.Related}}
      <div class="mt-10 pt-6 border-t border-gray-200 dark:border-gray-800">
        <h3 class="text-sm font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-3">See also</h3>
        <ul class="space-y-1">
          {{range .Page.Related}}
          <li><a href="{{$.Site.BasePath}}{{.Slug}}/" class="text-sm text-brand-600 dark:text-brand-400 hover:text-brand-800 dark:hover:text-brand-200">{{.Title}}</a></li>
          {{end}}
        </ul>
      </div>
      {{end}}
      {{if .Site.RepoURL}}<div class="mt-8 pt-4 border-t border-gray-200 dark:border-gray-800">
        <a href="{{.Site.RepoURL}}/edit/main/content/{{.Page.Slug}}.md" class="text-sm text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300">Edit this page on GitHub</a>
      </div>{{end}}
      <div id="pathway-nav" class="hidden mt-12 pt-6 border-t border-gray-200 dark:border-gray-800">
        <div class="flex justify-between items-center">
          <a id="pathway-prev" href="#" class="text-sm text-brand-600 dark:text-brand-400 hover:text-brand-800 dark:hover:text-brand-200">&larr; <span id="pathway-prev-title"></span></a>
          <a id="pathway-next" href="#" class="text-sm text-brand-600 dark:text-brand-400 hover:text-brand-800 dark:hover:text-brand-200"><span id="pathway-next-title"></span> &rarr;</a>
        </div>
      </div>
      <div class="hidden lg:flex mt-12 pt-6 pb-8 border-t border-gray-200 dark:border-gray-800 items-center justify-between gap-4 text-sm text-gray-400 dark:text-gray-500">
        <span>{{.Site.Title}}</span>
        <span>Built with <a href="https://github.com/peios/trail" class="hover:text-gray-600 dark:hover:text-gray-300">Trail</a></span>
      </div>
      </div>
    </article>
    <aside class="hidden lg:block w-56 flex-shrink-0 lg:min-h-0 lg:py-8">
      <div class="lg:h-full lg:overflow-y-auto lg:pr-2">
        <div id="font-size-controls" class="flex items-center gap-2 mb-4 pb-4 border-b border-gray-200 dark:border-gray-800"></div>
      <nav data-outline-nav>
        <h3 class="font-semibold text-sm text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-3">On this page</h3>
        <ul class="space-y-1 text-sm">
          {{range .Page.Headings}}
          <li>
            <a href="#{{.ID}}" class="block py-1 text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100{{headingIndent .Level}}">{{.Text}}</a>
          </li>
          {{else}}
          <li>
            <a href="#page-top" class="block py-1 text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100">Overview</a>
          </li>
          {{end}}
        </ul>
      </nav>
      </div>
    </aside>
  </div>
</div>
{{end}}`

const homepageTemplate = `{{define "title"}}{{.Site.Title}}{{end}}
{{define "content"}}
<div class="bg-brand-700 dark:bg-brand-900 text-white">
  <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-10 sm:py-16">
    <h1 class="text-3xl sm:text-4xl font-bold mb-3 sm:mb-4">{{.Site.Title}}</h1>
    <p class="text-base sm:text-xl text-brand-100 max-w-2xl">{{.Site.Description}}</p>
  </div>
</div>

{{if .Site.Products}}
{{$docs := docsProducts .Site.Products}}
{{$specs := specProducts .Site.Products}}
{{if $docs}}
<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8 sm:py-12">
  <h2 class="text-xl sm:text-2xl font-bold text-gray-900 dark:text-gray-100 mb-4 sm:mb-6">Documentation</h2>
  <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4 sm:gap-6">
    {{range $docs}}
    <a href="{{$.Site.BasePath}}{{.Slug}}/" class="block p-4 sm:p-6 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-800 rounded-lg hover:border-brand-300 dark:hover:border-brand-700 hover:shadow-md transition-all">
      <h3 class="font-semibold text-gray-900 dark:text-gray-100 mb-1 sm:mb-2">{{.Name}}</h3>
      {{if .Description}}<p class="text-sm text-gray-500 dark:text-gray-400 mb-2 sm:mb-3">{{.Description}}</p>{{end}}
      <span class="text-xs text-gray-400 dark:text-gray-500">{{len .Pages}} articles</span>
    </a>
    {{end}}
  </div>
</div>
{{end}}
{{if $specs}}
<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8 sm:py-12{{if $docs}} border-t border-gray-200 dark:border-gray-800{{end}}">
  <h2 class="text-xl sm:text-2xl font-bold text-gray-900 dark:text-gray-100 mb-4 sm:mb-6">Specifications</h2>
  <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4 sm:gap-6">
    {{range $specs}}
    <a href="{{$.Site.BasePath}}{{firstPageSlug .}}/" class="block p-4 sm:p-6 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-800 rounded-lg hover:border-brand-300 dark:hover:border-brand-700 hover:shadow-md transition-all">
      {{if .SpecID}}<span class="text-xs font-mono uppercase text-gray-400 dark:text-gray-500">{{.SpecID}}</span>{{end}}
      <h3 class="font-semibold text-gray-900 dark:text-gray-100 mb-1 sm:mb-2">{{.Name}}</h3>
      {{if .Description}}<p class="text-sm text-gray-500 dark:text-gray-400 mb-2 sm:mb-3">{{.Description}}</p>{{end}}
      {{if .VersionSlug}}<span class="text-xs text-brand-600 dark:text-brand-400">{{.VersionSlug}}</span>{{end}}
      <span class="text-xs text-gray-400 dark:text-gray-500">{{len .Pages}} sections</span>
    </a>
    {{end}}
  </div>
</div>
{{end}}
{{else}}
<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8 sm:py-12">
  <h2 class="text-xl sm:text-2xl font-bold text-gray-900 dark:text-gray-100 mb-4 sm:mb-6">Browse by Topic</h2>
  <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4 sm:gap-6">
    {{range .Site.Categories}}
    <div class="p-4 sm:p-6 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-800 rounded-lg">
      <h3 class="font-semibold text-gray-900 dark:text-gray-100 mb-3">{{.Title}}</h3>
      <ul class="space-y-1">
        {{range firstN 3 .Pages}}
        <li><a href="{{$.Site.BasePath}}{{.Slug}}/" class="text-sm text-brand-600 dark:text-brand-400 hover:text-brand-800 dark:hover:text-brand-200">{{.Title}}</a></li>
        {{end}}
      </ul>
      {{if gt (len .Pages) 3}}<a href="{{catPath .ProductSlug .Name}}" class="inline-block mt-2 text-xs text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300">See all {{len .Pages}} articles &rarr;</a>{{end}}
    </div>
    {{end}}
  </div>
</div>
{{end}}
{{end}}`

const categoryTemplate = `{{define "title"}}{{.Category.Title}} — {{.Site.Title}}{{end}}
{{define "content"}}
<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-12">
  <nav class="text-sm mb-6 flex flex-wrap items-center">
    <a href="{{.Site.BasePath}}" class="text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200">Home</a>
    {{if .Product}}<span class="text-gray-400 dark:text-gray-600 mx-2">/</span>
    <a href="{{$.Site.BasePath}}{{.Product.Slug}}/" class="text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200">{{.Product.Name}}</a>{{end}}
    <span class="text-gray-400 dark:text-gray-600 mx-2">/</span>
    <span class="text-gray-900 dark:text-gray-100">{{.Category.Title}}</span>
  </nav>
  <h1 class="text-3xl font-bold text-gray-900 dark:text-gray-100 mb-8">{{.Category.Title}}</h1>
  <div class="space-y-3">
    {{range .Category.Pages}}
    <a href="{{$.Site.BasePath}}{{.Slug}}/" class="block p-4 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-800 rounded-lg hover:border-brand-300 dark:hover:border-brand-700 hover:shadow-sm transition-all">
      {{if .Type}}<span class="inline-block text-xs font-medium uppercase tracking-wide text-brand-600 dark:text-brand-400 bg-brand-50 dark:bg-brand-900 px-2 py-0.5 rounded mb-1.5">{{typeLabel .Type}}</span>{{end}}
      <div class="text-sm font-medium text-gray-900 dark:text-gray-100">{{.Title}}</div>
      {{if .Description}}<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">{{.Description}}</p>{{end}}
    </a>
    {{end}}
  </div>
</div>
{{end}}`

const printTemplate = `{{define "title"}}{{if .ProductName}}{{.ProductName}} — {{end}}{{.Site.Title}} — Complete Reference{{end}}
{{define "content"}}
<div class="max-w-5xl mx-auto px-4 sm:px-6 lg:px-8 py-10 sm:py-12">
  <div class="print-toolbar flex flex-wrap items-center gap-3 mb-8">
    <a href="{{if .ProductSlug}}{{.Site.BasePath}}{{.ProductSlug}}/{{else}}{{.Site.BasePath}}{{end}}" class="inline-flex items-center justify-center rounded-md border border-gray-200 dark:border-gray-700 px-4 py-2 text-sm text-gray-600 dark:text-gray-300 hover:border-gray-300 dark:hover:border-gray-600 hover:text-gray-900 dark:hover:text-gray-100">
      {{if eq .ProductKind "spec"}}Back to specification{{else if .ProductSlug}}Back to docs{{else}}Back to site{{end}}
    </a>
    <button type="button" onclick="window.print()" class="inline-flex items-center justify-center rounded-md bg-brand-600 px-4 py-2 text-sm font-medium text-white hover:bg-brand-700">
      Print / Save PDF
    </button>
  </div>
  {{if eq .ProductKind "spec"}}
  <div class="print-cover-page flex flex-col justify-center rounded-2xl border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 px-8 py-12 mb-12">
    <p class="text-xs font-medium uppercase tracking-[0.3em] text-gray-400 dark:text-gray-500 mb-4">{{if .SpecID}}<span class="font-mono">{{.SpecID}}</span> — {{end}}Specification</p>
    <h1 class="text-4xl sm:text-5xl font-black text-gray-900 dark:text-gray-100 mb-4">{{.ProductName}}</h1>
    {{if .ProductDescription}}<p class="max-w-3xl text-base sm:text-lg text-gray-600 dark:text-gray-300 mb-8">{{.ProductDescription}}</p>{{end}}
    <div class="flex flex-wrap items-center gap-3 text-sm">
      {{if .VersionSlug}}<span class="inline-flex items-center rounded-full border border-gray-200 dark:border-gray-700 px-3 py-1 font-mono text-gray-700 dark:text-gray-300">{{.VersionSlug}}</span>{{end}}
      {{if gt .RevisionCount 1}}<span class="text-sm text-gray-400 dark:text-gray-500">revision {{.RevisionCount}}</span>{{end}}
      {{if eq .VersionStatus "draft"}}<span class="inline-flex items-center rounded-full bg-yellow-100 dark:bg-yellow-900 px-3 py-1 font-medium text-yellow-700 dark:text-yellow-300">Draft</span>{{end}}
      {{if eq .VersionStatus "final"}}<span class="inline-flex items-center rounded-full bg-green-100 dark:bg-green-900 px-3 py-1 font-medium text-green-700 dark:text-green-300">Final</span>{{end}}
      {{if eq .VersionStatus "superseded"}}<span class="inline-flex items-center rounded-full bg-gray-200 dark:bg-gray-700 px-3 py-1 font-medium text-gray-600 dark:text-gray-300">Superseded</span>{{end}}
      {{if eq .VersionStatus "withdrawn"}}<span class="inline-flex items-center rounded-full bg-red-100 dark:bg-red-900 px-3 py-1 font-medium text-red-700 dark:text-red-300">Withdrawn</span>{{end}}
      {{if .VersionDate}}<span class="text-gray-500 dark:text-gray-400">{{.VersionDate}}</span>{{end}}
    </div>
  </div>
  <div class="mb-12">
    <h2 class="text-lg font-semibold uppercase tracking-wide text-gray-500 dark:text-gray-400 mb-4">Contents</h2>
    <div class="space-y-6">
      {{range .Sections}}
      <section>
        <h3 class="text-sm font-semibold text-gray-900 dark:text-gray-100 mb-2">{{if .SectionNum}}<span class="text-gray-400 dark:text-gray-500 mr-2">{{.SectionNum}}</span>{{end}}{{.Name}}</h3>
        <ul class="space-y-1">
          {{range .Pages}}
          <li>
            <a href="#{{.AnchorID}}" class="text-sm text-brand-600 dark:text-brand-400 hover:text-brand-800 dark:hover:text-brand-200">
              {{if .SectionNum}}<span class="text-gray-400 dark:text-gray-500 mr-2">{{.SectionNum}}</span>{{end}}{{.Title}}
            </a>
          </li>
          {{end}}
        </ul>
      </section>
      {{end}}
    </div>
  </div>
  {{range $idx, $section := .Sections}}
  <section class="{{if gt $idx 0}}print-page-break {{end}}mb-16">
    <div class="print-section-page flex items-center border-b-4 border-brand-500 pb-8 mb-10">
      <div>
        <p class="text-xs font-medium uppercase tracking-[0.3em] text-gray-400 dark:text-gray-500 mb-2">Section</p>
        <h2 class="text-4xl font-black text-brand-700 dark:text-brand-300 m-0">{{if $section.SectionNum}}{{$section.SectionNum}} {{end}}{{$section.Name}}</h2>
      </div>
    </div>
    {{range $section.Pages}}
    <article id="{{.AnchorID}}" class="mb-16 pb-16 border-b border-gray-200 dark:border-gray-800 last:border-0">
      <div class="flex flex-wrap items-center gap-3 mb-3">
        {{if .SectionNum}}<span class="inline-flex items-center rounded-full border border-gray-200 dark:border-gray-700 px-3 py-1 text-xs font-mono text-gray-600 dark:text-gray-300">§{{.SectionNum}}</span>{{end}}
        {{if .Type}}<span class="inline-block text-xs font-medium uppercase tracking-wide text-brand-600 dark:text-brand-400 bg-brand-50 dark:bg-brand-900 px-2 py-0.5 rounded">{{typeLabel .Type}}</span>{{end}}
        {{if .CategoryTitle}}<span class="text-xs text-gray-400 dark:text-gray-500">{{if .CategorySectionNum}}{{.CategorySectionNum}} {{end}}{{.CategoryTitle}}</span>{{end}}
      </div>
      <h3 class="text-2xl font-bold text-gray-900 dark:text-gray-100 mb-4">{{.Title}}</h3>
      <div class="prose spec-prose">
        {{.HTML}}
      </div>
    </article>
    {{end}}
  </section>
  {{end}}
  {{else}}
  <h1 class="text-4xl font-bold text-gray-900 dark:text-gray-100 mb-2">{{if .ProductName}}{{.ProductName}}{{else}}{{.Site.Title}}{{end}}</h1>
  <p class="text-gray-500 dark:text-gray-400 mb-12">Complete reference — all pages in one document</p>
  {{range .Pages}}
  <article id="{{.AnchorID}}" class="mb-16 pb-16 border-b border-gray-200 dark:border-gray-800 last:border-0">
    <div class="flex items-center gap-3 mb-2">
      {{if .Type}}<span class="inline-block text-xs font-medium uppercase tracking-wide text-brand-600 dark:text-brand-400 bg-brand-50 dark:bg-brand-900 px-2 py-0.5 rounded">{{typeLabel .Type}}</span>{{end}}
      <span class="text-xs text-gray-400 dark:text-gray-500">{{.CategoryTitle}}</span>
    </div>
    <h2 class="text-2xl font-bold text-gray-900 dark:text-gray-100 mb-4">{{.Title}}</h2>
    <div class="prose">
      {{.HTML}}
    </div>
  </article>
  {{end}}
  {{end}}
</div>
{{end}}`

const printGlobalTemplate = `{{define "title"}}{{.Site.Title}} — Complete Reference{{end}}
{{define "content"}}
<div class="max-w-4xl mx-auto px-4 sm:px-6 lg:px-8 py-12">
  <div class="print-cover-page flex flex-col items-center justify-center min-h-screen mb-10">
    <h1 class="text-6xl font-black text-gray-900 dark:text-gray-100 mb-4 text-center">{{.Site.Title}}</h1>
    {{if .Site.Description}}<p class="text-xl text-gray-500 dark:text-gray-400 text-center max-w-2xl">{{.Site.Description}}</p>{{end}}
    <p class="text-sm text-gray-400 dark:text-gray-500 mt-8">Complete reference — all products in one document</p>
  </div>
  {{range .Sections}}
  <div class="mb-20">
    <div class="print-section-page flex items-center justify-center min-h-screen border-b-4 border-brand-500 mb-10">
      <h2 class="text-5xl font-black text-brand-700 dark:text-brand-300 m-0 text-center">{{.Name}}</h2>
    </div>
    {{range .Pages}}
    <article class="mb-16 pb-16 border-b border-gray-200 dark:border-gray-800 last:border-0">
      <div class="flex items-center gap-3 mb-2">
        {{if .Type}}<span class="inline-block text-xs font-medium uppercase tracking-wide text-brand-600 dark:text-brand-400 bg-brand-50 dark:bg-brand-900 px-2 py-0.5 rounded">{{typeLabel .Type}}</span>{{end}}
        <span class="text-xs text-gray-400 dark:text-gray-500">{{.CategoryTitle}}</span>
      </div>
      <h3 class="text-2xl font-bold text-gray-900 dark:text-gray-100 mb-4">{{.Title}}</h3>
      <div class="prose">
        {{.HTML}}
      </div>
    </article>
    {{end}}
  </div>
  {{end}}
</div>
{{end}}`

const productPageTemplate = `{{define "title"}}{{.Product.Name}} — {{.Site.Title}}{{end}}
{{define "content"}}
<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8 sm:py-12">
  <nav class="text-sm mb-4 sm:mb-6 flex flex-wrap items-center">
    <a href="{{.Site.BasePath}}" class="text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200">Home</a>
    <span class="text-gray-400 dark:text-gray-600 mx-2">/</span>
    <span class="text-gray-900 dark:text-gray-100">{{.Product.Name}}</span>
  </nav>
  <h1 class="text-2xl sm:text-3xl font-bold text-gray-900 dark:text-gray-100 mb-2">{{.Product.Name}}</h1>
  {{if .Product.Description}}<p class="text-sm sm:text-base text-gray-500 dark:text-gray-400 mb-6 sm:mb-8">{{.Product.Description}}</p>{{end}}

  {{$featured := featuredPathways .Product.Pathways}}
  {{if $featured}}
  <div class="mb-8 sm:mb-12">
    <div class="flex items-center justify-between mb-3 sm:mb-4">
      <h2 class="text-lg sm:text-xl font-bold text-gray-900 dark:text-gray-100">Learning Pathways</h2>
      {{if gt (len .Product.Pathways) (len $featured)}}<a href="{{$.Site.BasePath}}{{.Product.Slug}}/pathways/" class="text-sm text-brand-600 dark:text-brand-400 hover:text-brand-800 dark:hover:text-brand-200">View all &rarr;</a>{{end}}
    </div>
    <div class="flex gap-3 sm:gap-4 overflow-x-auto pb-2 -mx-4 px-4 sm:mx-0 sm:px-0">
      {{range $featured}}
      <a href="{{pathwayURL .Product (index .Pages 0) .Slug}}" class="flex-shrink-0 w-64 sm:w-72 p-3 sm:p-4 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-800 rounded-lg hover:border-brand-300 dark:hover:border-brand-700 hover:shadow-md transition-all">
        <h3 class="font-semibold text-sm text-gray-900 dark:text-gray-100 mb-1">{{.Name}}</h3>
        <p class="text-xs text-gray-500 dark:text-gray-400 mb-2">{{.Description}}</p>
        <span class="text-xs text-gray-400 dark:text-gray-500">{{len .Pages}} articles</span>
      </a>
      {{end}}
    </div>
  </div>
  {{end}}

  <h2 class="text-lg sm:text-xl font-bold text-gray-900 dark:text-gray-100 mb-3 sm:mb-4">Browse by Topic</h2>
  <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4 sm:gap-6">
    {{range .Product.Categories}}
    <div class="p-4 sm:p-6 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-800 rounded-lg">
      <h3 class="font-semibold text-gray-900 dark:text-gray-100 mb-3">{{.Title}}</h3>
      <ul class="space-y-1">
        {{range firstN 3 .Pages}}
        <li><a href="{{$.Site.BasePath}}{{.Slug}}/" class="text-sm text-brand-600 dark:text-brand-400 hover:text-brand-800 dark:hover:text-brand-200">{{.Title}}</a></li>
        {{end}}
      </ul>
      {{if gt (len .Pages) 3}}<a href="{{catPath .ProductSlug .Name}}" class="inline-block mt-2 text-xs text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300">See all {{len .Pages}} articles &rarr;</a>{{end}}
    </div>
    {{end}}
  </div>
</div>
{{end}}`

const pathwaysPageTemplate = `{{define "title"}}Learning Pathways — {{.Site.Title}}{{end}}
{{define "content"}}
<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-12">
  <nav class="text-sm mb-6">
    <a href="{{.Site.BasePath}}" class="text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200">Home</a>
    <span class="text-gray-400 dark:text-gray-600 mx-2">/</span>
    <span class="text-gray-900 dark:text-gray-100">Learning Pathways</span>
  </nav>
  <h1 class="text-3xl font-bold text-gray-900 dark:text-gray-100 mb-8">Learning Pathways</h1>
  <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
    {{range .Site.Pathways}}
    <a href="{{pathwayURL .Product (index .Pages 0) .Slug}}" class="block p-6 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-800 rounded-lg hover:border-brand-300 dark:hover:border-brand-700 hover:shadow-md transition-all">
      <h3 class="font-semibold text-gray-900 dark:text-gray-100 mb-2">{{.Name}}</h3>
      <p class="text-sm text-gray-600 dark:text-gray-400 mb-3">{{.Description}}</p>
      <span class="text-xs text-gray-400 dark:text-gray-500">{{len .Pages}} articles</span>
    </a>
    {{end}}
  </div>
</div>
{{end}}`

const productPathwaysPageTemplate = `{{define "title"}}Learning Pathways — {{.Product.Name}} — {{.Site.Title}}{{end}}
{{define "content"}}
<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-12">
  <nav class="text-sm mb-6">
    <a href="{{.Site.BasePath}}" class="text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200">Home</a>
    <span class="text-gray-400 dark:text-gray-600 mx-2">/</span>
    <a href="{{$.Site.BasePath}}{{.Product.Slug}}/" class="text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200">{{.Product.Name}}</a>
    <span class="text-gray-400 dark:text-gray-600 mx-2">/</span>
    <span class="text-gray-900 dark:text-gray-100">Learning Pathways</span>
  </nav>
  <h1 class="text-3xl font-bold text-gray-900 dark:text-gray-100 mb-8">Learning Pathways</h1>
  <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
    {{range .Product.Pathways}}
    <a href="{{pathwayURL .Product (index .Pages 0) .Slug}}" class="block p-6 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-800 rounded-lg hover:border-brand-300 dark:hover:border-brand-700 hover:shadow-md transition-all">
      <h3 class="font-semibold text-gray-900 dark:text-gray-100 mb-2">{{.Name}}</h3>
      <p class="text-sm text-gray-600 dark:text-gray-400 mb-3">{{.Description}}</p>
      <span class="text-xs text-gray-400 dark:text-gray-500">{{len .Pages}} articles</span>
    </a>
    {{end}}
  </div>
</div>
{{end}}`

const specPageTemplate = `{{define "title"}}{{.Page.Title}} — {{if .Product.SpecID}}{{.Product.SpecID}} {{end}}{{.Product.Name}} — {{.Site.Title}}{{end}}
{{define "meta_description"}}{{if .Page.Description}}<meta property="og:description" content="{{.Page.Description}}">
  <meta name="description" content="{{.Page.Description}}">{{else if .Site.Description}}<meta property="og:description" content="{{.Site.Description}}">
  <meta name="description" content="{{.Site.Description}}">{{end}}{{end}}
{{define "canonical"}}{{if .Site.BaseURL}}<link rel="canonical" href="{{.Site.BaseURL}}/{{.Page.Slug}}/">{{end}}{{end}}
{{define "footer_class"}}lg:hidden{{end}}
{{define "content"}}
<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8 lg:h-full lg:overflow-hidden lg:py-0">
  <div class="flex gap-8 lg:h-full lg:min-h-0">
    {{if .Product}}
    <aside class="hidden lg:block w-64 flex-shrink-0 lg:min-h-0 lg:py-8">
      <nav class="lg:h-full lg:overflow-y-auto lg:pr-2">
        {{if .Product.SpecID}}<div class="mb-3 pb-3 border-b border-gray-200 dark:border-gray-800">
          <span class="text-sm font-mono font-semibold uppercase text-gray-700 dark:text-gray-300">{{.Product.SpecID}}</span>
        </div>{{end}}
        {{if .Product.VersionSlug}}<div class="mb-4 pb-3 border-b border-gray-200 dark:border-gray-800">
          <span class="text-xs font-medium uppercase tracking-wide text-gray-400 dark:text-gray-500">Version</span>
          <div class="mt-1">
            <select id="version-switcher" class="w-full text-sm bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded px-2 py-1 text-gray-700 dark:text-gray-300" onchange="var v=this.value,c='{{.Product.VersionSlug}}';window.location.pathname=window.location.pathname.replace('/'+c+'/','/'+v+'/');">
              {{range .Product.Versions}}
              <option value="{{.Name}}"{{if eq .Name $.Product.VersionSlug}} selected{{end}}>{{.Name}}{{if eq .Status "draft"}} (draft){{else if eq .Status "superseded"}} (superseded){{else if eq .Status "withdrawn"}} (withdrawn){{end}}</option>
              {{end}}
            </select>
          </div>
        </div>{{end}}
        <div class="mb-4 pb-3 border-b border-gray-200 dark:border-gray-800">
          <a href="{{$.Site.BasePath}}{{$.Product.Slug}}/print/" target="_blank" rel="noopener" class="inline-flex w-full items-center justify-center rounded-md bg-brand-600 px-3 py-2 text-sm font-medium text-white hover:bg-brand-700">
            Print Full Specification
          </a>
        </div>
        {{$currentSlug := .Page.Slug}}
        {{range .Product.Categories}}
        <div class="mb-4">
          <h3 class="font-semibold text-xs text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-2">
            <span class="text-gray-400 dark:text-gray-500 font-normal">{{.SectionNum}}</span> {{.Title}}
          </h3>
          <ul class="space-y-0.5">
            {{range .Pages}}
            <li>
              <a href="{{$.Site.BasePath}}{{.Slug}}/"
                 class="block py-1 px-3 rounded text-sm {{if eq .Slug $currentSlug}}bg-brand-50 dark:bg-brand-900 text-brand-700 dark:text-brand-400 font-medium{{else}}text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100 hover:bg-gray-50 dark:hover:bg-gray-800{{end}}">
                <span class="text-gray-400 dark:text-gray-500 text-xs mr-1">{{.SectionNum}}</span> {{.Title}}
              </a>
            </li>
            {{end}}
          </ul>
        </div>
        {{end}}
      </nav>
    </aside>
    {{end}}
    <article class="flex-1 min-w-0 lg:min-h-0 lg:overflow-y-auto lg:py-8 overflow-x-hidden" data-scroll-root>
      <div id="page-top" class="max-w-3xl">
      <nav class="text-sm mb-4 flex flex-wrap items-center">
        <a href="{{.Site.BasePath}}" class="text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200">Home</a>
        {{if .Product}}<span class="text-gray-400 dark:text-gray-600 mx-2">/</span>
        <a href="{{$.Site.BasePath}}{{.Product.Slug}}/" class="text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200">{{.Product.Name}}</a>{{end}}
        {{if .Category}}<span class="text-gray-400 dark:text-gray-600 mx-2">/</span>
        <span class="text-gray-500 dark:text-gray-400">{{.Category.Title}}</span>{{end}}
      </nav>
      <details class="lg:hidden mb-4 border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden not-prose">
        <summary class="cursor-pointer px-4 py-2.5 text-sm font-medium text-gray-700 dark:text-gray-300 bg-gray-50 dark:bg-gray-800 select-none">On this page</summary>
        <ul class="px-4 py-2 space-y-1">
          {{range .Page.Headings}}
          <li><a href="#{{.ID}}" class="block py-1 text-sm text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100{{headingIndent .Level}}">{{if .SectionNum}}<span class="text-gray-400 dark:text-gray-500 text-xs">§{{.SectionNum}}</span> {{end}}{{.Text}}</a></li>
          {{else}}
          <li><a href="#page-top" class="block py-1 text-sm text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100">Overview</a></li>
          {{end}}
        </ul>
      </details>
      <div class="lg:hidden mb-4">
        <a href="{{$.Site.BasePath}}{{$.Product.Slug}}/print/" target="_blank" rel="noopener" class="inline-flex w-full items-center justify-center rounded-md bg-brand-600 px-4 py-2.5 text-sm font-medium text-white hover:bg-brand-700">
          Print Full Specification
        </a>
      </div>
      {{if .Page.IsSpecCover}}
      <div class="border border-gray-200 dark:border-gray-800 rounded-lg p-6 mb-8 bg-gray-50 dark:bg-gray-900 relative">
        <p class="text-xs font-medium uppercase tracking-widest text-gray-400 dark:text-gray-500 mb-2">{{if .Product.SpecID}}<span class="font-mono">{{.Product.SpecID}}</span> — {{end}}Specification</p>
        <h1 class="text-2xl sm:text-3xl font-bold text-gray-900 dark:text-gray-100 m-0">{{.Product.Name}}</h1>
        {{if .Product.Description}}<p class="text-sm text-gray-500 dark:text-gray-400 mt-2 pr-28">{{.Product.Description}}</p>{{end}}
        {{if .Product.VersionSlug}}
        <div class="absolute top-6 right-6 text-right">
          <span class="text-sm font-mono text-gray-700 dark:text-gray-300">{{.Product.VersionSlug}}</span>{{if gt (len .Product.Versions) 1}}<span class="text-xs text-gray-400 dark:text-gray-500 ml-1">(rev {{len .Product.Versions}})</span>{{end}}
          {{range .Product.Versions}}{{if eq .Name $.Product.VersionSlug}}
          <div class="mt-1">
            {{if eq .Status "draft"}}<span class="inline-block text-xs font-medium px-2 py-0.5 rounded bg-yellow-100 dark:bg-yellow-900 text-yellow-700 dark:text-yellow-300">Draft</span>
            {{else if eq .Status "final"}}<span class="inline-block text-xs font-medium px-2 py-0.5 rounded bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-300">Final</span>
            {{else if eq .Status "superseded"}}<span class="inline-block text-xs font-medium px-2 py-0.5 rounded bg-gray-200 dark:bg-gray-700 text-gray-500 dark:text-gray-400">Superseded</span>
            {{else if eq .Status "withdrawn"}}<span class="inline-block text-xs font-medium px-2 py-0.5 rounded bg-red-100 dark:bg-red-900 text-red-600 dark:text-red-300">Withdrawn</span>
            {{end}}
          </div>
          {{if .Date}}<div class="text-xs text-gray-400 dark:text-gray-500 mt-1">{{.Date}}</div>{{end}}
          {{end}}{{end}}
        </div>
        {{end}}
      </div>
      {{end}}
      <div class="mb-6">
        {{if .Page.SectionNum}}<span class="text-sm text-gray-400 dark:text-gray-500 font-mono">§{{.Page.SectionNum}}</span>{{end}}
        <h1 class="text-2xl font-bold text-gray-900 dark:text-gray-100">{{.Page.Title}}</h1>
      </div>
      <div class="prose spec-prose">
        {{.Page.HTML}}
      </div>
      {{if .Site.RepoURL}}<div class="mt-8 pt-4 border-t border-gray-200 dark:border-gray-800">
        <a href="{{.Site.RepoURL}}/edit/main/content/{{.Page.Slug}}.md" class="text-sm text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300">Edit this page on GitHub</a>
      </div>{{end}}
      <div class="hidden lg:flex mt-12 pt-6 pb-8 border-t border-gray-200 dark:border-gray-800 items-center justify-between gap-4 text-sm text-gray-400 dark:text-gray-500">
        <span>{{.Site.Title}}</span>
        <span>Built with <a href="https://github.com/peios/trail" class="hover:text-gray-600 dark:hover:text-gray-300">Trail</a></span>
      </div>
      </div>
    </article>
    <aside class="hidden lg:block w-56 flex-shrink-0 lg:min-h-0 lg:py-8">
      <div class="lg:h-full lg:overflow-y-auto lg:pr-2">
      <nav data-outline-nav>
        <h3 class="font-semibold text-sm text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-3">On this page</h3>
        <ul class="space-y-1 text-sm">
          {{range .Page.Headings}}
          <li>
            <a href="#{{.ID}}" class="block py-1 text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100{{headingIndent .Level}}">{{if .SectionNum}}<span class="text-gray-400 dark:text-gray-500 text-xs">§{{.SectionNum}}</span> {{end}}{{.Text}}</a>
          </li>
          {{else}}
          <li>
            <a href="#page-top" class="block py-1 text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100">Overview</a>
          </li>
          {{end}}
        </ul>
      </nav>
      </div>
    </aside>
  </div>
</div>
{{end}}`

const specProductPageTemplate = `{{define "title"}}{{if .Product.SpecID}}{{.Product.SpecID}} {{end}}{{.Product.Name}} — {{.Site.Title}}{{end}}
{{define "content"}}
<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8 sm:py-12">
  <nav class="text-sm mb-4 sm:mb-6 flex flex-wrap items-center">
    <a href="{{.Site.BasePath}}" class="text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200">Home</a>
    <span class="text-gray-400 dark:text-gray-600 mx-2">/</span>
    <span class="text-gray-900 dark:text-gray-100">{{.Product.Name}}</span>
  </nav>
  <div class="border border-gray-200 dark:border-gray-800 rounded-lg p-6 sm:p-10 mb-8 sm:mb-12 bg-gray-50 dark:bg-gray-900">
    <div class="flex items-start justify-between flex-wrap gap-4">
      <div>
        <p class="text-xs font-medium uppercase tracking-widest text-gray-400 dark:text-gray-500 mb-2">{{if .Product.SpecID}}<span class="font-mono">{{.Product.SpecID}}</span> — {{end}}Specification</p>
        <h1 class="text-2xl sm:text-3xl font-bold text-gray-900 dark:text-gray-100">{{.Product.Name}}</h1>
        {{if .Product.Description}}<p class="text-sm sm:text-base text-gray-500 dark:text-gray-400 mt-2 max-w-2xl">{{.Product.Description}}</p>{{end}}
      </div>
      {{if .Product.VersionSlug}}
      <div class="text-right">
        <span class="text-sm font-mono text-gray-700 dark:text-gray-300">{{.Product.VersionSlug}}</span>{{if gt (len .Product.Versions) 1}}<span class="text-xs text-gray-400 dark:text-gray-500 ml-1">(rev {{len .Product.Versions}})</span>{{end}}
        {{range .Product.Versions}}{{if eq .Name $.Product.VersionSlug}}
        <div class="mt-1">
          {{if eq .Status "draft"}}<span class="inline-block text-xs font-medium px-2 py-0.5 rounded bg-yellow-100 dark:bg-yellow-900 text-yellow-700 dark:text-yellow-300">Draft</span>
          {{else if eq .Status "final"}}<span class="inline-block text-xs font-medium px-2 py-0.5 rounded bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-300">Final</span>
          {{else if eq .Status "superseded"}}<span class="inline-block text-xs font-medium px-2 py-0.5 rounded bg-gray-200 dark:bg-gray-700 text-gray-500 dark:text-gray-400">Superseded</span>
          {{end}}
        </div>
        {{if .Date}}<div class="text-xs text-gray-400 dark:text-gray-500 mt-1">{{.Date}}</div>{{end}}
        {{end}}{{end}}
      </div>
      {{end}}
    </div>
  </div>

  <h2 class="text-lg sm:text-xl font-bold text-gray-900 dark:text-gray-100 mb-3 sm:mb-4">Table of Contents</h2>
  <div class="space-y-3">
    {{range .Product.Categories}}
    <div class="p-4 sm:p-6 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-800 rounded-lg">
      <h3 class="font-semibold text-gray-900 dark:text-gray-100 mb-3"><span class="text-gray-400 dark:text-gray-500 font-normal mr-2">{{.SectionNum}}</span>{{.Title}}</h3>
      <ul class="space-y-1">
        {{range .Pages}}
        <li><a href="{{$.Site.BasePath}}{{.Slug}}/" class="text-sm text-brand-600 dark:text-brand-400 hover:text-brand-800 dark:hover:text-brand-200"><span class="text-gray-400 dark:text-gray-500 text-xs mr-1">{{.SectionNum}}</span> {{.Title}}</a></li>
        {{end}}
      </ul>
    </div>
    {{end}}
  </div>
</div>
{{end}}`

const notFoundTemplate = `{{define "title"}}Page Not Found — {{.Site.Title}}{{end}}
{{define "content"}}
<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-24 text-center">
  <h1 class="text-6xl font-bold text-gray-300 dark:text-gray-700 mb-4">404</h1>
  <p class="text-xl text-gray-600 dark:text-gray-400 mb-8">This page doesn't exist.</p>
  <a href="{{.Site.BasePath}}" class="text-brand-600 dark:text-brand-400 hover:text-brand-800 dark:hover:text-brand-200 font-medium">Back to home</a>
</div>
{{end}}`
