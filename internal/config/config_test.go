package config

import (
	"os"
	"path/filepath"
	"testing"
)

func TestParseSpecDirName(t *testing.T) {
	tests := []struct {
		input  string
		wantID string
		wantSlug string
	}{
		{"psd-004--kacs", "psd-004", "kacs"},
		{"psd-001--psd", "psd-001", "psd"},
		{"psd-002--binary-identifiers", "psd-002", "binary-identifiers"},
		{"plain-name", "", "plain-name"},
		{"", "", ""},
	}

	for _, tt := range tests {
		id, slug := ParseSpecDirName(tt.input)
		if id != tt.wantID || slug != tt.wantSlug {
			t.Errorf("ParseSpecDirName(%q) = (%q, %q), want (%q, %q)",
				tt.input, id, slug, tt.wantID, tt.wantSlug)
		}
	}
}

func TestDictDirDefault(t *testing.T) {
	c := &DictionaryConfig{}
	got := c.DictDir("/site")
	want := filepath.Join("/site", "dict")
	if got != want {
		t.Errorf("want %q, got %q", want, got)
	}
}

func TestDictDirCustom(t *testing.T) {
	c := &DictionaryConfig{Dir: "glossary"}
	got := c.DictDir("/site")
	want := filepath.Join("/site", "glossary")
	if got != want {
		t.Errorf("want %q, got %q", want, got)
	}
}

func TestAutoLinkForProductGlobalDefault(t *testing.T) {
	c := &DictionaryConfig{AutoLink: false}
	if c.AutoLinkForProduct("kacs") {
		t.Error("expected false when global is false and no product override")
	}

	c.AutoLink = true
	if !c.AutoLinkForProduct("kacs") {
		t.Error("expected true when global is true and no product override")
	}
}

func TestAutoLinkForProductOverride(t *testing.T) {
	yes := true
	no := false

	c := &DictionaryConfig{
		AutoLink: false,
		Products: map[string]DictionaryProductConfig{
			"kacs":  {AutoLink: &yes},
			"peios": {AutoLink: &no},
		},
	}

	if !c.AutoLinkForProduct("kacs") {
		t.Error("expected true: product override enables auto_link")
	}
	if c.AutoLinkForProduct("peios") {
		t.Error("expected false: product override disables auto_link")
	}
	if c.AutoLinkForProduct("lcs") {
		t.Error("expected false: no override, falls back to global false")
	}
}

func TestAutoLinkForProductOverrideWhenGlobalTrue(t *testing.T) {
	no := false

	c := &DictionaryConfig{
		AutoLink: true,
		Products: map[string]DictionaryProductConfig{
			"peios": {AutoLink: &no},
		},
	}

	if c.AutoLinkForProduct("peios") {
		t.Error("expected false: product override disables even when global is true")
	}
	if !c.AutoLinkForProduct("kacs") {
		t.Error("expected true: no override, falls back to global true")
	}
}

func TestDictionaryConfigFromToml(t *testing.T) {
	dir := t.TempDir()
	tomlContent := `
title = "Test Site"

[dictionary]
dir = "glossary"
auto_link = true

[dictionary.products.kacs]
auto_link = false
`
	if err := os.WriteFile(filepath.Join(dir, "trail.toml"), []byte(tomlContent), 0o644); err != nil {
		t.Fatal(err)
	}

	cfg, err := Load(dir)
	if err != nil {
		t.Fatal(err)
	}

	if cfg.Dictionary.Dir != "glossary" {
		t.Errorf("dir: want 'glossary', got %q", cfg.Dictionary.Dir)
	}
	if !cfg.Dictionary.AutoLink {
		t.Error("auto_link: want true")
	}

	p, ok := cfg.Dictionary.Products["kacs"]
	if !ok {
		t.Fatal("expected product override for 'kacs'")
	}
	if p.AutoLink == nil || *p.AutoLink != false {
		t.Error("kacs auto_link: want false")
	}
}

