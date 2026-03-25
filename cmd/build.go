package cmd

import (
	"fmt"
	"path/filepath"

	"github.com/peios/trail/internal/build"
	"github.com/peios/trail/internal/config"
	"github.com/peios/trail/internal/content"
)

func runBuild(f flags) error {
	dir, err := filepath.Abs(f.dir)
	if err != nil {
		return fmt.Errorf("resolving directory: %w", err)
	}

	outDir, err := filepath.Abs(f.output)
	if err != nil {
		return fmt.Errorf("resolving output directory: %w", err)
	}

	cfg, err := config.Load(dir)
	if err != nil {
		return fmt.Errorf("loading config: %w", err)
	}

	site, err := content.Load(dir, cfg)
	if err != nil {
		return fmt.Errorf("loading content: %w", err)
	}

	if err := build.Build(site, cfg, dir, outDir); err != nil {
		return fmt.Errorf("building site: %w", err)
	}

	fmt.Printf("Built %d pages in %d categories with %d pathways → %s\n",
		len(site.Pages), len(site.Categories), len(cfg.Pathways), outDir)
	return nil
}
