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

/// A word of a title or heading: as written, and without the punctuation
/// around it ("(the", "Is:" → "the", "Is").
#[derive(Debug)]
pub struct Word {
    pub raw: String,
    pub bare: String,
}

pub fn words(text: &str) -> Vec<Word> {
    text.split_whitespace()
        .map(|raw| Word {
            raw: raw.to_string(),
            bare: raw.trim_matches(|c: char| !c.is_alphanumeric()).to_string(),
        })
        .collect()
}

/// The positions of words that belong to a proper noun, whose casing is
/// fixed whatever the rules say: "Registry Editor", "netd".
pub fn fixed_words(words: &[Word], proper_nouns: &[String]) -> std::collections::BTreeSet<usize> {
    let mut fixed = std::collections::BTreeSet::new();
    for noun in proper_nouns {
        let parts: Vec<&str> = noun.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        for start in 0..words.len() {
            let matches = parts.len() <= words.len() - start
                && parts.iter().enumerate().all(|(k, part)| words[start + k].bare == *part);
            if matches {
                fixed.extend(start..start + parts.len());
            }
        }
    }
    fixed
}

/// Whether a word's casing is its own: an acronym, an identifier, or a
/// name with an inner capital or digit ("GXWI", "SIDs", "IPv6", "v2").
pub fn is_identifier(word: &str) -> bool {
    word.chars().skip(1).any(|c| c.is_uppercase() || c.is_ascii_digit())
        || word.contains(['_', '.', '/'])
        || !word.starts_with(char::is_alphabetic)
}

/// Whether `title` is in title case, and if not, what it should be.
/// Proper nouns and identifiers keep their casing; a word after a colon
/// starts a subtitle, and so is capitalized like a first word.
pub fn check(title: &str, proper_nouns: &[String]) -> Option<String> {
    let words = words(title);
    let fixed = fixed_words(&words, proper_nouns);
    let last = words.len().saturating_sub(1);
    let mut expected = Vec::with_capacity(words.len());
    for (i, word) in words.iter().enumerate() {
        if fixed.contains(&i) || word.bare.is_empty() || is_identifier(&word.bare) {
            expected.push(word.raw.clone());
            continue;
        }
        let starts = i == 0 || words[i - 1].raw.ends_with(':');
        let ends = i == last || word.raw.ends_with(':');
        let bare = if starts || ends || !is_minor(&word.bare) {
            capitalized(&word.bare)
        } else {
            lowercased(&word.bare)
        };
        expected.push(word.raw.replacen(&word.bare, &bare, 1));
    }
    let expected = expected.join(" ");
    (expected != words.iter().map(|w| w.raw.as_str()).collect::<Vec<_>>().join(" "))
        .then(|| format!("titles use title case: \"{expected}\""))
}

fn lowercased(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
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

    #[test]
    fn titles_are_checked_against_title_case() {
        let nouns = vec!["netd".to_string(), "regman".to_string(), "evman".to_string()];
        for good in [
            "Developing for Peios",
            "What an Event Is",
            "regman and evman",
            "Writing Policy for the Network Firewall",
            "Man Pages: An Overview",
            "GXWI Themes",
            "The netd Book",
        ] {
            assert_eq!(check(good, &nouns), None, "{good}");
        }
        assert_eq!(
            check("Developing For peios", &nouns),
            Some("titles use title case: \"Developing for Peios\"".to_string())
        );
        assert_eq!(
            check("what the Kernel does", &nouns),
            Some("titles use title case: \"What the Kernel Does\"".to_string())
        );
    }
}
