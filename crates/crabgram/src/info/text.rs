use crate::text::starts_with;

pub(super) const fn is_unlisted(id: &[u8]) -> bool {
    starts_with(id, b"LicenseRef-") || starts_with(id, b"DocumentRef-")
}

const fn separates(byte: u8) -> bool {
    byte.is_ascii_whitespace() || matches!(byte, b'(' | b')' | b'/')
}

const fn next_token(expression: &str) -> (&str, &str) {
    let bytes = expression.as_bytes();
    let mut start = 0;

    while start < bytes.len() && separates(bytes[start]) {
        start += 1;
    }

    let mut end = start;

    while end < bytes.len() && !separates(bytes[end]) {
        end += 1;
    }

    let (through_token, rest) = expression.split_at(end);

    (through_token.split_at(start).1, rest)
}

/// The first license of `expression`, skipping operators and `WITH` exceptions, and the rest.
pub(super) const fn next_license(expression: &str) -> Option<(&str, &str)> {
    let (token, rest) = next_token(expression);

    match token.as_bytes() {
        [] => None,
        b"OR" | b"AND" => next_license(rest),
        b"WITH" => next_license(next_token(rest).1),
        _ => Some((token, rest)),
    }
}

pub(super) const fn names_unlisted_license(license: &str) -> bool {
    let mut rest = license;

    while let Some((id, after)) = next_license(rest) {
        if is_unlisted(id.as_bytes()) {
            return true;
        }

        rest = after;
    }

    false
}

pub(super) const fn position(bytes: &[u8], byte: u8) -> usize {
    let mut index = 0;

    while index < bytes.len() && bytes[index] != byte {
        index += 1;
    }

    index
}

/// The minor part of `version`, as `CARGO_PKG_VERSION_MINOR` holds it.
pub(super) const fn minor(version: &str) -> &[u8] {
    let bytes = version.as_bytes();
    let start = position(bytes, b'.') + 1;

    if start >= bytes.len() {
        return &[];
    }

    let after_major = bytes.split_at(start).1;

    after_major.split_at(position(after_major, b'.')).0
}

pub(super) const fn first_author(authors: &[u8]) -> &[u8] {
    authors.split_at(position(authors, b':')).0
}

/// The name and e-mail of the first of `authors`, `"Name <email>"`, both trimmed; none without an
/// e-mail. Every cut falls on an ASCII byte, so on a character boundary.
pub(super) const fn maintainer_of(authors: &str) -> Option<(&str, &str)> {
    let first = authors.split_at(first_author(authors.as_bytes()).len()).0;
    let (name, from_open) = first.split_at(position(first.as_bytes(), b'<'));
    let close = position(from_open.as_bytes(), b'>');

    if close >= from_open.len() {
        return None;
    }

    let email = from_open.split_at(close).0.split_at(1).1.trim_ascii();

    if email.is_empty() {
        None
    } else {
        Some((name.trim_ascii(), email))
    }
}
