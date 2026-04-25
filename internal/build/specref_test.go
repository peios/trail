package build

import (
	"testing"

	"github.com/peios/trail/internal/content"
)

func TestBuildRefIndex(t *testing.T) {
	site := &content.Site{
		Products: []*content.Product{
			{
				Kind:        "spec",
				SpecID:      "psd-004",
				Slug:        "spec/psd-004/v0.20",
				VersionSlug: "v0.20",
				Categories: []*content.Category{
					{
						SectionNum: "3",
						Pages: []*content.Page{
							{Slug: "spec/psd-004/v0.20/security/overview", SectionNum: "3.1", Body: []byte("## First\n\nText\n\n## Second\n\nMore\n")},
							{Slug: "spec/psd-004/v0.20/security/acls", SectionNum: "3.2", Body: []byte("## Overview\n\n### By Type\n\n### By Position\n")},
						},
					},
				},
				Pages: []*content.Page{
					{Slug: "spec/psd-004/v0.20/security/overview", SectionNum: "3.1", Body: []byte("## First\n\nText\n\n## Second\n\nMore\n")},
					{Slug: "spec/psd-004/v0.20/security/acls", SectionNum: "3.2", Body: []byte("## Overview\n\n### By Type\n\n### By Position\n")},
				},
			},
		},
	}

	idx := BuildRefIndex(site)

	// Chapter-level.
	target, ok := idx.Resolve("psd-004", "v0.20", "3")
	if !ok {
		t.Fatal("expected chapter 3 to resolve")
	}
	if target.PageSlug != "spec/psd-004/v0.20/security/overview" {
		t.Errorf("chapter 3: want overview page, got %q", target.PageSlug)
	}

	// Page-level.
	target, ok = idx.Resolve("psd-004", "v0.20", "3.2")
	if !ok {
		t.Fatal("expected §3.2 to resolve")
	}
	if target.PageSlug != "spec/psd-004/v0.20/security/acls" {
		t.Errorf("§3.2: want acls page, got %q", target.PageSlug)
	}

	// Subsection-level from heading scan.
	target, ok = idx.Resolve("psd-004", "v0.20", "3.2.1")
	if !ok {
		t.Fatal("expected §3.2.1 to resolve")
	}
	if target.Anchor != "3.2.1" {
		t.Errorf("§3.2.1: want anchor %q, got %q", "3.2.1", target.Anchor)
	}

	// Deeper subsection.
	target, ok = idx.Resolve("psd-004", "v0.20", "3.2.1.1")
	if !ok {
		t.Fatal("expected §3.2.1.1 to resolve")
	}
	if target.Anchor != "3.2.1.1" {
		t.Errorf("§3.2.1.1: want anchor %q, got %q", "3.2.1.1", target.Anchor)
	}

	// Non-existent.
	_, ok = idx.Resolve("psd-004", "v0.20", "99.99")
	if ok {
		t.Error("expected §99.99 to not resolve")
	}
}

func TestVersionResolution(t *testing.T) {
	site := &content.Site{
		Products: []*content.Product{
			{Kind: "spec", SpecID: "psd-004", Slug: "spec/psd-004/v0.20", VersionSlug: "v0.20",
				Pages: []*content.Page{{Slug: "spec/psd-004/v0.20/intro", SectionNum: "1.1", Body: []byte("")}}},
			{Kind: "spec", SpecID: "psd-004", Slug: "spec/psd-004/v0.22", VersionSlug: "v0.22",
				Pages: []*content.Page{{Slug: "spec/psd-004/v0.22/intro", SectionNum: "1.1", Body: []byte("")}}},
			{Kind: "spec", SpecID: "psd-004", Slug: "spec/psd-004/v0.56.1", VersionSlug: "v0.56.1",
				Pages: []*content.Page{{Slug: "spec/psd-004/v0.56.1/intro", SectionNum: "1.1", Body: []byte("")}}},
		},
	}

	idx := BuildRefIndex(site)

	tests := []struct {
		from string
		want string
	}{
		{"v0.20", "v0.20"},
		{"v0.22", "v0.22"},
		{"v0.25", "v0.22"},   // highest ≤ v0.25
		{"v0.56.1", "v0.56.1"},
		{"v1.0", "v0.56.1"},  // highest ≤ v1.0
		{"v0.19", ""},         // none ≤ v0.19
	}

	for _, tt := range tests {
		got := idx.ResolveVersion("psd-004", tt.from)
		if got != tt.want {
			t.Errorf("ResolveVersion(psd-004, %q) = %q, want %q", tt.from, got, tt.want)
		}
	}

	// Unknown spec.
	if got := idx.ResolveVersion("unknown", "v0.20"); got != "" {
		t.Errorf("unknown spec: want empty, got %q", got)
	}
}

