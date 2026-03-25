package server

import (
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/peios/trail/internal/build"
	"github.com/peios/trail/internal/config"
	"github.com/peios/trail/internal/content"
)

func Serve(dir, output, port string) error {
	absDir, err := filepath.Abs(dir)
	if err != nil {
		return err
	}
	outDir, err := filepath.Abs(output)
	if err != nil {
		return err
	}

	if err := rebuild(absDir, outDir); err != nil {
		return err
	}
	writeLiveReloadJS(outDir)

	// SSE clients for live reload
	var (
		mu      sync.Mutex
		clients = make(map[chan struct{}]struct{})
	)

	notify := func() {
		mu.Lock()
		defer mu.Unlock()
		for ch := range clients {
			select {
			case ch <- struct{}{}:
			default:
			}
		}
	}

	// File watcher: poll for changes
	go func() {
		var lastMod time.Time
		for {
			time.Sleep(500 * time.Millisecond)
			mod := latestMod(absDir)
			if mod.After(lastMod) {
				lastMod = mod
				if err := rebuild(absDir, outDir); err != nil {
					fmt.Fprintf(os.Stderr, "rebuild error: %v\n", err)
					continue
				}
				writeLiveReloadJS(outDir)
				fmt.Println("Rebuilt.")
				notify()
			}
		}
	}()

	mux := http.NewServeMux()

	// SSE endpoint for live reload
	mux.HandleFunc("/__reload", func(w http.ResponseWriter, r *http.Request) {
		flusher, ok := w.(http.Flusher)
		if !ok {
			http.Error(w, "streaming not supported", http.StatusInternalServerError)
			return
		}

		w.Header().Set("Content-Type", "text/event-stream")
		w.Header().Set("Cache-Control", "no-cache")
		w.Header().Set("Connection", "keep-alive")

		ch := make(chan struct{}, 1)
		mu.Lock()
		clients[ch] = struct{}{}
		mu.Unlock()

		defer func() {
			mu.Lock()
			delete(clients, ch)
			mu.Unlock()
		}()

		for {
			select {
			case <-ch:
				fmt.Fprintf(w, "data: reload\n\n")
				flusher.Flush()
			case <-r.Context().Done():
				return
			}
		}
	})

	// Static file server with 404 fallback
	fs := http.Dir(outDir)
	mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		// Try to serve the file normally
		f, err := fs.Open(r.URL.Path)
		if err == nil {
			f.Close()
			http.FileServer(fs).ServeHTTP(w, r)
			return
		}
		// Try path/index.html for directory-style URLs
		if f, err := fs.Open(r.URL.Path + "/index.html"); err == nil {
			f.Close()
			r.URL.Path = r.URL.Path + "/"
			http.FileServer(fs).ServeHTTP(w, r)
			return
		}
		// Serve 404 page
		w.WriteHeader(http.StatusNotFound)
		notFound, err := os.ReadFile(filepath.Join(outDir, "404.html"))
		if err != nil {
			http.Error(w, "404 not found", http.StatusNotFound)
			return
		}
		w.Write(notFound)
	})

	fmt.Printf("Serving on http://localhost:%s (live reload enabled)\n", port)
	return http.ListenAndServe(":"+port, mux)
}

func rebuild(dir, outDir string) error {
	cfg, err := config.Load(dir)
	if err != nil {
		return fmt.Errorf("loading config: %w", err)
	}

	site, err := content.Load(dir, cfg)
	if err != nil {
		return fmt.Errorf("loading content: %w", err)
	}

	if err := build.Build(site, cfg, outDir); err != nil {
		return fmt.Errorf("building site: %w", err)
	}

	return nil
}

func writeLiveReloadJS(outDir string) {
	js := `(function() {
  var es = new EventSource('/__reload');
  es.onmessage = function() { window.location.reload(); };
  es.onerror = function() { es.close(); setTimeout(function() { window.location.reload(); }, 1000); };
})();`
	os.MkdirAll(filepath.Join(outDir, "assets"), 0o755)
	os.WriteFile(filepath.Join(outDir, "assets", "livereload.js"), []byte(js), 0o644)
}

func latestMod(dir string) time.Time {
	var latest time.Time
	filepath.Walk(dir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return nil
		}
		// Skip output directory and hidden dirs
		name := info.Name()
		if info.IsDir() && (name == "_site" || strings.HasPrefix(name, ".")) {
			return filepath.SkipDir
		}
		if !info.IsDir() && info.ModTime().After(latest) {
			latest = info.ModTime()
		}
		return nil
	})
	return latest
}
