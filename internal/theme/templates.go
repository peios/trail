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
    .prose p { @apply mb-4 leading-relaxed text-gray-700 dark:text-gray-300; }
    .prose ul { @apply mb-4 ml-6 list-disc text-gray-700 dark:text-gray-300; }
    .prose ol { @apply mb-4 ml-6 list-decimal text-gray-700 dark:text-gray-300; }
    .prose li { @apply mb-1; }
    .prose table { @apply w-full mb-6 border-collapse; }
    .prose th { @apply text-left p-3 bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 font-semibold text-sm; }
    .prose td { @apply p-3 border border-gray-200 dark:border-gray-700 text-sm; }
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
    .prose h2 .anchor, .prose h3 .anchor { @apply invisible ml-2 text-gray-300 dark:text-gray-600 no-underline; }
    .prose h2:hover .anchor, .prose h3:hover .anchor { @apply visible; }

    @media print {
      header, footer, aside, #mobile-menu, #pathway-nav, #search-input, #search-results, #theme-toggle, #mobile-menu-toggle, .anchor { display: none !important; }
      body { background: white !important; color: black !important; }
      .prose a { color: inherit !important; text-decoration: underline !important; }
      .prose pre { border: 1px solid #ccc !important; }
      article { max-width: 100% !important; }
    }
  </style>
</head>
<body class="bg-white dark:bg-gray-950 text-gray-900 dark:text-gray-100 antialiased">
  {{if .Site.Announcement}}<div id="announcement-bar" class="bg-brand-600 text-white text-sm text-center py-2 px-4">
    <span>{{.Site.Announcement}}</span>
    <button onclick="this.parentElement.remove();localStorage.setItem('announcement-dismissed','{{.Site.Announcement}}')" class="ml-3 text-brand-200 hover:text-white">&times;</button>
  </div>
  <script>if(localStorage.getItem('announcement-dismissed')==='{{.Site.Announcement}}')document.getElementById('announcement-bar').remove();</script>
  {{end}}
  <header class="border-b border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-950">
    <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
      <div class="flex items-center justify-between h-14">
        <a href="/" class="text-lg font-semibold text-gray-900 dark:text-gray-100 hover:text-brand-600 dark:hover:text-brand-400">{{.Site.Title}}</a>
        <div class="flex items-center gap-4">
          <nav class="hidden md:flex items-center gap-6 text-sm">
            {{range .Site.Nav}}
            <a href="{{.URL}}" class="text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100">{{.Label}}</a>
            {{end}}
          </nav>
          <div class="relative">
            <input id="search-input" type="text" placeholder="Search... (/)" class="w-48 lg:w-64 px-3 py-1.5 text-sm bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-md text-gray-900 dark:text-gray-100 placeholder-gray-400 dark:placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-brand-500 focus:border-transparent">
            <div id="search-results" class="hidden absolute top-full left-0 right-0 mt-1 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg overflow-hidden z-50 max-h-96 overflow-y-auto"></div>
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
    <nav class="max-w-7xl mx-auto px-4 py-3 space-y-1">
      {{range .Site.Nav}}
      <a href="{{.URL}}" class="block py-2 px-3 rounded text-sm text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100 hover:bg-gray-50 dark:hover:bg-gray-800">{{.Label}}</a>
      {{end}}
    </nav>
  </div>
  <main>
    {{template "content" .}}
  </main>
  <button id="back-to-top" class="hidden fixed bottom-6 right-6 p-2.5 rounded-full bg-gray-200 dark:bg-gray-800 text-gray-600 dark:text-gray-400 hover:bg-gray-300 dark:hover:bg-gray-700 shadow-lg transition-opacity z-40" aria-label="Back to top">
    <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M5 15l7-7 7 7"/></svg>
  </button>
  <footer class="border-t border-gray-200 dark:border-gray-800 mt-16">
    <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
      <div class="flex flex-col sm:flex-row justify-between items-center gap-4 text-sm text-gray-400 dark:text-gray-500">
        <span>{{.Site.Title}}</span>
        <span>Built with <a href="https://github.com/peios/trail" class="hover:text-gray-600 dark:hover:text-gray-300">Trail</a></span>
      </div>
    </div>
  </footer>
  <script src="/assets/livereload.js"></script>
  <script src="https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.min.js"></script>
  <script>
    document.querySelectorAll('.mermaid').forEach(function(el) {
      el.setAttribute('data-mermaid-src', el.textContent);
    });
    mermaid.initialize({ startOnLoad: true, theme: document.documentElement.classList.contains('dark') ? 'dark' : 'default' });
  </script>
  <script src="https://cdn.jsdelivr.net/npm/fuse.js@7.0.0/dist/fuse.min.js"></script>
  <script src="/assets/pathway.js"></script>
  <script src="/assets/theme.js"></script>
  <script src="/assets/search.js"></script>
  <script src="/assets/mobile.js"></script>
  <script src="/assets/copycode.js"></script>
  <script src="/assets/tabs.js"></script>
  <script src="/assets/scrollspy.js"></script>
  <script src="/assets/backtotop.js"></script>
  <script src="/assets/highlight.js"></script>
  <script src="/assets/fontsize.js"></script>
</body>
</html>{{end}}`

const pageTemplate = `{{define "title"}}{{.Page.Title}} — {{.Site.Title}}{{end}}
{{define "meta_description"}}{{if .Page.Description}}<meta property="og:description" content="{{.Page.Description}}">
  <meta name="description" content="{{.Page.Description}}">{{else if .Site.Description}}<meta property="og:description" content="{{.Site.Description}}">
  <meta name="description" content="{{.Site.Description}}">{{end}}{{end}}
{{define "canonical"}}{{if .Site.BaseURL}}<link rel="canonical" href="{{.Site.BaseURL}}/{{.Page.Slug}}/">{{end}}{{end}}
{{define "content"}}
<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
  <div class="flex gap-8">
    {{if .Category}}
    <aside class="hidden lg:block w-64 flex-shrink-0">
      <nav class="sticky top-8">
        <h3 class="font-semibold text-sm text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-3">{{.Category.Title}}</h3>
        <ul class="space-y-1">
          {{$currentSlug := .Page.Slug}}
          {{range .Category.Pages}}
          <li>
            <a href="/{{.Slug}}/"
               class="block py-1.5 px-3 rounded text-sm {{if eq .Slug $currentSlug}}bg-brand-50 dark:bg-brand-900 text-brand-700 dark:text-brand-400 font-medium{{else}}text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100 hover:bg-gray-50 dark:hover:bg-gray-800{{end}}">
              {{typeIcon .Type}} {{.Title}}
            </a>
          </li>
          {{end}}
        </ul>
      </nav>
    </aside>
    {{end}}
    <article class="flex-1 min-w-0 max-w-3xl">
      <nav class="text-sm mb-4">
        <a href="/" class="text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200">Home</a>
        {{if .Category}}<span class="text-gray-400 dark:text-gray-600 mx-2">/</span>
        <a href="/{{.Category.Name}}/" class="text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200">{{.Category.Title}}</a>{{end}}
        <span class="text-gray-400 dark:text-gray-600 mx-2">/</span>
        <span class="text-gray-900 dark:text-gray-100">{{.Page.Title}}</span>
      </nav>
      {{if .Page.Headings}}
      <details class="xl:hidden mb-4 border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden not-prose">
        <summary class="cursor-pointer px-4 py-2.5 text-sm font-medium text-gray-700 dark:text-gray-300 bg-gray-50 dark:bg-gray-800 select-none">On this page</summary>
        <ul class="px-4 py-2 space-y-1">
          {{range .Page.Headings}}
          <li><a href="#{{.ID}}" class="block py-1 text-sm text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100{{if eq .Level 3}} pl-3{{end}}">{{.Text}}</a></li>
          {{end}}
        </ul>
      </details>
      {{end}}
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
          <li><a href="/{{.Slug}}/" class="text-sm text-brand-600 dark:text-brand-400 hover:text-brand-800 dark:hover:text-brand-200">{{.Title}}</a></li>
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
    </article>
    {{if .Page.Headings}}
    <aside class="hidden xl:block w-56 flex-shrink-0">
      <div class="sticky top-8">
        <div id="font-size-controls" class="flex items-center gap-2 mb-4 pb-4 border-b border-gray-200 dark:border-gray-800"></div>
      <nav>
        <h3 class="font-semibold text-sm text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-3">On this page</h3>
        <ul class="space-y-1 text-sm">
          {{range .Page.Headings}}
          <li>
            <a href="#{{.ID}}" class="block py-1 text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100{{if eq .Level 3}} pl-3{{end}}">{{.Text}}</a>
          </li>
          {{end}}
        </ul>
      </nav>
      </div>
    </aside>
    {{end}}
  </div>
</div>
{{end}}`

const homepageTemplate = `{{define "title"}}{{.Site.Title}}{{end}}
{{define "content"}}
<div class="bg-brand-700 dark:bg-brand-900 text-white">
  <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-16">
    <h1 class="text-4xl font-bold mb-4">{{.Site.Title}}</h1>
    <p class="text-xl text-brand-100 max-w-2xl">{{.Site.Description}}</p>
  </div>
</div>

{{$featured := featuredPathways .Site.Pathways}}
{{if $featured}}
<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 pt-10 pb-6">
  <div class="flex items-center justify-between mb-4">
    <h2 class="text-2xl font-bold text-gray-900 dark:text-gray-100">Learning Pathways</h2>
    <a href="/pathways/" class="text-sm text-brand-600 dark:text-brand-400 hover:text-brand-800 dark:hover:text-brand-200">View all &rarr;</a>
  </div>
  <div class="flex gap-4 overflow-x-auto pb-2">
    {{range $featured}}
    <a href="/{{(index .Pages 0)}}/?pathway={{.Slug}}" class="flex-shrink-0 w-72 p-4 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-800 rounded-lg hover:border-brand-300 dark:hover:border-brand-700 hover:shadow-md transition-all">
      <h3 class="font-semibold text-sm text-gray-900 dark:text-gray-100 mb-1">{{.Name}}</h3>
      <p class="text-xs text-gray-500 dark:text-gray-400 mb-2">{{.Description}}</p>
      <span class="text-xs text-gray-400 dark:text-gray-500">{{len .Pages}} articles</span>
    </a>
    {{end}}
  </div>
</div>
{{end}}

<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-12">
  <h2 class="text-2xl font-bold text-gray-900 dark:text-gray-100 mb-6">Browse by Topic</h2>
  <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
    {{range .Site.Categories}}
    <div class="p-6 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-800 rounded-lg">
      <h3 class="font-semibold text-gray-900 dark:text-gray-100 mb-3">{{.Title}}</h3>
      <ul class="space-y-1">
        {{range firstN 3 .Pages}}
        <li><a href="/{{.Slug}}/" class="text-sm text-brand-600 dark:text-brand-400 hover:text-brand-800 dark:hover:text-brand-200">{{.Title}}</a></li>
        {{end}}
      </ul>
      {{if gt (len .Pages) 3}}<a href="/{{.Name}}/" class="inline-block mt-2 text-xs text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300">See all {{len .Pages}} articles &rarr;</a>{{end}}
    </div>
    {{end}}
  </div>
</div>
{{end}}`

const categoryTemplate = `{{define "title"}}{{.Category.Title}} — {{.Site.Title}}{{end}}
{{define "content"}}
<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-12">
  <nav class="text-sm mb-6">
    <a href="/" class="text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200">Home</a>
    <span class="text-gray-400 dark:text-gray-600 mx-2">/</span>
    <span class="text-gray-900 dark:text-gray-100">{{.Category.Title}}</span>
  </nav>
  <h1 class="text-3xl font-bold text-gray-900 dark:text-gray-100 mb-8">{{.Category.Title}}</h1>
  <div class="space-y-3">
    {{range .Category.Pages}}
    <a href="/{{.Slug}}/" class="block p-4 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-800 rounded-lg hover:border-brand-300 dark:hover:border-brand-700 hover:shadow-sm transition-all">
      <div class="flex items-center gap-3">
        {{if .Type}}<span class="inline-block text-xs font-medium uppercase tracking-wide text-brand-600 dark:text-brand-400 bg-brand-50 dark:bg-brand-900 px-2 py-0.5 rounded">{{typeIcon .Type}} {{typeLabel .Type}}</span>{{end}}
        <span class="text-sm font-medium text-gray-900 dark:text-gray-100">{{.Title}}</span>
      </div>
      {{if .Description}}<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">{{.Description}}</p>{{end}}
    </a>
    {{end}}
  </div>
</div>
{{end}}`

const printTemplate = `{{define "title"}}{{.Site.Title}} — Complete Reference{{end}}
{{define "content"}}
<div class="max-w-4xl mx-auto px-4 sm:px-6 lg:px-8 py-12">
  <h1 class="text-4xl font-bold text-gray-900 dark:text-gray-100 mb-2">{{.Site.Title}}</h1>
  <p class="text-gray-500 dark:text-gray-400 mb-12">Complete reference — all pages in one document</p>
  {{range .Pages}}
  <article class="mb-16 pb-16 border-b border-gray-200 dark:border-gray-800 last:border-0">
    <div class="flex items-center gap-3 mb-2">
      {{if .Type}}<span class="inline-block text-xs font-medium uppercase tracking-wide text-brand-600 dark:text-brand-400 bg-brand-50 dark:bg-brand-900 px-2 py-0.5 rounded">{{typeLabel .Type}}</span>{{end}}
      <span class="text-xs text-gray-400 dark:text-gray-500">{{.Category}}</span>
    </div>
    <h2 class="text-2xl font-bold text-gray-900 dark:text-gray-100 mb-4">{{.Title}}</h2>
    <div class="prose">
      {{.HTML}}
    </div>
  </article>
  {{end}}
</div>
{{end}}`

const pathwaysPageTemplate = `{{define "title"}}Learning Pathways — {{.Site.Title}}{{end}}
{{define "content"}}
<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-12">
  <nav class="text-sm mb-6">
    <a href="/" class="text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200">Home</a>
    <span class="text-gray-400 dark:text-gray-600 mx-2">/</span>
    <span class="text-gray-900 dark:text-gray-100">Learning Pathways</span>
  </nav>
  <h1 class="text-3xl font-bold text-gray-900 dark:text-gray-100 mb-8">Learning Pathways</h1>
  <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
    {{range .Site.Pathways}}
    <a href="/{{(index .Pages 0)}}/?pathway={{.Slug}}" class="block p-6 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-800 rounded-lg hover:border-brand-300 dark:hover:border-brand-700 hover:shadow-md transition-all">
      <h3 class="font-semibold text-gray-900 dark:text-gray-100 mb-2">{{.Name}}</h3>
      <p class="text-sm text-gray-600 dark:text-gray-400 mb-3">{{.Description}}</p>
      <span class="text-xs text-gray-400 dark:text-gray-500">{{len .Pages}} articles</span>
    </a>
    {{end}}
  </div>
</div>
{{end}}`

const notFoundTemplate = `{{define "title"}}Page Not Found — {{.Site.Title}}{{end}}
{{define "content"}}
<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-24 text-center">
  <h1 class="text-6xl font-bold text-gray-300 dark:text-gray-700 mb-4">404</h1>
  <p class="text-xl text-gray-600 dark:text-gray-400 mb-8">This page doesn't exist.</p>
  <a href="/" class="text-brand-600 dark:text-brand-400 hover:text-brand-800 dark:hover:text-brand-200 font-medium">Back to home</a>
</div>
{{end}}`
