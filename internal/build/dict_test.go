package build

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/peios/trail/internal/config"
	"github.com/peios/trail/internal/content"
	"github.com/peios/trail/internal/dictionary"
)

func loadTestDict(t *testing.T, toml string) *dictionary.Dictionary {
	t.Helper()
	dir := filepath.Join(t.TempDir(), "dict")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "test.toml"), []byte(toml), 0o644); err != nil {
		t.Fatal(err)
	}
	d, err := dictionary.Load(dir)
	if err != nil {
		t.Fatal(err)
	}
	return d
}

var testDict = `
[[terms]]
term = "Security Descriptor"
abbr = "SD"
aliases = ["SDs", "security descriptors"]
definition = "A data structure containing security information."
category = "KACS"

[[terms]]
term = "Access Control Entry"
abbr = "ACE"
definition = "A single entry in an access control list."
category = "KACS"

[[terms]]
term = "Hive"
definition = "A top-level registry namespace."
category = "LCS"
`

// --- Manual [Term] replacement ---

func TestManualBasic(t *testing.T) {
	dict := loadTestDict(t, testDict)
	input := `<p>A [SD] controls access.</p>`
	got := transformDictManual(input, dict, "")

	want := `<p>A <span class="dict-term" data-dict-term="Security Descriptor">SD</span> controls access.</p>`
	if got != want {
		t.Errorf("\ngot:  %s\nwant: %s", got, want)
	}
}

func TestManualFullName(t *testing.T) {
	dict := loadTestDict(t, testDict)
	input := `<p>The [Security Descriptor] is important.</p>`
	got := transformDictManual(input, dict, "")

	want := `<p>The <span class="dict-term" data-dict-term="Security Descriptor">Security Descriptor</span> is important.</p>`
	if got != want {
		t.Errorf("\ngot:  %s\nwant: %s", got, want)
	}
}

func TestManualWithProduct(t *testing.T) {
	dict := loadTestDict(t, `
[[terms]]
term = "Key"
definition = "Global key."

[[terms]]
term = "Key"
definition = "LCS registry key."
product = "lcs"
`)
	input := `<p>A [Key]{dict:lcs} in the registry.</p>`
	got := transformDictManual(input, dict, "")

	want := `<p>A <span class="dict-term" data-dict-term="Key">Key</span> in the registry.</p>`
	if got != want {
		t.Errorf("\ngot:  %s\nwant: %s", got, want)
	}
}

func TestManualUnresolved(t *testing.T) {
	dict := loadTestDict(t, testDict)
	input := `<p>A [Nonexistent] thing.</p>`
	got := transformDictManual(input, dict, "")

	if got != input {
		t.Errorf("unresolved term should be unchanged\ngot:  %s\nwant: %s", got, input)
	}
}

func TestManualSkipsCodeBlocks(t *testing.T) {
	dict := loadTestDict(t, testDict)
	input := `<p>See <code>[SD]</code> in code.</p>`
	got := transformDictManual(input, dict, "")

	if got != input {
		t.Errorf("should not transform inside code\ngot:  %s\nwant: %s", got, input)
	}
}

func TestManualSkipsPreBlocks(t *testing.T) {
	dict := loadTestDict(t, testDict)
	input := `<pre><code>[SD] in a code block</code></pre>`
	got := transformDictManual(input, dict, "")

	if got != input {
		t.Errorf("should not transform inside pre\ngot:  %s\nwant: %s", got, input)
	}
}

func TestManualMultipleTerms(t *testing.T) {
	dict := loadTestDict(t, testDict)
	input := `<p>An [SD] contains [ACE] entries.</p>`
	got := transformDictManual(input, dict, "")

	want := `<p>An <span class="dict-term" data-dict-term="Security Descriptor">SD</span> contains <span class="dict-term" data-dict-term="Access Control Entry">ACE</span> entries.</p>`
	if got != want {
		t.Errorf("\ngot:  %s\nwant: %s", got, want)
	}
}

func TestManualExistingLinkWins(t *testing.T) {
	dict := loadTestDict(t, testDict)
	// Goldmark converts [SD](url) to <a href="url">SD</a> — no brackets left.
	input := `<p>See <a href="/docs/sd">SD</a> for details.</p>`
	got := transformDictManual(input, dict, "")

	// No [SD] in the text, so nothing to transform.
	if got != input {
		t.Errorf("existing links should not be affected\ngot:  %s\nwant: %s", got, input)
	}
}

// --- Automatic linking ---

func TestAutoLinkBasic(t *testing.T) {
	dict := loadTestDict(t, testDict)
	input := `<p>The SD controls access to the hive.</p>`
	got := transformDictAutoLink(input, dict, "")

	want := `<p>The <span class="dict-term" data-dict-term="Security Descriptor">SD</span> controls access to the <span class="dict-term" data-dict-term="Hive">hive</span>.</p>`
	if got != want {
		t.Errorf("\ngot:  %s\nwant: %s", got, want)
	}
}

