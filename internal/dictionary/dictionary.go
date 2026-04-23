package dictionary

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/BurntSushi/toml"
)

// Term represents a single dictionary definition.
type Term struct {
	Term       string   `toml:"term"`
	Abbr       string   `toml:"abbr"`
	Plural     string   `toml:"plural"`
	Aliases    []string `toml:"aliases"`
	Definition string   `toml:"definition"`
	Body       string   `toml:"body"`
	Category   string   `toml:"category"`
	Product    string   `toml:"product"`
	Refs       []Ref    `toml:"refs"`
	Etymology  string   `toml:"etymology"`

	// SourceFile is the filename this term was loaded from (set during loading).
	SourceFile string `toml:"-"`
}

// Ref is a reference link to further documentation.
type Ref struct {
	Label string `toml:"label"`
	Path  string `toml:"path"`
}

// Dictionary holds all loaded terms with an index for fast lookup.
type Dictionary struct {
	Terms []*Term

	// index maps scope → lowercase form → *Term.
	// Scope is "" for global terms, or a product slug for product-scoped terms.
	index map[string]map[string]*Term
}

// dictFile is the TOML structure of a dictionary file.
type dictFile struct {
	Terms []Term `toml:"terms"`
}

// Load reads all .toml files from dir and returns a Dictionary.
// Returns an error on duplicate terms/aliases within the same scope
// or missing required fields. If dir does not exist, returns an empty Dictionary.
func Load(dir string) (*Dictionary, error) {
	if _, err := os.Stat(dir); os.IsNotExist(err) {
		return empty(), nil
	}

	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil, fmt.Errorf("reading dictionary directory: %w", err)
	}

	var terms []*Term
	for _, entry := range entries {
		if entry.IsDir() || filepath.Ext(entry.Name()) != ".toml" {
			continue
		}

		path := filepath.Join(dir, entry.Name())
		var df dictFile
		if _, err := toml.DecodeFile(path, &df); err != nil {
			return nil, fmt.Errorf("parsing %s: %w", entry.Name(), err)
		}

		for i := range df.Terms {
			t := df.Terms[i]
			t.SourceFile = entry.Name()
			terms = append(terms, &t)
		}
	}

	if err := validate(terms); err != nil {
		return nil, err
	}

	index, err := buildIndex(terms)
	if err != nil {
		return nil, err
	}

	sort.Slice(terms, func(i, j int) bool {
		return strings.ToLower(terms[i].Term) < strings.ToLower(terms[j].Term)
	})

	return &Dictionary{Terms: terms, index: index}, nil
}

// Resolve looks up a term by any form (name, abbreviation, alias).
// Product-scoped definitions take precedence over global ones.
func (d *Dictionary) Resolve(form, productSlug string) *Term {
	lower := strings.ToLower(form)

	// Try product scope first.
	if productSlug != "" {
		if scope, ok := d.index[productSlug]; ok {
			if t, ok := scope[lower]; ok {
				return t
			}
		}
	}

	// Fall back to global.
	if global, ok := d.index[""]; ok {
		if t, ok := global[lower]; ok {
			return t
		}
	}

	return nil
}

// IsEmpty reports whether the dictionary contains no terms.
func (d *Dictionary) IsEmpty() bool {
	return len(d.Terms) == 0
}

// VisibleForms returns all unique lowercase forms that are resolvable
// in the given product context (product-scoped + global, deduplicated).
func (d *Dictionary) VisibleForms(productSlug string) []string {
	seen := make(map[string]struct{})
	var forms []string

	// Product-scoped forms take precedence.
	if productSlug != "" {
		if scope, ok := d.index[productSlug]; ok {
			for form := range scope {
				seen[form] = struct{}{}
				forms = append(forms, form)
			}
		}
	}

	// Global forms, skipping any shadowed by product scope.
	if global, ok := d.index[""]; ok {
		for form := range global {
			if _, ok := seen[form]; !ok {
				forms = append(forms, form)
			}
		}
	}

	return forms
}

// AllForms returns every unique lowercase string that resolves to this term.
// This includes the canonical name, abbreviation, aliases, and auto-generated
// plural forms. Plurals are included for matching but are not display aliases.
func (t *Term) AllForms() []string {
	seen := make(map[string]struct{})
	var forms []string
	add := func(s string) {
		lower := strings.ToLower(s)
		if _, ok := seen[lower]; !ok {
			seen[lower] = struct{}{}
			forms = append(forms, lower)
		}
	}

	add(t.Term)
	if t.Abbr != "" {
		add(t.Abbr)
	}
	for _, a := range t.Aliases {
		add(a)
	}

	// Add plural forms for matching.
	if t.Plural != "" {
		// Explicit irregular plural.
		add(t.Plural)
	} else {
		// Auto-generate term + "s" if it doesn't already end in "s".
		add(autoPlural(t.Term))
	}
	if t.Abbr != "" {
		add(autoPlural(t.Abbr))
	}

	return forms
}

// autoPlural returns s + "s" unless s already ends in "s" (case-insensitive).
func autoPlural(s string) string {
	if len(s) == 0 {
		return s
	}
	last := s[len(s)-1]
	if last == 's' || last == 'S' {
		return s
	}
	return s + "s"
}

func empty() *Dictionary {
	return &Dictionary{index: make(map[string]map[string]*Term)}
}

func validate(terms []*Term) error {
	for _, t := range terms {
		if t.Term == "" {
			return fmt.Errorf("%s: term has empty 'term' field", t.SourceFile)
		}
		if t.Definition == "" {
			return fmt.Errorf("%s: term %q has empty 'definition' field", t.SourceFile, t.Term)
		}
	}
	return nil
}

func buildIndex(terms []*Term) (map[string]map[string]*Term, error) {
	index := make(map[string]map[string]*Term)

	for _, t := range terms {
		scope := t.Product
		if _, ok := index[scope]; !ok {
			index[scope] = make(map[string]*Term)
		}
		scopeIdx := index[scope]

		for _, form := range t.AllForms() {
			if existing, ok := scopeIdx[form]; ok && existing != t {
				scopeLabel := "global scope"
				if scope != "" {
					scopeLabel = fmt.Sprintf("product %q scope", scope)
				}
				return nil, fmt.Errorf("dictionary conflict in %s: %q is claimed by both %q (%s) and %q (%s)",
					scopeLabel, form, existing.Term, existing.SourceFile, t.Term, t.SourceFile)
			}
			scopeIdx[form] = t
		}
	}

	return index, nil
}
