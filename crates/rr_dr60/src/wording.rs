//! The words the engine's own strings do not use (spec 026 D3).
//!
//! The engine describes signal processing only. These lists keep its manifests, its guide and
//! its other strings to that: [`ENGINE_STRINGS`] is the stage and control manifests' list, and
//! [`CLAIMS`] the words its guide sections avoid. This is **not a store or copy check**: the
//! claims list is a subset of what a host's own copy rules cover, and a host layers its own.
//!
//! The matching rule:
//! - whole words only, where a word is a maximal run of [`char::is_alphanumeric`] characters or
//!   `_`;
//! - letters compared case-insensitively in ASCII (the lists are ASCII);
//! - a stem matches a word equal to the stem, or the stem plus one of the [`Lists::suffixes`];
//! - a phrase matches a window on word boundaries;
//! - the leftmost hit in the text wins, stems before phrases at one position.
//!
//! The rule differs from a Unicode regex `\w` in a few character classes, which the engine's
//! strings do not use:
//! - combining marks and connector punctuation other than `_` are not word characters here;
//! - superscript and fraction digits (`²`, `½`) are;
//! - non-ASCII case folding (the Kelvin sign, long ſ) is not applied.

/// A tier's lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lists {
    /// Word stems, matched with an optional suffix.
    pub stems: &'static [&'static str],
    /// Exact words and phrases, matched on word boundaries.
    pub phrases: &'static [&'static str],
    /// The suffixes a stem may take.
    pub suffixes: &'static [&'static str],
    /// Phrases that do not count when the text right before them is this (`("measured", "not ")`).
    pub exceptions: &'static [(&'static str, &'static str)],
}

/// The stage and control manifests' list (spec 007 FR-011): reword the string, never this list.
pub const ENGINE_STRINGS: Lists = Lists {
    stems: &[
        "verif",
        "detect",
        "sens",
        "anomal",
        "spik",
        "activit",
        "event",
        "paranormal",
        "evp",
        "ghost",
        "spirit",
        "entit",
    ],
    phrases: &[
        "listening for",
        "signal present",
        "voice from",
        "communication",
    ],
    suffixes: &[
        "e", "es", "s", "ed", "ied", "ies", "y", "ying", "ing", "ion", "ions", "ication",
        "ications", "or", "ors", "ous", "ive",
    ],
    exceptions: &[],
};

/// The words the guide's sections avoid, each with its forms (spec 026 D3).
pub const CLAIMS: Lists = Lists {
    stems: &[],
    phrases: &[
        "accurate",
        "accuracy",
        "authentic",
        "authenticity",
        "reveal",
        "reveals",
        "revealed",
        "uncover",
        "uncovers",
        "hidden voice",
        "hidden voices",
        "measured",
    ],
    suffixes: &[],
    exceptions: &[("measured", "not ")],
};

/// A hit: the list entry and the matched byte range of the text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    /// The stem or phrase that matched.
    pub entry: &'static str,
    /// The match's first byte.
    pub start: usize,
    /// One past its last byte.
    pub end: usize,
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The ASCII-case-insensitive equality of `text[at..]`'s prefix with `lit`, and its length.
fn starts_with_ci(text: &str, at: usize, lit: &str) -> bool {
    let t = text.as_bytes();
    let l = lit.as_bytes();
    t.len() >= at + l.len()
        && t[at..at + l.len()]
            .iter()
            .zip(l)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

/// Whether a word boundary lies at byte `at` of `text`.
fn boundary(text: &str, at: usize) -> bool {
    let before = text[..at].chars().next_back().is_some_and(is_word);
    let after = text[at..].chars().next().is_some_and(is_word);
    before != after
}

/// The first hit of `lists` in `text` (the matching rule above). Allocation-free; linear in the
/// text's length times the lists' size.
pub fn first_hit_in(lists: &Lists, text: &str) -> Option<Hit> {
    for (at, _) in text.char_indices() {
        if !boundary(text, at) || !text[at..].chars().next().is_some_and(is_word) {
            continue;
        }
        // The word starting here.
        let end_word = text[at..]
            .char_indices()
            .find(|(_, c)| !is_word(*c))
            .map_or(text.len(), |(i, _)| at + i);
        for &stem in lists.stems {
            if starts_with_ci(text, at, stem) {
                let rest = at + stem.len();
                if rest == end_word
                    || lists.suffixes.iter().any(|sfx| {
                        rest + sfx.len() == end_word && starts_with_ci(text, rest, sfx)
                    })
                {
                    return Some(Hit {
                        entry: stem,
                        start: at,
                        end: end_word,
                    });
                }
            }
        }
        for &phrase in lists.phrases {
            if starts_with_ci(text, at, phrase) && boundary(text, at + phrase.len()) {
                let excepted = lists.exceptions.iter().any(|(p, before)| {
                    *p == phrase && at >= before.len() && starts_with_ci(text, at - before.len(), before)
                });
                if !excepted {
                    return Some(Hit {
                        entry: phrase,
                        start: at,
                        end: at + phrase.len(),
                    });
                }
            }
        }
    }
    None
}

/// The first hit of [`ENGINE_STRINGS`] in `text`.
pub fn first_hit(text: &str) -> Option<Hit> {
    first_hit_in(&ENGINE_STRINGS, text)
}

/// The first hit of [`CLAIMS`] in `text` ("not measured" excepted).
pub fn first_claim_hit(text: &str) -> Option<Hit> {
    first_hit_in(&CLAIMS, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_words_and_suffixes() {
        assert_eq!(first_hit("a gate event").map(|h| h.entry), Some("event"));
        assert_eq!(first_hit("Events here").map(|h| (h.start, h.end)), Some((0, 6)));
        assert!(first_hit("The sensitivity setting").is_none(), "sens + itivity is no suffix");
        assert!(first_hit("prevented").is_none(), "inside a word");
        assert_eq!(first_hit("we are listening for it").map(|h| h.entry), Some("listening for"));
        assert_eq!(first_claim_hit("an accurate copy").map(|h| h.entry), Some("accurate"));
        assert!(first_claim_hit("not measured").is_none());
        assert!(first_claim_hit("measured on a unit").is_some());
        assert!(first_claim_hit("revealing").is_none(), "only the listed forms");
    }
}
