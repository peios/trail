package config

import (
	"os"
	"path/filepath"
	"testing"
)

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