func TestAutoLinkCaseInsensitive(t *testing.T) {
	dict := loadTestDict(t, testDict)
	input := `<p>An sd and a HIVE.</p>`
	got := transformDictAutoLink(input, dict, "")

	want := `<p>An <span class="dict-term" data-dict-term="Security Descriptor">sd</span> and a <span class="dict-term" data-dict-term="Hive">HIVE</span>.</p>`
	if got != want {
		t.Errorf("\ngot:  %s\nwant: %s", got, want)
	}
}

func TestAutoLinkWordBoundary(t *testing.T) {
	dict := loadTestDict(t, testDict)
	// "SD" should not match inside "SDcard" or "USD".
	input := `<p>An SDcard and USD are not SDs.</p>`
	got := transformDictAutoLink(input, dict, "")

	// Only "SDs" should match (it's an alias).
	want := `<p>An SDcard and USD are not <span class="dict-term" data-dict-term="Security Descriptor">SDs</span>.</p>`
	if got != want {
		t.Errorf("\ngot:  %s\nwant: %s", got, want)
	}
}

func TestAutoLinkSkipsCode(t *testing.T) {
	dict := loadTestDict(t, testDict)
	input := `<p>See <code>SD</code> in code.</p>`
	got := transformDictAutoLink(input, dict, "")

	if got != input {
		t.Errorf("should not auto-link inside code\ngot:  %s\nwant: %s", got, input)
	}
}

func TestAutoLinkSkipsLinks(t *testing.T) {
	dict := loadTestDict(t, testDict)
	input := `<p>See <a href="/sd">SD documentation</a>.</p>`
	got := transformDictAutoLink(input, dict, "")

	// "SD documentation" is inside <a>, should not be linked.
	if got != input {
		t.Errorf("should not auto-link inside links\ngot:  %s\nwant: %s", got, input)
	}
}

func TestAutoLinkSkipsHeadings(t *testing.T) {
	dict := loadTestDict(t, testDict)
	input := `<h2 id="sd">Security Descriptor</h2><p>The SD is important.</p>`
	got := transformDictAutoLink(input, dict, "")

	// Heading content should not be linked, but paragraph should.
	want := `<h2 id="sd">Security Descriptor</h2><p>The <span class="dict-term" data-dict-term="Security Descriptor">SD</span> is important.</p>`
	if got != want {
		t.Errorf("\ngot:  %s\nwant: %s", got, want)
	}
}

func TestAutoLinkSkipsDictTerms(t *testing.T) {
	dict := loadTestDict(t, testDict)
	// Simulate output from manual pass: SD is already wrapped.
	input := `<p>An <span class="dict-term" data-dict-term="Security Descriptor">SD</span> is here.</p>`
	got := transformDictAutoLink(input, dict, "")

	// "SD" inside the existing dict-term span should not be double-wrapped.
	if got != input {
		t.Errorf("should not double-wrap dict terms\ngot:  %s\nwant: %s", got, input)
	}
}

func TestAutoLinkLongerFormFirst(t *testing.T) {
	dict := loadTestDict(t, testDict)
	input := `<p>A Security Descriptor protects resources.</p>`
	got := transformDictAutoLink(input, dict, "")

	// Should match "Security Descriptor" as a whole, not "Security" separately.
	want := `<p>A <span class="dict-term" data-dict-term="Security Descriptor">Security Descriptor</span> protects resources.</p>`
	if got != want {
		t.Errorf("\ngot:  %s\nwant: %s", got, want)
	}
}

func TestAutoLinkEveryOccurrence(t *testing.T) {
	dict := loadTestDict(t, testDict)
	input := `<p>First SD here. Second SD there.</p>`
	got := transformDictAutoLink(input, dict, "")

	want := `<p>First <span class="dict-term" data-dict-term="Security Descriptor">SD</span> here. Second <span class="dict-term" data-dict-term="Security Descriptor">SD</span> there.</p>`
	if got != want {
		t.Errorf("\ngot:  %s\nwant: %s", got, want)
	}
}

func TestAutoLinkEmptyDict(t *testing.T) {
	dict := loadTestDict(t, ``)
	input := `<p>No terms to link.</p>`
	got := transformDictAutoLink(input, dict, "")

	if got != input {
		t.Errorf("empty dict should be a no-op\ngot:  %s\nwant: %s", got, input)
	}
}

// --- Product slug extraction ---

func TestDictProductSlugDocs(t *testing.T) {
	prod := &content.Product{Slug: "peios", Kind: "docs"}
	if got := dictProductSlug(prod); got != "peios" {
		t.Errorf("want 'peios', got %q", got)
	}
}

func TestDictProductSlugSpec(t *testing.T) {
	prod := &content.Product{Slug: "spec/kacs/v0.22", Kind: "spec"}
	if got := dictProductSlug(prod); got != "kacs" {
		t.Errorf("want 'kacs', got %q", got)
	}
}

func TestDictProductSlugNil(t *testing.T) {
	if got := dictProductSlug(nil); got != "" {
		t.Errorf("want empty, got %q", got)
	}
}

// --- walkHTMLText ---

func TestWalkHTMLTextPassthrough(t *testing.T) {
	input := `<p>Hello <strong>world</strong></p>`
	got := walkHTMLText(input, 0, func(s string) string { return s })
	if got != input {
		t.Errorf("identity transform should not change HTML\ngot:  %s\nwant: %s", got, input)
	}
}