func TestCompareVersions(t *testing.T) {
	tests := []struct {
		a, b string
		want int
	}{
		{"v0.20", "v0.22", -1},
		{"v0.22", "v0.20", 1},
		{"v0.20", "v0.20", 0},
		{"v0.56.1", "v0.56.1", 0},
		{"v0.56", "v0.56.1", -1},
		{"v1.0", "v0.99", 1},
	}
	for _, tt := range tests {
		got := compareVersions(tt.a, tt.b)
		if got != tt.want {
			t.Errorf("compareVersions(%q, %q) = %d, want %d", tt.a, tt.b, got, tt.want)
		}
	}
}

func TestExtractMarkdownHeadings(t *testing.T) {
	body := []byte("## Overview\n\nText.\n\n## Ordering\n\n### By Type\n\n### By Position\n\n#### Detailed\n")
	headings := extractMarkdownHeadings(body, "3.2")

	want := []struct {
		sectionNum string
		level      int
	}{
		{"3.2.1", 2},
		{"3.2.2", 2},
		{"3.2.2.1", 3},
		{"3.2.2.2", 3},
		{"3.2.2.2.1", 4},
	}

	if len(headings) != len(want) {
		t.Fatalf("got %d headings, want %d", len(headings), len(want))
	}
	for i, w := range want {
		if headings[i].sectionNum != w.sectionNum {
			t.Errorf("heading[%d].sectionNum = %q, want %q", i, headings[i].sectionNum, w.sectionNum)
		}
		if headings[i].level != w.level {
			t.Errorf("heading[%d].level = %d, want %d", i, headings[i].level, w.level)
		}
	}
}

func TestTransformSpecRefsCrossSpec(t *testing.T) {
	site := &content.Site{
		Products: []*content.Product{
			{Kind: "spec", SpecID: "psd-004", Slug: "spec/psd-004/v0.20", VersionSlug: "v0.20",
				Categories: []*content.Category{{SectionNum: "3", Pages: []*content.Page{
					{Slug: "spec/psd-004/v0.20/security/acls", SectionNum: "3.2", Body: []byte("## Overview\n")},
				}}},
				Pages: []*content.Page{
					{Slug: "spec/psd-004/v0.20/security/acls", SectionNum: "3.2", Body: []byte("## Overview\n")},
				}},
		},
	}
	idx := BuildRefIndex(site)

	// Cross-spec reference with section.
	input := `<p>See psd-004 §3.2 for details.</p>`
	got := transformSpecRefs(input, idx, "psd-007", "v0.20", "/")
	want := `<p>See <a href="/spec/psd-004/v0.20/security/acls/" class="spec-ref">psd-004 §3.2</a> for details.</p>`
	if got != want {
		t.Errorf("cross-spec §3.2:\ngot:  %s\nwant: %s", got, want)
	}

	// Uppercase spec ID (PSD-004 in prose, psd-004 from filesystem).
	input = `<p>See PSD-004 §3.2 for details.</p>`
	got = transformSpecRefs(input, idx, "psd-007", "v0.20", "/")
	want = `<p>See <a href="/spec/psd-004/v0.20/security/acls/" class="spec-ref">PSD-004 §3.2</a> for details.</p>`
	if got != want {
		t.Errorf("uppercase cross-spec:\ngot:  %s\nwant: %s", got, want)
	}

	// Bare spec ID reference.
	input = `<p>As defined in psd-004.</p>`
	got = transformSpecRefs(input, idx, "psd-007", "v0.20", "/")
	want = `<p>As defined in <a href="/spec/psd-004/v0.20/" class="spec-ref">psd-004</a>.</p>`
	if got != want {
		t.Errorf("bare spec ID:\ngot:  %s\nwant: %s", got, want)
	}

	// Unresolvable section — left as plain text.
	input = `<p>See psd-004 §99.1 for details.</p>`
	got = transformSpecRefs(input, idx, "psd-007", "v0.20", "/")
	if got != input {
		t.Errorf("unresolvable: expected no change, got: %s", got)
	}

	// Unknown spec ID — left as plain text.
	input = `<p>See unknown-001 §1.1 for details.</p>`
	got = transformSpecRefs(input, idx, "psd-007", "v0.20", "/")
	if got != input {
		t.Errorf("unknown spec: expected no change, got: %s", got)
	}
}

