use core::ffi::CStr;

use crate::text::{char_count, fits_iri, has_scheme, is_blank, is_iri};

/// Why [`info!`](macro@crate::info) fails to compile, with its [`message`](Self::message).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum InfoProblem {
    /// The URI is not UTF-8.
    UriNotUtf8,
    /// The URI has no scheme, as `urn:` or `https:`.
    UriWithoutScheme,
    /// The URI holds a character Turtle forbids in an IRI.
    UriNotIri,
    /// The name is empty or only whitespace.
    BlankName,
    /// `channels` is 0.
    NoChannels,
    /// The abbreviation is empty or longer than three characters.
    AbbreviationLength,
    /// The abbreviation is only whitespace.
    BlankAbbreviation,
    /// `license_document` is no absolute IRI.
    LicenseDocumentNotIri,
    /// The license names one outside the SPDX list, but `license_document` is `None`.
    UnlistedLicenseWithoutDocument,
    /// The first Cargo author has no `<email>`.
    MaintainerWithoutEmail,
    /// The e-mail of the first Cargo author holds a character Turtle forbids in an IRI.
    MaintainerEmailNotIri,
    /// The Cargo `repository` is missing or no absolute IRI.
    RepositoryNotIri,
    /// The Cargo `license` is missing.
    NoLicense,
    /// The Cargo minor version reaches 1000.
    MinorVersionPastLimit,
}

impl InfoProblem {
    /// The sentence that describes the problem.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::UriNotUtf8 => "the URI must be UTF-8",
            Self::UriWithoutScheme => "the URI needs a scheme, as `urn:` or `https:`",
            Self::UriNotIri => "the URI cannot hold whitespace, control characters or <>\"{}|^`\\",
            Self::BlankName => "the name cannot be blank",
            Self::NoChannels => "a plugin needs at least one channel",
            Self::AbbreviationLength => "the abbreviation must be one to three characters",
            Self::BlankAbbreviation => "the abbreviation cannot be blank",
            Self::LicenseDocumentNotIri => {
                "`license_document` is an IRI with a scheme, as `https:`, and without whitespace"
            }
            Self::UnlistedLicenseWithoutDocument => {
                "a license outside the SPDX list needs `license_document`, the IRI of its text"
            }
            Self::MaintainerWithoutEmail => {
                "Cargo.toml needs `authors = [\"Name <email>\"]`: the first author is the maintainer"
            }
            Self::MaintainerEmailNotIri => {
                "the e-mail of the first author cannot hold whitespace, control characters or <>\"{}|^`\\"
            }
            Self::RepositoryNotIri => {
                "Cargo.toml needs `repository`, an IRI with a scheme, as `https:`"
            }
            Self::NoLicense => "Cargo.toml needs `license`",
            Self::MinorVersionPastLimit => {
                "the minor version must stay below 1000: the LV2 version is 1000 × major + minor"
            }
        }
    }

    /// The first of `problems` there is.
    pub(super) const fn first<const N: usize>(problems: [Option<Self>; N]) -> Option<Self> {
        let mut index = 0;

        while index < N {
            if problems[index].is_some() {
                return problems[index];
            }

            index += 1;
        }

        None
    }

    /// Why `info!` refuses `uri`, if it does.
    pub(super) const fn of_uri(uri: &CStr) -> Option<Self> {
        if uri.to_str().is_err() {
            Some(Self::UriNotUtf8)
        } else if !has_scheme(uri.to_bytes()) {
            Some(Self::UriWithoutScheme)
        } else if !fits_iri(uri.to_bytes()) {
            Some(Self::UriNotIri)
        } else {
            None
        }
    }

    /// Why `info!` refuses `abbreviation`, if it does.
    pub(super) const fn of_abbreviation(abbreviation: &str) -> Option<Self> {
        if !matches!(char_count(abbreviation), 1..=3) {
            Some(Self::AbbreviationLength)
        } else if is_blank(abbreviation) {
            Some(Self::BlankAbbreviation)
        } else {
            None
        }
    }

    /// Why `info!` refuses `channels`, if it does.
    pub(super) const fn of_channels(channels: u32) -> Option<Self> {
        if channels == 0 {
            Some(Self::NoChannels)
        } else {
            None
        }
    }

    /// Why `info!` refuses `license_document`, if it does.
    pub(super) const fn of_license_document(document: &str) -> Option<Self> {
        if is_iri(document.as_bytes()) {
            None
        } else {
            Some(Self::LicenseDocumentNotIri)
        }
    }

    pub(super) const fn fail(self) -> ! {
        panic!("{}", self.message());
    }
}
