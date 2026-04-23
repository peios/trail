package dictionary

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func writeFile(t *testing.T, dir, name, content string) {
	t.Helper()
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, name), []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}

// --- Loading ---

func TestLoadNonexistentDir(t *testing.T) {
	d, err := Load(filepath.Join(t.TempDir(), "nonexistent"))
	if err != nil {
		t.Fatal(err)
	}
	if !d.IsEmpty() {
		t.Fatal("expected empty dictionary for nonexistent directory")
	}
}

func TestLoadEmptyFile(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "dict")
	writeFile(t, dir, "empty.toml", "")

	d, err := Load(dir)
	if err != nil {
		t.Fatal(err)
	}
	if !d.IsEmpty() {
		t.Fatal("expected empty dictionary from empty file")
	}
}

func TestLoadBasic(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "dict")
	writeFile(t, dir, "kacs.toml", `
[[terms]]
term = "Security Descriptor"
abbr = "SD"
aliases = ["security descriptors", "SDs"]
definition = "A data structure containing security information for a securable object."
category = "KACS"
product = "kacs"
etymology = "From the Windows NT security model"

[[terms.refs]]
label = "What is an SD?"
path = "peios/access-control/security-descriptors"

[[terms.refs]]
label = "SD Specification"
path = "spec/kacs/v0.22/security-descriptors/sd-format"

[[terms]]
term = "Access Control Entry"
abbr = "ACE"
definition = "A single entry in an access control list."
category = "KACS"
product = "kacs"
`)

	d, err := Load(dir)
	if err != nil {
		t.Fatal(err)
	}
	if len(d.Terms) != 2 {
		t.Fatalf("expected 2 terms, got %d", len(d.Terms))
	}

	// Terms should be sorted alphabetically.
	if d.Terms[0].Term != "Access Control Entry" {
		t.Errorf("expected first term 'Access Control Entry', got %q", d.Terms[0].Term)
	}
	if d.Terms[1].Term != "Security Descriptor" {
		t.Errorf("expected second term 'Security Descriptor', got %q", d.Terms[1].Term)
	}

	// Check all fields loaded on SD.
	sd := d.Resolve("Security Descriptor", "kacs")
	if sd == nil {
		t.Fatal("failed to resolve 'Security Descriptor'")
	}
	if sd.Abbr != "SD" {
		t.Errorf("abbr: want 'SD', got %q", sd.Abbr)
	}
	if len(sd.Aliases) != 2 {
		t.Fatalf("aliases: want 2, got %d", len(sd.Aliases))
	}
	if sd.Category != "KACS" {
		t.Errorf("category: want 'KACS', got %q", sd.Category)
	}
	if sd.Product != "kacs" {
		t.Errorf("product: want 'kacs', got %q", sd.Product)
	}
	if sd.Etymology != "From the Windows NT security model" {
		t.Errorf("etymology: got %q", sd.Etymology)
	}
	if len(sd.Refs) != 2 {
		t.Fatalf("refs: want 2, got %d", len(sd.Refs))
	}
	if sd.Refs[0].Label != "What is an SD?" || sd.Refs[0].Path != "peios/access-control/security-descriptors" {
		t.Errorf("refs[0]: got %+v", sd.Refs[0])
	}
}

func TestLoadMultipleFiles(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "dict")
	writeFile(t, dir, "kacs.toml", `
[[terms]]
term = "Token"
definition = "A kernel object representing identity."
category = "KACS"
`)
	writeFile(t, dir, "lcs.toml", `
[[terms]]
term = "Hive"
definition = "A top-level registry namespace."
category = "LCS"
`)
	writeFile(t, dir, "general.toml", `
[[terms]]
term = "Peios"
definition = "An operating system for secure infrastructure."
category = "General"
`)

	d, err := Load(dir)
	if err != nil {
		t.Fatal(err)
	}
	if len(d.Terms) != 3 {
		t.Fatalf("expected 3 terms, got %d", len(d.Terms))
	}
	for _, name := range []string{"Token", "Hive", "Peios"} {
		if d.Resolve(name, "") == nil {
			t.Errorf("failed to resolve %q", name)
		}
	}
}

