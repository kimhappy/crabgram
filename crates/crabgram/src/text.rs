//! Const checks of text, shared by the controls and the description.

pub const fn starts_with(bytes: &[u8], prefix: &[u8]) -> bool {
    if bytes.len() < prefix.len() {
        return false;
    }

    let mut index = 0;

    while index < prefix.len() && bytes[index] == prefix[index] {
        index += 1;
    }

    index == prefix.len()
}

pub const fn same(left: &str, right: &str) -> bool {
    left.len() == right.len() && starts_with(left.as_bytes(), right.as_bytes())
}

/// The first character of `text` and the text after it; none for empty text.
const fn split_first_char(text: &str) -> Option<(char, &str)> {
    let bytes = text.as_bytes();
    let (width, lead) = match bytes.first() {
        None => return None,
        Some(&lead @ 0x00..=0x7F) => (1, lead),
        Some(&lead @ 0xC0..=0xDF) => (2, lead & 0x1F),
        Some(&lead @ 0xE0..=0xEF) => (3, lead & 0x0F),
        Some(&lead) => (4, lead & 0x07),
    };
    let mut code = lead as u32;
    let mut index = 1;

    while index < width {
        code = code << 6 | (bytes[index] & 0x3F) as u32;
        index += 1;
    }

    match char::from_u32(code) {
        Some(first) => Some((first, text.split_at(width).1)),
        None => None,
    }
}

/// How many characters `text` has: its bytes that start one.
pub const fn char_count(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut count = 0;

    while index < bytes.len() {
        count += (bytes[index] & 0xC0 != 0x80) as usize;
        index += 1;
    }

    count
}

/// Whether `iri` opens with a scheme: an ASCII letter, then letters, digits, `+`, `-` or `.`, then
/// `:`.
pub const fn has_scheme(iri: &[u8]) -> bool {
    let mut index = 0;

    while index < iri.len()
        && (iri[index].is_ascii_alphanumeric() || matches!(iri[index], b'+' | b'-' | b'.'))
    {
        index += 1;
    }

    index > 0 && iri[0].is_ascii_alphabetic() && index < iri.len() && iri[index] == b':'
}

/// Whether `text` holds none of the characters Turtle forbids in an IRI: whitespace, controls and
/// ``<>"{}|^`\``.
#[must_use]
pub const fn fits_iri(text: &[u8]) -> bool {
    let mut index = 0;

    while index < text.len() {
        if matches!(
            text[index],
            0..=b' ' | 0x7F | b'<' | b'>' | b'"' | b'{' | b'}' | b'|' | b'^' | b'`' | b'\\'
        ) {
            return false;
        }

        index += 1;
    }

    true
}

/// Whether `text` is an absolute IRI Turtle can write.
pub const fn is_iri(text: &[u8]) -> bool {
    has_scheme(text) && fits_iri(text)
}

/// Whether `text` is empty or only whitespace.
pub const fn is_blank(text: &str) -> bool {
    let mut rest = text;

    while let Some((first, after)) = split_first_char(rest) {
        if !first.is_whitespace() {
            return false;
        }

        rest = after;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::{char_count, fits_iri, has_scheme, is_blank};

    #[test]
    fn characters_count_code_points_not_bytes() {
        assert_eq!(char_count("TRM"), 3);
        assert_eq!(char_count("ÄÖÜ"), 3);
        assert_eq!(char_count(""), 0);
    }

    #[test]
    fn a_scheme_is_a_letter_then_scheme_characters_then_a_colon() {
        assert!(
            ["urn:example:gain", "https://example.com", "a+b-c.d:x"]
                .into_iter()
                .all(|it| has_scheme(it.as_bytes()))
        );
        assert!(
            [
                "",
                "gain",
                ":gain",
                "1urn:gain",
                "path/to:gain",
                "ur n:gain"
            ]
            .into_iter()
            .all(|it| !has_scheme(it.as_bytes()))
        );
    }

    #[test]
    fn spaces_controls_and_turtle_delimiters_do_not_fit_an_iri() {
        assert!(
            [
                "a b", "a\tb", "ab\n", "a\u{7f}", "a<b", "a\"b", "a{b", "a|b", "a^b", "a`b", "a\\b"
            ]
            .into_iter()
            .all(|it| !fits_iri(it.as_bytes()))
        );
        assert!(fits_iri("urn:example:gäin".as_bytes()));
    }

    #[test]
    fn only_whitespace_is_blank() {
        assert!(
            ["", " ", "\t\n", "\u{3000}", " \u{a0} "]
                .into_iter()
                .all(is_blank)
        );
        assert!(!["a", " a ", "\u{3000}ä", "가"].into_iter().any(is_blank));
    }
}
