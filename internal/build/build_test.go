package build

import (
	"testing"
)

func TestAssignHeadingSectionNums(t *testing.T) {
	tests := []struct {
		name     string
		pageBase string
		headings []heading
		want     []string // expected SectionNum for each heading
	}{
		{
			name:     "h2 only",
			pageBase: "3.2",
			headings: []heading{
				{Level: 2}, {Level: 2}, {Level: 2},
			},
			want: []string{"3.2.1", "3.2.2", "3.2.3"},
		},
		{
			name:     "h2 and h3",
			pageBase: "3.2",
			headings: []heading{
				{Level: 2}, {Level: 3}, {Level: 3}, {Level: 2}, {Level: 3},
			},
			want: []string{"3.2.1", "3.2.1.1", "3.2.1.2", "3.2.2", "3.2.2.1"},
		},
		{
			name:     "h2 h3 h4 — three levels deep",
			pageBase: "3.2",
			headings: []heading{
				{Level: 2}, {Level: 3}, {Level: 4}, {Level: 4}, {Level: 3},
			},
			want: []string{"3.2.1", "3.2.1.1", "3.2.1.1.1", "3.2.1.1.2", "3.2.1.2"},
		},
		{
			name:     "h4 resets when parent h3 increments",
			pageBase: "1.1",
			headings: []heading{
				{Level: 2}, {Level: 3}, {Level: 4}, {Level: 3}, {Level: 4},
			},
			want: []string{"1.1.1", "1.1.1.1", "1.1.1.1.1", "1.1.1.2", "1.1.1.2.1"},
		},
		{
			name:     "five levels deep",
			pageBase: "2.1",
			headings: []heading{
				{Level: 2}, {Level: 3}, {Level: 4}, {Level: 5}, {Level: 6},
			},
			want: []string{"2.1.1", "2.1.1.1", "2.1.1.1.1", "2.1.1.1.1.1", "2.1.1.1.1.1.1"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := assignHeadingSectionNums(tt.headings, tt.pageBase)
			if len(result) != len(tt.want) {
				t.Fatalf("got %d headings, want %d", len(result), len(tt.want))
			}
			for i, want := range tt.want {
				if result[i].SectionNum != want {
					t.Errorf("heading[%d].SectionNum = %q, want %q", i, result[i].SectionNum, want)
				}
			}
		})
	}
}

func TestExtractHeadingsDepth(t *testing.T) {
	html := `<h2 id="a">Alpha</h2><h3 id="b">Beta</h3><h4 id="c">Gamma</h4><h5 id="d">Delta</h5><h6 id="e">Epsilon</h6>`
	headings := extractHeadings(html)
	if len(headings) != 5 {
		t.Fatalf("got %d headings, want 5", len(headings))
	}
	wantLevels := []int{2, 3, 4, 5, 6}
	wantTexts := []string{"Alpha", "Beta", "Gamma", "Delta", "Epsilon"}
	for i := range headings {
		if headings[i].Level != wantLevels[i] {
			t.Errorf("heading[%d].Level = %d, want %d", i, headings[i].Level, wantLevels[i])
		}
		if headings[i].Text != wantTexts[i] {
			t.Errorf("heading[%d].Text = %q, want %q", i, headings[i].Text, wantTexts[i])
		}
	}
}