func TestLoadMinimalTerm(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "dict")
	writeFile(t, dir, "test.toml", `
[[terms]]
term = "Foo"
definition = "A foo."
`)

	d, err := Load(dir)
	if err != nil {
		t.Fatal(err)
	}
	if len(d.Terms) != 1 {
		t.Fatalf("expected 1 term, got %d", len(d.Terms))
	}
	foo := d.Terms[0]
	if foo.Abbr != "" || len(foo.Aliases) != 0 || foo.Body != "" ||
		foo.Category != "" || foo.Product != "" || foo.Etymology != "" || len(foo.Refs) != 0 {
		t.Errorf("optional fields should be zero-valued: %+v", foo)
	}
}

func TestLoadNonTomlFilesIgnored(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "dict")
	writeFile(t, dir, "readme.md", "# Not a dictionary file")
	writeFile(t, dir, "notes.txt", "just notes")
	writeFile(t, dir, "valid.toml", `
[[terms]]
term = "Test"
definition = "A test term."
`)

	d, err := Load(dir)
	if err != nil {
		t.Fatal(err)
	}
	if len(d.Terms) != 1 {
		t.Fatalf("expected 1 term (non-TOML ignored), got %d", len(d.Terms))
	}
}

func TestLoadSubdirectoriesIgnored(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "dict")
	writeFile(t, dir, "valid.toml", `
[[terms]]
term = "Top"
definition = "Top-level."
`)
	writeFile(t, filepath.Join(dir, "subdir"), "nested.toml", `
[[terms]]
term = "Nested"
definition = "Should not be loaded."
`)

	d, err := Load(dir)
	if err != nil {
		t.Fatal(err)
	}
	if len(d.Terms) != 1 {
		t.Fatalf("expected 1 term (subdirs ignored), got %d", len(d.Terms))
	}
	if d.Terms[0].Term != "Top" {
		t.Errorf("expected 'Top', got %q", d.Terms[0].Term)
	}
}

func TestLoadSourceFileTracking(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "dict")
	writeFile(t, dir, "kacs.toml", `
[[terms]]
term = "Token"
definition = "A kernel object."
`)

	d, err := Load(dir)
	if err != nil {
		t.Fatal(err)
	}
	if d.Terms[0].SourceFile != "kacs.toml" {
		t.Errorf("SourceFile: want 'kacs.toml', got %q", d.Terms[0].SourceFile)
	}
}

// --- Validation ---

func TestValidateMissingTerm(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "dict")
	writeFile(t, dir, "bad.toml", `
[[terms]]
definition = "Has a definition but no term name."
`)

	_, err := Load(dir)
	if err == nil {
		t.Fatal("expected error for missing 'term' field")
	}
	if !strings.Contains(err.Error(), "empty 'term'") {
		t.Errorf("expected 'empty term' error, got: %v", err)
	}
}

func TestValidateMissingDefinition(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "dict")
	writeFile(t, dir, "bad.toml", `
[[terms]]
term = "Something"
`)

	_, err := Load(dir)
	if err == nil {
		t.Fatal("expected error for missing 'definition' field")
	}
	if !strings.Contains(err.Error(), "empty 'definition'") {
		t.Errorf("expected 'empty definition' error, got: %v", err)
	}
}

func TestValidateInvalidToml(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "dict")
	writeFile(t, dir, "bad.toml", `this is not valid toml {{{`)

	_, err := Load(dir)
	if err == nil {
		t.Fatal("expected error for invalid TOML")
	}
}

// --- Conflict detection ---

func TestConflictDuplicateTermName(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "dict")
	writeFile(t, dir, "a.toml", `
[[terms]]
term = "Token"
definition = "First definition."
`)
	writeFile(t, dir, "b.toml", `
[[terms]]
term = "Token"
definition = "Second definition."
`)

	_, err := Load(dir)
	if err == nil {
		t.Fatal("expected conflict error for duplicate term name")
	}
	if !strings.Contains(err.Error(), "conflict") {
		t.Errorf("expected conflict error, got: %v", err)
	}
	if !strings.Contains(err.Error(), "Token") {
		t.Errorf("expected error to mention 'Token', got: %v", err)
	}
}

