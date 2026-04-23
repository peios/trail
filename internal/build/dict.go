package build

import (
	"encoding/json"
	"fmt"
	"html"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"

	"github.com/peios/trail/internal/config"
	"github.com/peios/trail/internal/content"
	"github.com/peios/trail/internal/dictionary"
	"github.com/peios/trail/internal/theme"
)

// manualDictRe matches [Term] and [Term]{dict:product} in rendered HTML text.
// Only matches literal bracket text left over after Goldmark resolves reference links.
var manualDictRe = regexp.MustCompile(`\[([^\[\]]+)\](?:\{dict:([^}]+)\})?`)

// skipRule controls which elements are skipped during text segment transformation.
type skipRule int

const (
	skipCode      skipRule = 1 << iota // <code>, <pre>
	skipLinks                          // <a>
	skipHeadings                       // <h1>–<h6>
	skipDictTerms                      // <span class="dict-term">
)

// dictProductSlug extracts the base product slug for dictionary resolution.
// Spec products have slugs like "spec/kacs/v0.20" — this returns "kacs".
func dictProductSlug(prod *content.Product) string {
	if prod == nil {
		return ""
	}
	if prod.Kind == "spec" {
		parts := strings.Split(prod.Slug, "/")
		if len(parts) >= 2 {
			return parts[1]
		}
	}
	return prod.Slug
}

// dictTermSpan wraps matched text in a dictionary term span element.
func dictTermSpan(matchedText, canonicalTerm string) string {
	return `<span class="dict-term" data-dict-term="` +
		html.EscapeString(canonicalTerm) + `">` +
		matchedText + `</span>`
}

// transformDictManual replaces [Term] and [Term]{dict:product} markup with
// dictionary term spans. Only text outside code blocks is transformed.
// Unresolved terms are left unchanged.
func transformDictManual(htmlStr string, dict *dictionary.Dictionary, defaultProduct string) string {
	return walkHTMLText(htmlStr, skipCode, func(text string) string {
		return manualDictRe.ReplaceAllStringFunc(text, func(match string) string {
			sub := manualDictRe.FindStringSubmatch(match)
			if len(sub) < 2 {
				return match
			}
			termText := sub[1]
			product := defaultProduct
			if len(sub) >= 3 && sub[2] != "" {
				product = sub[2]
			}

			resolved := dict.Resolve(termText, product)
			if resolved == nil {
				return match
			}

			return dictTermSpan(termText, resolved.Term)
		})
	})
}

// transformDictAutoLink wraps every occurrence of a dictionary term in the
// rendered HTML with a dictionary term span. Skips code blocks, existing links,
// headings, and already-linked dictionary terms.
func transformDictAutoLink(htmlStr string, dict *dictionary.Dictionary, productSlug string) string {
	re := buildAutoLinkPattern(dict, productSlug)
	if re == nil {
		return htmlStr
	}

	skip := skipCode | skipLinks | skipHeadings | skipDictTerms
	return walkHTMLText(htmlStr, skip, func(text string) string {
		return re.ReplaceAllStringFunc(text, func(match string) string {
			resolved := dict.Resolve(match, productSlug)
			if resolved == nil {
				return match
			}
			return dictTermSpan(match, resolved.Term)
		})
	})
}

// buildAutoLinkPattern builds a case-insensitive regex that matches any
// dictionary term form visible in the given product context. Returns nil
// if no forms are available. Longer forms are tried first.
func buildAutoLinkPattern(dict *dictionary.Dictionary, productSlug string) *regexp.Regexp {
	forms := dict.VisibleForms(productSlug)
	if len(forms) == 0 {
		return nil
	}

	// Longest first so "Security Descriptor" matches before "Security".
	sort.Slice(forms, func(i, j int) bool {
		return len(forms[i]) > len(forms[j])
	})

	parts := make([]string, len(forms))
	for i, form := range forms {
		parts[i] = regexp.QuoteMeta(form)
	}

	pattern := `(?i)\b(` + strings.Join(parts, "|") + `)\b`
	return regexp.MustCompile(pattern)
}

