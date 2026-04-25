package build

import (
	"fmt"
	"regexp"
	"sort"
	"strconv"
	"strings"

	"github.com/peios/trail/internal/content"
)

// RefTarget is where a section reference points.
type RefTarget struct {
	PageSlug string // full slug, e.g. "spec/psd-004/v0.22/security-descriptors/acls"
	Anchor   string // heading ID fragment, e.g. "3.2.1" (empty for page-level)
}

// URL returns the full URL path for this target.
func (t RefTarget) URL(basePath string) string {
	u := basePath + t.PageSlug + "/"
	if t.Anchor != "" {
		u += "#" + t.Anchor
	}
	return u
}

// SpecRefIndex maps spec IDs → versions → section numbers → targets.
type SpecRefIndex struct {
	// specs[specID][versionName][sectionNum] = target
	specs map[string]map[string]map[string]RefTarget
	// sortedVersions[specID] = ["v0.20", "v0.22"] sorted ascending
	sortedVersions map[string][]string
	// crossRefRe matches known spec IDs with optional § references.
	// Built once from the set of known spec IDs.
	crossRefRe *regexp.Regexp
	// withinRefRe matches bare §N.M.K(C) references.
	withinRefRe *regexp.Regexp
}

// BuildRefIndex builds a cross-reference index from all spec products in the site.
func BuildRefIndex(site *content.Site) *SpecRefIndex {
	idx := &SpecRefIndex{
		specs:          make(map[string]map[string]map[string]RefTarget),
		sortedVersions: make(map[string][]string),
	}

	for _, prod := range site.Products {
		if prod.Kind != "spec" || prod.SpecID == "" || prod.VersionSlug == "" {
			continue
		}

		specID := prod.SpecID
		version := prod.VersionSlug

		// Ensure maps exist.
		if idx.specs[specID] == nil {
			idx.specs[specID] = make(map[string]map[string]RefTarget)
		}
		if idx.specs[specID][version] == nil {
			idx.specs[specID][version] = make(map[string]RefTarget)
		}

		sections := idx.specs[specID][version]

		// Chapter-level entries from categories.
		for _, cat := range prod.Categories {
			if cat.SectionNum == "" {
				continue
			}
			if len(cat.Pages) > 0 {
				sections[cat.SectionNum] = RefTarget{PageSlug: cat.Pages[0].Slug}
			}
		}

		// Page-level and subsection-level entries.
		for _, page := range prod.Pages {
			if page.SectionNum == "" {
				continue
			}
			sections[page.SectionNum] = RefTarget{PageSlug: page.Slug}

			// Scan raw markdown for subsection headings.
			for _, sub := range extractMarkdownHeadings(page.Body, page.SectionNum) {
				sections[sub.sectionNum] = RefTarget{
					PageSlug: page.Slug,
					Anchor:   sub.sectionNum,
				}
			}
		}

		// Track versions.
		idx.sortedVersions[specID] = append(idx.sortedVersions[specID], version)
	}

	// Sort and deduplicate version lists.
	for specID := range idx.sortedVersions {
		versions := idx.sortedVersions[specID]
		sort.Slice(versions, func(i, j int) bool {
			return compareVersions(versions[i], versions[j]) < 0
		})
		// Deduplicate (each version product appears once, but just in case).
		deduped := versions[:0]
		seen := make(map[string]bool)
		for _, v := range versions {
			if !seen[v] {
				seen[v] = true
				deduped = append(deduped, v)
			}
		}
		idx.sortedVersions[specID] = deduped
	}

	// Build regexes.
	idx.crossRefRe = buildCrossRefRegex(idx.specs)
	idx.withinRefRe = regexp.MustCompile(`§([\d.]+)(\(\d+\))?`)

	return idx
}

type markdownHeading struct {
	sectionNum string
	level      int
}

// extractMarkdownHeadings scans raw markdown for heading lines and computes
// section numbers relative to the page's base section number.
func extractMarkdownHeadings(body []byte, pageBase string) []markdownHeading {
	lines := strings.Split(string(body), "\n")
	counters := make(map[int]int)
	var headings []markdownHeading

	for _, line := range lines {
		level := markdownHeadingLevel(line)
		if level < 2 {
			continue
		}

		counters[level]++
		for l := level + 1; l <= 6; l++ {
			delete(counters, l)
		}

		num := pageBase
		for l := 2; l <= level; l++ {
			if c, ok := counters[l]; ok {
				num += fmt.Sprintf(".%d", c)
			}
		}

		headings = append(headings, markdownHeading{
			sectionNum: num,
			level:      level,
		})
	}

	return headings
}

// markdownHeadingLevel returns the heading level (2-6) for a markdown line,
// or 0 if the line is not a heading.
func markdownHeadingLevel(line string) int {
	if !strings.HasPrefix(line, "##") {
		return 0
	}
	level := 0
	for _, c := range line {
		if c == '#' {
			level++
		} else {
			break
		}
	}
	if level < 2 || level > 6 {
		return 0
	}
	// Must be followed by a space (ATX heading).
	if len(line) <= level || line[level] != ' ' {
		return 0
	}
	return level
}

