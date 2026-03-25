package theme

import (
	"fmt"
	"html/template"
	"os"
	"path/filepath"

	"github.com/peios/trail/internal/config"
)

type Templates struct {
	Page     *template.Template
	Homepage *template.Template
	Category *template.Template
	NotFound *template.Template
}

func LoadTemplates(cfg *config.Config) (*Templates, error) {
	funcMap := template.FuncMap{
		"typeLabel": func(t string) string {
			switch t {
			case "concept":
				return "Concept"
			case "how-to":
				return "How-to"
			default:
				return t
			}
		},
	}

	// Parse base template
	base, err := template.New("base").Funcs(funcMap).Parse(baseTemplate)
	if err != nil {
		return nil, fmt.Errorf("parsing base template: %w", err)
	}

	// Clone base and add page-specific blocks
	pageTmpl, err := template.Must(base.Clone()).Parse(pageTemplate)
	if err != nil {
		return nil, fmt.Errorf("parsing page template: %w", err)
	}

	// Clone base and add homepage-specific blocks
	homepageTmpl, err := template.Must(base.Clone()).Parse(homepageTemplate)
	if err != nil {
		return nil, fmt.Errorf("parsing homepage template: %w", err)
	}

	categoryTmpl, err := template.Must(base.Clone()).Parse(categoryTemplate)
	if err != nil {
		return nil, fmt.Errorf("parsing category template: %w", err)
	}

	notFoundTmpl, err := template.Must(base.Clone()).Parse(notFoundTemplate)
	if err != nil {
		return nil, fmt.Errorf("parsing 404 template: %w", err)
	}

	return &Templates{
		Page:     pageTmpl,
		Homepage: homepageTmpl,
		Category: categoryTmpl,
		NotFound: notFoundTmpl,
	}, nil
}

func WriteAssets(outDir string) error {
	assetsDir := filepath.Join(outDir, "assets")
	if err := os.MkdirAll(assetsDir, 0o755); err != nil {
		return err
	}

	if err := os.WriteFile(filepath.Join(assetsDir, "pathway.js"), []byte(pathwayJS), 0o644); err != nil {
		return err
	}

	if err := os.WriteFile(filepath.Join(assetsDir, "theme.js"), []byte(themeJS), 0o644); err != nil {
		return err
	}

	if err := os.WriteFile(filepath.Join(assetsDir, "search.js"), []byte(searchJS), 0o644); err != nil {
		return err
	}

	if err := os.WriteFile(filepath.Join(assetsDir, "mobile.js"), []byte(mobileJS), 0o644); err != nil {
		return err
	}

	return os.WriteFile(filepath.Join(assetsDir, "copycode.js"), []byte(copyCodeJS), 0o644)
}