func TestTransformSpecRefsWithinSpec(t *testing.T) {
	site := &content.Site{
		Products: []*content.Product{
			{Kind: "spec", SpecID: "psd-004", Slug: "spec/psd-004/v0.20", VersionSlug: "v0.20",
				Pages: []*content.Page{
					{Slug: "spec/psd-004/v0.20/security/acls", SectionNum: "3.2", Body: []byte("## Overview\n")},
				}},
		},
	}
	idx := BuildRefIndex(site)

	// Within-spec § reference.
	input := `<p>See §3.2 for details.</p>`
	got := transformSpecRefs(input, idx, "psd-004", "v0.20", "/")
	want := `<p>See <a href="/spec/psd-004/v0.20/security/acls/" class="spec-ref">§3.2</a> for details.</p>`
	if got != want {
		t.Errorf("within-spec:\ngot:  %s\nwant: %s", got, want)
	}

	// Unresolvable within-spec — left as plain text.
	input = `<p>See §99.1 for details.</p>`
	got = transformSpecRefs(input, idx, "psd-004", "v0.20", "/")
	if got != input {
		t.Errorf("unresolvable within-spec: expected no change, got: %s", got)
	}
}

func TestTransformSpecRefsSkipsCode(t *testing.T) {
	site := &content.Site{
		Products: []*content.Product{
			{Kind: "spec", SpecID: "psd-004", Slug: "spec/psd-004/v0.20", VersionSlug: "v0.20",
				Pages: []*content.Page{
					{Slug: "spec/psd-004/v0.20/intro/scope", SectionNum: "1.1", Body: []byte("")},
				}},
		},
	}
	idx := BuildRefIndex(site)

	// Inside <code> — should not be linked.
	input := `<code>psd-004 §1.1</code>`
	got := transformSpecRefs(input, idx, "psd-007", "v0.20", "/")
	if got != input {
		t.Errorf("inside code: expected no change, got: %s", got)
	}

	// Inside <a> — should not be linked.
	input = `<a href="#">psd-004 §1.1</a>`
	got = transformSpecRefs(input, idx, "psd-007", "v0.20", "/")
	if got != input {
		t.Errorf("inside link: expected no change, got: %s", got)
	}
}

func TestTransformSpecRefsClauseSuffix(t *testing.T) {
	site := &content.Site{
		Products: []*content.Product{
			{Kind: "spec", SpecID: "psd-004", Slug: "spec/psd-004/v0.20", VersionSlug: "v0.20",
				Pages: []*content.Page{
					{Slug: "spec/psd-004/v0.20/security/acls", SectionNum: "3.2", Body: []byte("## Overview\n")},
				}},
		},
	}
	idx := BuildRefIndex(site)

	// Clause suffix — links to the section (clause anchors not yet implemented).
	input := `<p>Per psd-004 §3.2.1(4) the ACL must exist.</p>`
	got := transformSpecRefs(input, idx, "psd-007", "v0.20", "/")
	want := `<p>Per <a href="/spec/psd-004/v0.20/security/acls/#3.2.1" class="spec-ref">psd-004 §3.2.1(4)</a> the ACL must exist.</p>`
	if got != want {
		t.Errorf("clause suffix:\ngot:  %s\nwant: %s", got, want)
	}
}