// ResolveVersion returns the highest version of specID that is ≤ fromVersion.
// Returns empty string if no suitable version exists.
func (idx *SpecRefIndex) ResolveVersion(specID, fromVersion string) string {
	versions := idx.sortedVersions[specID]
	if len(versions) == 0 {
		return ""
	}

	// Find the highest version ≤ fromVersion.
	best := ""
	for _, v := range versions {
		if compareVersions(v, fromVersion) <= 0 {
			best = v
		}
	}
	return best
}

// Resolve looks up a section number for a spec at a resolved version.
// Returns the target and true if found, zero value and false otherwise.
func (idx *SpecRefIndex) Resolve(specID, version, sectionNum string) (RefTarget, bool) {
	if versionMap, ok := idx.specs[specID]; ok {
		if sections, ok := versionMap[version]; ok {
			if target, ok := sections[sectionNum]; ok {
				return target, true
			}
		}
	}
	return RefTarget{}, false
}

// parseVersion parses "v0.22" or "v0.56.1" into comparable integers.
func parseVersion(s string) (major, minor, patch int) {
	s = strings.TrimPrefix(s, "v")
	parts := strings.SplitN(s, ".", 3)
	if len(parts) >= 1 {
		major, _ = strconv.Atoi(parts[0])
	}
	if len(parts) >= 2 {
		minor, _ = strconv.Atoi(parts[1])
	}
	if len(parts) >= 3 {
		patch, _ = strconv.Atoi(parts[2])
	}
	return
}

// compareVersions returns -1, 0, or 1 comparing two version strings.
func compareVersions(a, b string) int {
	aMaj, aMin, aPat := parseVersion(a)
	bMaj, bMin, bPat := parseVersion(b)
	if aMaj != bMaj {
		return intCmp(aMaj, bMaj)
	}
	if aMin != bMin {
		return intCmp(aMin, bMin)
	}
	return intCmp(aPat, bPat)
}

func intCmp(a, b int) int {
	if a < b {
		return -1
	}
	if a > b {
		return 1
	}
	return 0
}

// buildCrossRefRegex builds a regex that matches any known spec ID, optionally
// followed by a § section reference. Returns nil if there are no spec IDs.
func buildCrossRefRegex(specs map[string]map[string]map[string]RefTarget) *regexp.Regexp {
	if len(specs) == 0 {
		return nil
	}

	// Sort IDs longest first so longer IDs match before shorter prefixes.
	ids := make([]string, 0, len(specs))
	for id := range specs {
		ids = append(ids, id)
	}
	sort.Slice(ids, func(i, j int) bool {
		return len(ids[i]) > len(ids[j])
	})

	escaped := make([]string, len(ids))
	for i, id := range ids {
		escaped[i] = regexp.QuoteMeta(id)
	}

	// Match: spec-id optionally followed by whitespace + §section(clause).
	// Case-insensitive so "PSD-004" in prose matches "psd-004" from the filesystem.
	pattern := `(?i)\b(` + strings.Join(escaped, "|") + `)(\s+§([\d.]+)(\(\d+\))?)?`
	return regexp.MustCompile(pattern)
}

// transformSpecRefs replaces spec cross-references in rendered HTML with links.
// References that cannot be resolved are left as plain text.
func transformSpecRefs(htmlStr string, idx *SpecRefIndex, currentSpecID, currentVersion, basePath string) string {
	if idx == nil {
		return htmlStr
	}

	return walkHTMLText(htmlStr, skipCode|skipLinks|skipHeadings, func(text string) string {
		// First pass: cross-spec references (spec-id + optional §section).
		if idx.crossRefRe != nil {
			text = idx.crossRefRe.ReplaceAllStringFunc(text, func(match string) string {
				sub := idx.crossRefRe.FindStringSubmatch(match)
				if len(sub) < 2 {
					return match
				}

				specID := strings.ToLower(sub[1])
				sectionPart := sub[3] // may be empty

				// Resolve version.
				version := idx.ResolveVersion(specID, currentVersion)
				if version == "" {
					return match
				}

				if sectionPart == "" {
					// Bare spec ID reference — link to the spec's landing page.
					target := basePath + "spec/" + specID + "/" + version + "/"
					return `<a href="` + target + `" class="spec-ref">` + match + `</a>`
				}

				// Section reference — resolve against the index.
				if target, ok := idx.Resolve(specID, version, sectionPart); ok {
					return `<a href="` + target.URL(basePath) + `" class="spec-ref">` + match + `</a>`
				}

				return match
			})
		}

		// Second pass: within-spec bare § references.
		if currentSpecID != "" && currentVersion != "" && idx.withinRefRe != nil {
			text = idx.withinRefRe.ReplaceAllStringFunc(text, func(match string) string {
				sub := idx.withinRefRe.FindStringSubmatch(match)
				if len(sub) < 2 {
					return match
				}

				sectionNum := sub[1]
				if target, ok := idx.Resolve(currentSpecID, currentVersion, sectionNum); ok {
					return `<a href="` + target.URL(basePath) + `" class="spec-ref">` + match + `</a>`
				}

				return match
			})
		}

		return text
	})
}
