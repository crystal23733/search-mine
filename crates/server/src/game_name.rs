//! Shared validation for game nicknames and room titles.
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};
use unicode_segmentation::UnicodeSegmentation;

pub(crate) fn normalize(value: &str, max_graphemes: usize, max_bytes: usize) -> Option<String> {
    if value.len() > 256 {
        return None;
    }
    let value: String = value.nfc().collect();
    if value.len() > max_bytes || value.starts_with(' ') || value.ends_with(' ') {
        return None;
    }
    let mut count = 0;
    for grapheme in value.graphemes(true) {
        count += 1;
        if count > max_graphemes {
            return None;
        }
        if grapheme == " " {
            continue;
        }
        if !grapheme.chars().next().is_some_and(char::is_alphanumeric)
            || !grapheme.chars().all(|character| {
                (character.is_alphanumeric() || is_combining_mark(character))
                    && !matches!(
                        character,
                        '\u{034f}' | '\u{180b}'..='\u{180f}' | '\u{fe00}'..='\u{fe0f}'
                            | '\u{e0100}'..='\u{e01ef}'
                    )
            })
        {
            return None;
        }
    }
    (count >= 2).then_some(value)
}