func TestConflictDuplicateTermNameCaseInsensitive(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "dict")
	writeFile(t, dir, "test.toml", `
[[terms]]
term = "Token"
definition = "Uppercase."

[[terms]]
term = "token"
definition = "Lowercase."
`)

	_, err := Load(dir)
	if err == nil {
		t.Fatal("expected conflict error for case-insensitive duplicate")
	}
	if !strings.Contains(err.Error(), "conflict") {
		t.Errorf("expected conflict error, got: %v", err)
	}
}

func TestConflictAbbrVsTerm(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "dict")
	writeFile(t, dir, "test.toml", `
[[terms]]
term = "SD"
definition = "Something called SD."

[[terms]]
term = "Security Descriptor"
abbr = "SD"
definition = "A data structure."
`)

	_, err := Load(dir)
	if err == nil {
		t.Fatal("expected conflict error for abbr-vs-term collision")
	}
	if !strings.Contains(err.Error(), "conflict") {
		t.Errorf("expected conflict error, got: %v", err)
	}
}

func TestConflictAliasVsTerm(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "dict")
	writeFile(t, dir, "test.toml", `
[[terms]]
term = "Access Token"
definition = "A kernel object."

[[terms]]
term = "Session Token"
aliases = ["access token"]
definition = "A session identifier."
`)

	_, err := Load(dir)
	if err == nil {
		t.Fatal("expected conflict error for alias-vs-term collision")
	}
	if !strings.Contains(err.Error(), "conflict") {
		t.Errorf("expected conflict error, got: %v", err)
	}
}

func TestConflictAliasVsAbbr(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "dict")
	writeFile(t, dir, "test.toml", `
[[terms]]
term = "First Thing"
abbr = "FT"
definition = "First."

[[terms]]
term = "Second Thing"
aliases = ["ft"]
definition = "Second."
`)

	_, err := Load(dir)
	if err == nil {
		t.Fatal("expected conflict error for alias-vs-abbr collision")
	}
	if !strings.Contains(err.Error(), "conflict") {
		t.Errorf("expected conflict error, got: %v", err)
	}
}

func TestConflictAliasVsAlias(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "dict")
	writeFile(t, dir, "test.toml", `
[[terms]]
term = "Alpha"
aliases = ["shared"]
definition = "First."

[[terms]]
term = "Beta"
aliases = ["shared"]
definition = "Second."
`)

	_, err := Load(dir)
	if err == nil {
		t.Fatal("expected conflict error for alias-vs-alias collision")
	}
	if !strings.Contains(err.Error(), "conflict") {
		t.Errorf("expected conflict error, got: %v", err)
	}
}

func TestConflictWithinSameProduct(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "dict")
	writeFile(t, dir, "test.toml", `
[[terms]]
term = "Key"
definition = "First key def."
product = "lcs"

[[terms]]
term = "Key"
definition = "Second key def."
product = "lcs"
`)

	_, err := Load(dir)
	if err == nil {
		t.Fatal("expected conflict error within same product scope")
	}
	if !strings.Contains(err.Error(), `product "lcs"`) {
		t.Errorf("expected error to mention product scope, got: %v", err)
	}
}

func TestConflictCrossProductAllowed(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "dict")
	writeFile(t, dir, "test.toml", `
[[terms]]
term = "Hive"
definition = "Global definition."

[[terms]]
term = "Hive"
definition = "LCS-specific definition."
product = "lcs"
`)

	d, err := Load(dir)
	if err != nil {
		t.Fatalf("cross-scope should not conflict: %v", err)
	}
	if len(d.Terms) != 2 {
		t.Fatalf("expected 2 terms, got %d", len(d.Terms))
	}
}

func TestConflictCrossDifferentProductsAllowed(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "dict")
	writeFile(t, dir, "test.toml", `
[[terms]]
term = "Key"
definition = "LCS key."
product = "lcs"

[[terms]]
term = "Key"
definition = "KACS key."
product = "kacs"
`)

	d, err := Load(dir)
	if err != nil {
		t.Fatalf("different product scopes should not conflict: %v", err)
	}
	if len(d.Terms) != 2 {
		t.Fatalf("expected 2 terms, got %d", len(d.Terms))
	}
}

