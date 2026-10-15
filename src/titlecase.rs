//! Chicago-style title case: the first and last words and every major
//! word capitalized; articles, coordinating conjunctions, prepositions and
//! "as" lowercase between them. Titles derived from slugs use it, and the
//! style lint checks authored titles against it.

/// Words a title keeps lowercase unless they are its first or last word.
/// Words that are as often adverbs or particles ("up", "off", "over") or
/// subordinating conjunctions ("before", "until") are left out: in
/// "Setting Up the Network" the particle is capitalized, and a slug can't
/// say which reading it means.
const MINOR_WORDS: &[&str] = &[
    // articles
    "a", "an", "the",
    // coordinating conjunctions
    "and", "but", "or", "nor", "for", "so", "yet",
    // "as" in any role
    "as",
    // prepositions
    "about", "across", "against", "along", "amid", "among", "amongst", "at", "behind",
    "below", "beneath", "beside", "besides", "between", "beyond", "by", "despite", "during",
    "except", "from", "in", "inside", "into", "of", "on", "onto", "outside", "per", "than",
    "through", "throughout", "to", "toward", "towards", "under", "underneath", "unlike",
    "upon", "versus", "via", "vs", "with", "within", "without",
];

/// Whether a title leaves `word` lowercase between its first and last words.
pub fn is_minor(word: &str) -> bool {
    MINOR_WORDS.contains(&word.to_ascii_lowercase().as_str())
}

/// A slug as a title: "developing-for-peios" → "Developing for Peios".
pub fn title_from_slug(slug: &str) -> String {
    let words: Vec<&str> = slug.split('-').filter(|word| !word.is_empty()).collect();
    let last = words.len().saturating_sub(1);
    words
        .iter()
        .enumerate()
        .map(|(i, word)| {
            if i != 0 && i != last && is_minor(word) {
                word.to_string()
            } else {
                capitalized(word)
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn capitalized(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_become_chicago_titles() {
        assert_eq!(title_from_slug("developing-for-peios"), "Developing for Peios");
        assert_eq!(title_from_slug("what-an-event-is"), "What an Event Is");
        assert_eq!(title_from_slug("the-two-gates"), "The Two Gates");
        assert_eq!(title_from_slug("writing-policy-for-the-network-firewall"), "Writing Policy for the Network Firewall");
        // First and last words are capitalized whatever they are.
        assert_eq!(title_from_slug("about-tokens"), "About Tokens");
        assert_eq!(title_from_slug("what-it-runs-on"), "What It Runs On");
        // Particles are not prepositions to a slug.
        assert_eq!(title_from_slug("setting-up-the-network"), "Setting Up the Network");
        assert_eq!(title_from_slug("identity"), "Identity");
    }
}