// walkHTMLText walks rendered HTML, calling transform on text segments
// that are not inside any of the element types specified by skip.
func walkHTMLText(htmlStr string, skip skipRule, transform func(string) string) string {
	var result strings.Builder
	result.Grow(len(htmlStr) + len(htmlStr)/10)

	i := 0
	var codeDepth, linkDepth, headingDepth, dictDepth int

	for i < len(htmlStr) {
		if htmlStr[i] != '<' {
			// Text segment: find the next tag.
			nextTag := strings.IndexByte(htmlStr[i:], '<')
			if nextTag == -1 {
				nextTag = len(htmlStr) - i
			}
			text := htmlStr[i : i+nextTag]

			if codeDepth > 0 || linkDepth > 0 || headingDepth > 0 || dictDepth > 0 {
				result.WriteString(text)
			} else {
				result.WriteString(transform(text))
			}
			i += nextTag
			continue
		}

		// Tag: find end.
		end := strings.IndexByte(htmlStr[i:], '>')
		if end == -1 {
			result.WriteString(htmlStr[i:])
			break
		}
		tag := htmlStr[i : i+end+1]

		// Track element depths for each skip rule.
		if skip&skipCode != 0 {
			if strings.HasPrefix(tag, "<code") || strings.HasPrefix(tag, "<pre") {
				codeDepth++
			} else if strings.HasPrefix(tag, "</code") || strings.HasPrefix(tag, "</pre") {
				if codeDepth > 0 {
					codeDepth--
				}
			}
		}
		if skip&skipLinks != 0 {
			if strings.HasPrefix(tag, "<a ") || tag == "<a>" {
				linkDepth++
			} else if strings.HasPrefix(tag, "</a") {
				if linkDepth > 0 {
					linkDepth--
				}
			}
		}
		if skip&skipHeadings != 0 {
			if isOpenHeading(tag) {
				headingDepth++
			} else if isCloseHeading(tag) {
				if headingDepth > 0 {
					headingDepth--
				}
			}
		}
		if skip&skipDictTerms != 0 {
			if strings.HasPrefix(tag, `<span class="dict-term"`) {
				dictDepth++
			} else if strings.HasPrefix(tag, "</span") && dictDepth > 0 {
				dictDepth--
			}
		}

		result.WriteString(tag)
		i += end + 1
	}

	return result.String()
}

func isOpenHeading(tag string) bool {
	if len(tag) < 4 {
		return false
	}
	return tag[0] == '<' && tag[1] == 'h' &&
		tag[2] >= '1' && tag[2] <= '6' &&
		(tag[3] == ' ' || tag[3] == '>')
}

func isCloseHeading(tag string) bool {
	if len(tag) < 5 {
		return false
	}
	return tag[0] == '<' && tag[1] == '/' && tag[2] == 'h' &&
		tag[3] >= '1' && tag[3] <= '6' && tag[4] == '>'
}

// --- dictionary browse page ---

type dictBrowseData struct {
	Site       siteData
	Terms      []dictBrowseTerm
	Letters    []string
	ByLetter   []dictLetterGroup
	ByCategory []dictCategoryGroup
}

type dictBrowseTerm struct {
	Term       string
	Abbr       string
	Slug       string
	Letter     string
	Definition string
	Category   string
	Product    string
	Aliases    []string
	AliasText  string
	Refs       []dictBrowseRef
	Etymology  string
	AppearsOn  []dictPageRef
}

type dictBrowseRef struct {
	Label string
	URL   string
}

type dictPageRef struct {
	Title string
	URL   string
}

type dictLetterGroup struct {
	Letter string
	Terms  []dictBrowseTerm
}

type dictCategoryGroup struct {
	Category string
	Terms    []dictBrowseTerm
}

func buildDictionaryPage(tmpl *theme.Templates, site *content.Site, cfg *config.Config, dict *dictionary.Dictionary, outDir string) error {
	if dict.IsEmpty() {
		return nil
	}

	dictDir := filepath.Join(outDir, "dictionary")
	if err := os.MkdirAll(dictDir, 0o755); err != nil {
		return err
	}

	f, err := os.Create(filepath.Join(dictDir, "index.html"))
	if err != nil {
		return err
	}
	defer f.Close()

	basePath := cfg.BasePath()
	usage := buildReverseIndex(dict, site, basePath)

	var terms []dictBrowseTerm
	letterSet := make(map[string]bool)
	categorySet := make(map[string]bool)

	for _, t := range dict.Terms {
		letter := strings.ToUpper(t.Term[:1])
		letterSet[letter] = true
		if t.Category != "" {
			categorySet[t.Category] = true
		}

		var refs []dictBrowseRef
		for _, r := range t.Refs {
			refs = append(refs, dictBrowseRef{
				Label: r.Label,
				URL:   basePath + r.Path + "/",
			})
		}

		slug := strings.ToLower(t.Term)
		slug = strings.ReplaceAll(slug, " ", "-")

		bt := dictBrowseTerm{
			Term:       t.Term,
			Abbr:       t.Abbr,
			Slug:       slug,
			Letter:     letter,
			Definition: t.Definition,
			Category:   t.Category,
			Product:    t.Product,
			Aliases:    t.Aliases,
			AliasText:  strings.Join(t.Aliases, ", "),
			Refs:       refs,
			Etymology:  t.Etymology,
			AppearsOn:  usage[t.Term],
		}
		terms = append(terms, bt)
	}

	// Build sorted letter list.
	var letters []string
	for l := range letterSet {
		letters = append(letters, l)
	}
	sort.Strings(letters)

	// Group by letter.
	letterMap := make(map[string][]dictBrowseTerm)
	for _, t := range terms {
		letterMap[t.Letter] = append(letterMap[t.Letter], t)
	}
	var byLetter []dictLetterGroup
	for _, l := range letters {
		byLetter = append(byLetter, dictLetterGroup{Letter: l, Terms: letterMap[l]})
	}

	// Group by category.
	var categories []string
	for c := range categorySet {
		categories = append(categories, c)
	}
	sort.Strings(categories)
	catMap := make(map[string][]dictBrowseTerm)
	for _, t := range terms {
		cat := t.Category
		if cat == "" {
			cat = "Uncategorized"
		}
		catMap[cat] = append(catMap[cat], t)
	}
	var byCategory []dictCategoryGroup
	for _, c := range categories {
		byCategory = append(byCategory, dictCategoryGroup{Category: c, Terms: catMap[c]})
	}
	if uncategorized, ok := catMap["Uncategorized"]; ok {
		byCategory = append(byCategory, dictCategoryGroup{Category: "Uncategorized", Terms: uncategorized})
	}

	data := dictBrowseData{
		Site:       newSiteData(site, cfg),
		Terms:      terms,
		Letters:    letters,
		ByLetter:   byLetter,
		ByCategory: byCategory,
	}

	return tmpl.Dictionary.ExecuteTemplate(f, "base", data)
}