func TestConflictErrorMentionsBothTerms(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "dict")
	writeFile(t, dir, "a.toml", `
[[terms]]
term = "Widget"
definition = "First widget."
`)
	writeFile(t, dir, "b.toml", `
[[terms]]
term = "Gadget"
aliases = ["widget"]
definition = "A gadget."
`)

	_, err := Load(dir)
	if err == nil {
		t.Fatal("expected conflict error")
	}
	msg := err.Error()
	if !strings.Contains(msg, "Widget") || !strings.Contains(msg, "Gadget") {
		t.Errorf("error should mention both terms, got: %v", err)
	}
	if !strings.Contains(msg, "a.toml") || !strings.Contains(msg, "b.toml") {
		t.Errorf("error should mention both source files, got: %v", err)
	}
}

// --- Resolution ---

func TestResolveByAllForms(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "dict")
	writeFile(t, dir, "test.toml", `
[[terms]]
term = "Security Descriptor"
abbr = "SD"
aliases = ["security descriptors"]
definition = "A data structure."
`)

	d, err := Load(dir)
	if err != nil {
		t.Fatal(err)
	}

	cases := []struct {
		form string
		want bool
	}{
		{"Security Descriptor", true},
		{"security descriptor", true},
		{"SD", true},
		{"sd", true},
		{"Sd", true},
		{"SDs", true},  // auto-plural of abbr
		{"security descriptors", true},
		{"SECURITY DESCRIPTORS", true},
		{"nonexistent", false},
		{"Security", false},
		{"Descriptor", false},
	}

	for _, tc := range cases {
		t.Run(tc.form, func(t *testing.T) {
			got := d.Resolve(tc.form, "")
			if tc.want && got == nil {
				t.Errorf("expected to resolve %q", tc.form)
			}
			if !tc.want && got != nil {
				t.Errorf("expected nil for %q, got %q", tc.form, got.Term)
			}
			if tc.want && got != nil && got.Term != "Security Descriptor" {
				t.Errorf("expected 'Security Descriptor', got %q", got.Term)
			}
		})
	}
}

func TestResolveProductShadowsGlobal(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "dict")
	writeFile(t, dir, "global.toml", `
[[terms]]
term = "Layer"
definition = "A generic organizational level."
`)
	writeFile(t, dir, "lcs.toml", `
[[terms]]
term = "Layer"
definition = "A configuration overlay in the LCS registry."
product = "lcs"
`)

	d, err := Load(dir)
	if err != nil {
		t.Fatal(err)
	}

	// From LCS context: product-scoped definition wins.
	lcs := d.Resolve("Layer", "lcs")
	if lcs == nil {
		t.Fatal("failed to resolve 'Layer' in lcs scope")
	}
	if lcs.Product != "lcs" {
		t.Errorf("expected product 'lcs', got %q", lcs.Product)
	}

	// From unrelated product: falls back to global.
	other := d.Resolve("Layer", "peios")
	if other == nil {
		t.Fatal("failed to resolve 'Layer' with global fallback")
	}
	if other.Product != "" {
		t.Errorf("expected global term (empty product), got %q", other.Product)
	}

	// No product context: global.
	none := d.Resolve("Layer", "")
	if none == nil {
		t.Fatal("failed to resolve 'Layer' with no scope")
	}
	if none.Product != "" {
		t.Errorf("expected global term, got %q", none.Product)
	}
}

func TestResolveProductOnlyTerm(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "dict")
	writeFile(t, dir, "test.toml", `
[[terms]]
term = "RSI"
definition = "Registry Source Interface."
product = "lcs"
`)

	d, err := Load(dir)
	if err != nil {
		t.Fatal(err)
	}

	// Should resolve in its own product scope.
	if d.Resolve("RSI", "lcs") == nil {
		t.Error("expected to resolve 'RSI' in lcs scope")
	}

	// Should NOT resolve in other scopes (no global fallback).
	if d.Resolve("RSI", "kacs") != nil {
		t.Error("expected nil for 'RSI' in kacs scope (no global)")
	}
	if d.Resolve("RSI", "") != nil {
		t.Error("expected nil for 'RSI' with no scope (no global)")
	}
}