func TestLoadSpecProduct(t *testing.T) {
	dir := t.TempDir()

	// Root trail.toml
	if err := os.WriteFile(filepath.Join(dir, "trail.toml"), []byte(`title = "Test"`), 0o644); err != nil {
		t.Fatal(err)
	}

	// Create spec directory with id--name convention
	specDir := filepath.Join(dir, "specs", "psd-004--kacs")
	if err := os.MkdirAll(specDir, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(specDir, "trail.toml"), []byte(`name = "KACS"
description = "Kernel Access Control Subsystem"`), 0o644); err != nil {
		t.Fatal(err)
	}

	// Create a version directory
	verDir := filepath.Join(specDir, "v0.20")
	if err := os.MkdirAll(verDir, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(verDir, "trail.toml"), []byte(`status = "draft"
date = "2026-04-25"`), 0o644); err != nil {
		t.Fatal(err)
	}

	cfg, err := Load(dir)
	if err != nil {
		t.Fatal(err)
	}

	if len(cfg.Products) != 1 {
		t.Fatalf("want 1 product, got %d", len(cfg.Products))
	}

	prod := cfg.Products[0]
	if prod.Slug != "psd-004" {
		t.Errorf("slug: want %q, got %q", "psd-004", prod.Slug)
	}
	if prod.SpecID != "psd-004" {
		t.Errorf("spec ID: want %q, got %q", "psd-004", prod.SpecID)
	}
	if prod.Name != "KACS" {
		t.Errorf("name: want %q, got %q", "KACS", prod.Name)
	}
	if prod.Kind != "spec" {
		t.Errorf("kind: want %q, got %q", "spec", prod.Kind)
	}
	if len(prod.Versions) != 1 {
		t.Fatalf("want 1 version, got %d", len(prod.Versions))
	}
	if prod.Versions[0].Name != "v0.20" {
		t.Errorf("version name: want %q, got %q", "v0.20", prod.Versions[0].Name)
	}
}

func TestLoadSpecProductOrdering(t *testing.T) {
	dir := t.TempDir()

	if err := os.WriteFile(filepath.Join(dir, "trail.toml"), []byte(`title = "Test"`), 0o644); err != nil {
		t.Fatal(err)
	}

	// Create specs out of order
	for _, spec := range []struct{ dir, name string }{
		{"psd-003--loregd", "loregd"},
		{"psd-001--psd", "PSD"},
		{"psd-002--binary-identifiers", "Binary Identifiers"},
	} {
		specDir := filepath.Join(dir, "specs", spec.dir)
		if err := os.MkdirAll(specDir, 0o755); err != nil {
			t.Fatal(err)
		}
		toml := `name = "` + spec.name + `"` + "\n" + `description = "test"`
		if err := os.WriteFile(filepath.Join(specDir, "trail.toml"), []byte(toml), 0o644); err != nil {
			t.Fatal(err)
		}
	}

	cfg, err := Load(dir)
	if err != nil {
		t.Fatal(err)
	}

	if len(cfg.Products) != 3 {
		t.Fatalf("want 3 products, got %d", len(cfg.Products))
	}

	// Specs sort by SpecID (psd-001 < psd-002 < psd-003)
	// Slug and SpecID are both the spec identifier
	wantIDs := []string{"psd-001", "psd-002", "psd-003"}
	for i, want := range wantIDs {
		if cfg.Products[i].SpecID != want {
			t.Errorf("product[%d].SpecID = %q, want %q", i, cfg.Products[i].SpecID, want)
		}
		if cfg.Products[i].Slug != want {
			t.Errorf("product[%d].Slug = %q, want %q", i, cfg.Products[i].Slug, want)
		}
	}
}

func TestDictionaryConfigDefaults(t *testing.T) {
	dir := t.TempDir()
	tomlContent := `title = "Test Site"`
	if err := os.WriteFile(filepath.Join(dir, "trail.toml"), []byte(tomlContent), 0o644); err != nil {
		t.Fatal(err)
	}

	cfg, err := Load(dir)
	if err != nil {
		t.Fatal(err)
	}

	if cfg.Dictionary.Dir != "" {
		t.Errorf("dir: want empty (default), got %q", cfg.Dictionary.Dir)
	}
	if cfg.Dictionary.AutoLink {
		t.Error("auto_link: want false by default")
	}
	if cfg.Dictionary.Products != nil {
		t.Error("products: want nil by default")
	}

	// DictDir should still return the default path.
	want := filepath.Join("/site", "dict")
	if got := cfg.Dictionary.DictDir("/site"); got != want {
		t.Errorf("DictDir: want %q, got %q", want, got)
	}
}