// buildReverseIndex scans all page bodies for dictionary term occurrences
// and returns a map of canonical term name → pages where it appears.
func buildReverseIndex(dict *dictionary.Dictionary, site *content.Site, basePath string) map[string][]dictPageRef {
	result := make(map[string][]dictPageRef)

	for _, t := range dict.Terms {
		forms := t.AllForms()
		// Build a regex for this term's forms.
		parts := make([]string, len(forms))
		for i, f := range forms {
			parts[i] = regexp.QuoteMeta(f)
		}
		pattern := `(?i)\b(` + strings.Join(parts, "|") + `)\b`
		re := regexp.MustCompile(pattern)

		for _, page := range site.Pages {
			if re.Match(page.Body) {
				result[t.Term] = append(result[t.Term], dictPageRef{
					Title: page.Title,
					URL:   basePath + page.Slug + "/",
				})
			}
		}
	}

	return result
}

// --- dictionary.json generation ---

// dictJSONEntry is the JSON representation of a dictionary term.
type dictJSONEntry struct {
	Term       string            `json:"term"`
	Abbr       string            `json:"abbr,omitempty"`
	Aliases    []string          `json:"aliases,omitempty"`
	Definition string            `json:"definition"`
	Body       string            `json:"body,omitempty"`
	Category   string            `json:"category,omitempty"`
	Product    string            `json:"product,omitempty"`
	Refs       []dictJSONRef     `json:"refs,omitempty"`
	Etymology  string            `json:"etymology,omitempty"`
	AppearsOn  []dictJSONPageRef `json:"appears_on,omitempty"`
}

type dictJSONRef struct {
	Label string `json:"label"`
	Path  string `json:"path"`
}

type dictJSONPageRef struct {
	Title string `json:"title"`
	Slug  string `json:"slug"`
}

func buildDictionaryJSON(dict *dictionary.Dictionary, site *content.Site, cfg *config.Config, outDir string) error {
	if dict.IsEmpty() {
		return nil
	}

	usage := buildReverseIndex(dict, site, "")

	entries := make([]dictJSONEntry, len(dict.Terms))
	for i, t := range dict.Terms {
		var refs []dictJSONRef
		for _, r := range t.Refs {
			refs = append(refs, dictJSONRef{Label: r.Label, Path: r.Path})
		}
		var appearsOn []dictJSONPageRef
		for _, p := range usage[t.Term] {
			appearsOn = append(appearsOn, dictJSONPageRef{
				Title: p.Title,
				Slug:  strings.TrimSuffix(strings.TrimPrefix(p.URL, "/"), "/"),
			})
		}
		entries[i] = dictJSONEntry{
			Term:       t.Term,
			Abbr:       t.Abbr,
			Aliases:    t.Aliases,
			Definition: t.Definition,
			Body:       t.Body,
			Category:   t.Category,
			Product:    t.Product,
			Refs:       refs,
			Etymology:  t.Etymology,
			AppearsOn:  appearsOn,
		}
	}

	data, err := json.MarshalIndent(entries, "", "  ")
	if err != nil {
		return fmt.Errorf("marshalling dictionary JSON: %w", err)
	}

	return os.WriteFile(filepath.Join(outDir, "dictionary.json"), data, 0o644)
}