func TestWalkHTMLTextTransform(t *testing.T) {
	input := `<p>hello world</p>`
	got := walkHTMLText(input, 0, func(s string) string {
		return "[" + s + "]"
	})
	want := `<p>[hello world]</p>`
	if got != want {
		t.Errorf("\ngot:  %s\nwant: %s", got, want)
	}
}

func TestWalkHTMLTextSkipCode(t *testing.T) {
	input := `<p>before <code>inside</code> after</p>`
	got := walkHTMLText(input, skipCode, func(s string) string {
		return "[" + s + "]"
	})
	want := `<p>[before ]<code>inside</code>[ after]</p>`
	if got != want {
		t.Errorf("\ngot:  %s\nwant: %s", got, want)
	}
}

func TestWalkHTMLTextSkipNestedCode(t *testing.T) {
	input := `<pre><code>inside code</code></pre>`
	got := walkHTMLText(input, skipCode, func(s string) string {
		return "[" + s + "]"
	})
	if got != input {
		t.Errorf("text inside pre>code should not be transformed\ngot:  %s\nwant: %s", got, input)
	}
}

// --- dictionary.json generation ---

func TestBuildDictionaryJSON(t *testing.T) {
	dict := loadTestDict(t, `
[[terms]]
term = "Security Descriptor"
abbr = "SD"
aliases = ["SDs"]
definition = "A data structure containing security information."
body = "Longer explanation here."
category = "KACS"
product = "kacs"
etymology = "From the Windows NT security model"

[[terms.refs]]
label = "What is an SD?"
path = "peios/access-control/security-descriptors"

[[terms]]
term = "Hive"
definition = "A top-level registry namespace."
category = "LCS"
`)

	outDir := t.TempDir()
	if err := buildDictionaryJSON(dict, &content.Site{}, &config.Config{}, outDir); err != nil {
		t.Fatal(err)
	}

	data, err := os.ReadFile(filepath.Join(outDir, "dictionary.json"))
	if err != nil {
		t.Fatal(err)
	}

	// Parse and verify structure.
	var entries []dictJSONEntry
	if err := json.Unmarshal(data, &entries); err != nil {
		t.Fatalf("invalid JSON: %v", err)
	}

	if len(entries) != 2 {
		t.Fatalf("expected 2 entries, got %d", len(entries))
	}

	// Terms are sorted alphabetically by dictionary.Load.
	if entries[0].Term != "Hive" {
		t.Errorf("first term: want 'Hive', got %q", entries[0].Term)
	}

	sd := entries[1]
	if sd.Term != "Security Descriptor" {
		t.Fatalf("second term: want 'Security Descriptor', got %q", sd.Term)
	}
	if sd.Abbr != "SD" {
		t.Errorf("abbr: want 'SD', got %q", sd.Abbr)
	}
	if len(sd.Aliases) != 1 || sd.Aliases[0] != "SDs" {
		t.Errorf("aliases: got %v", sd.Aliases)
	}
	if sd.Category != "KACS" {
		t.Errorf("category: want 'KACS', got %q", sd.Category)
	}
	if sd.Product != "kacs" {
		t.Errorf("product: want 'kacs', got %q", sd.Product)
	}
	if sd.Body != "Longer explanation here." {
		t.Errorf("body: got %q", sd.Body)
	}
	if sd.Etymology != "From the Windows NT security model" {
		t.Errorf("etymology: got %q", sd.Etymology)
	}
	if len(sd.Refs) != 1 || sd.Refs[0].Label != "What is an SD?" {
		t.Errorf("refs: got %v", sd.Refs)
	}
}

func TestBuildDictionaryJSONOmitsEmpty(t *testing.T) {
	dict := loadTestDict(t, `
[[terms]]
term = "Simple"
definition = "A minimal term."
`)

	outDir := t.TempDir()
	if err := buildDictionaryJSON(dict, &content.Site{}, &config.Config{}, outDir); err != nil {
		t.Fatal(err)
	}

	data, err := os.ReadFile(filepath.Join(outDir, "dictionary.json"))
	if err != nil {
		t.Fatal(err)
	}

	// Verify omitempty: optional fields should not appear in output.
	s := string(data)
	for _, field := range []string{`"abbr"`, `"aliases"`, `"body"`, `"category"`, `"product"`, `"refs"`, `"etymology"`} {
		if strings.Contains(s, field) {
			t.Errorf("expected %s to be omitted for minimal term, but found it in output", field)
		}
	}
}

func TestBuildDictionaryJSONEmptyDict(t *testing.T) {
	dict := loadTestDict(t, ``)
	outDir := t.TempDir()

	if err := buildDictionaryJSON(dict, &content.Site{}, &config.Config{}, outDir); err != nil {
		t.Fatal(err)
	}

	// Should not create the file at all.
	_, err := os.Stat(filepath.Join(outDir, "dictionary.json"))
	if !os.IsNotExist(err) {
		t.Error("expected no dictionary.json for empty dictionary")
	}
}