// --- AllForms ---

func TestAllForms(t *testing.T) {
	term := &Term{
		Term:    "Security Descriptor",
		Abbr:    "SD",
		Aliases: []string{"SDs", "security descriptors"},
	}

	forms := term.AllForms()
	expected := map[string]bool{
		"security descriptor":  true,
		"sd":                   true,
		"sds":                  true,
		"security descriptors": true,
	}

	if len(forms) != len(expected) {
		t.Fatalf("expected %d forms, got %d: %v", len(expected), len(forms), forms)
	}
	for _, f := range forms {
		if !expected[f] {
			t.Errorf("unexpected form: %q", f)
		}
	}
}

func TestAllFormsDedup(t *testing.T) {
	term := &Term{
		Term:    "SD",
		Abbr:    "sd",
		Aliases: []string{"SD", "Sd"},
	}

	forms := term.AllForms()
	// "sd" + auto-plural "sds" = 2
	if len(forms) != 2 {
		t.Fatalf("expected 2 deduplicated forms, got %d: %v", len(forms), forms)
	}
}

func TestAllFormsMinimal(t *testing.T) {
	term := &Term{Term: "Foo"}
	forms := term.AllForms()
	// "foo" + auto-plural "foos" = 2
	expected := map[string]bool{"foo": true, "foos": true}
	if len(forms) != len(expected) {
		t.Fatalf("expected %v, got %v", expected, forms)
	}
	for _, f := range forms {
		if !expected[f] {
			t.Errorf("unexpected form: %q", f)
		}
	}
}

func TestAllFormsAutoPlural(t *testing.T) {
	term := &Term{
		Term: "Token",
		Abbr: "TK",
	}

	forms := term.AllForms()
	expected := map[string]bool{
		"token":  true,
		"tokens": true, // auto-plural of term
		"tk":     true,
		"tks":    true, // auto-plural of abbr
	}
	if len(forms) != len(expected) {
		t.Fatalf("expected %d forms, got %d: %v", len(expected), len(forms), forms)
	}
	for _, f := range forms {
		if !expected[f] {
			t.Errorf("unexpected form: %q", f)
		}
	}
}

func TestAllFormsExplicitPlural(t *testing.T) {
	term := &Term{
		Term:   "Access Control Entry",
		Abbr:   "ACE",
		Plural: "Access Control Entries",
	}

	forms := term.AllForms()
	expected := map[string]bool{
		"access control entry":   true,
		"ace":                    true,
		"access control entries": true, // explicit plural
		"aces":                   true, // auto-plural of abbr
	}
	if len(forms) != len(expected) {
		t.Fatalf("expected %d forms, got %d: %v", len(expected), len(forms), forms)
	}
	for _, f := range forms {
		if !expected[f] {
			t.Errorf("unexpected form: %q", f)
		}
	}
}

func TestAllFormsNoDoubleS(t *testing.T) {
	// Terms ending in "s" should not get "ss" appended.
	term := &Term{Term: "Windows"}
	forms := term.AllForms()
	for _, f := range forms {
		if f == "windowss" {
			t.Error("should not double the 's'")
		}
	}
	if len(forms) != 1 {
		t.Fatalf("expected 1 form (no auto-plural for s-ending term), got %d: %v", len(forms), forms)
	}
}

// --- Sorting ---

func TestTermsSortedAlphabetically(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "dict")
	writeFile(t, dir, "test.toml", `
[[terms]]
term = "Zebra"
definition = "Last."

[[terms]]
term = "Alpha"
definition = "First."

[[terms]]
term = "Middle"
definition = "Middle."
`)

	d, err := Load(dir)
	if err != nil {
		t.Fatal(err)
	}

	names := make([]string, len(d.Terms))
	for i, term := range d.Terms {
		names[i] = term.Term
	}

	expected := []string{"Alpha", "Middle", "Zebra"}
	for i, want := range expected {
		if names[i] != want {
			t.Errorf("position %d: want %q, got %q (full order: %v)", i, want, names[i], names)
			break
		}
	}
}
