use core::{ffi::CStr, iter};

use super::{
    Category, InfoProblem, Licensing, Version,
    make::Fields,
    text::{is_unlisted, maintainer_of, minor, names_unlisted_license, next_license},
};
use crate::text::{fits_iri, is_blank, is_iri};

/// Everything about a plugin but its controls, written with [`info!`](macro@crate::info).
///
/// ```
/// use crabgram::Info;
///
/// const INFO: Info = crabgram::info! {
///     uri: c"https://example.com/plugins/tremolo",
///     name: "Example Tremolo",
///     description: "A tremolo.",
///     abbreviation: "TRM",
///     category: Modulator,
///     channels: 1,
/// };
///
/// assert_eq!(INFO.uri(), "https://example.com/plugins/tremolo");
/// assert_eq!(INFO.bundle(), "crabgram.lv2");
/// assert_eq!(INFO.maintainer().name(), "kimhappy");
/// assert_eq!(INFO.maintainer().email(), "babtul21@gmail.com");
/// assert_eq!(
///     INFO.license_iris().collect::<Vec<_>>(),
///     ["http://spdx.org/licenses/GPL-3.0-only.html"]
/// );
/// ```
///
/// `licensing` and `license_document` are optional:
///
/// ```
/// use crabgram::{Info, Licensing};
///
/// const INFO: Info = crabgram::info! {
///     uri: c"urn:example:tremolo",
///     name: "Tremolo",
///     description: "A tremolo.",
///     abbreviation: "TRM",
///     category: Modulator,
///     channels: 1,
///     licensing: Commercial,
///     license_document: "https://example.com/license",
/// };
///
/// assert_eq!(INFO.uri(), "urn:example:tremolo");
/// assert_eq!(INFO.licensing(), Licensing::Commercial);
/// assert_eq!(INFO.license_document(), Some("https://example.com/license"));
/// ```
///
/// Any [`InfoProblem`], such as an abbreviation over three characters, fails to compile:
///
/// ```compile_fail,E0080
/// use crabgram::Info;
///
/// const INFO: Info = crabgram::info! {
///     uri: c"urn:example:tremolo",
///     name: "Tremolo",
///     description: "",
///     abbreviation: "TREM",
///     category: Modulator,
///     channels: 1,
/// };
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Info(pub(super) Fields);

/// The first author of a package.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Maintainer {
    name: &'static str,
    email: &'static str,
}

impl Maintainer {
    /// The name before the `<`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// The e-mail between `<` and `>`.
    #[must_use]
    pub const fn email(self) -> &'static str {
        self.email
    }
}

impl Info {
    /// The plugin URI; never change it after release.
    #[must_use]
    pub const fn uri(&self) -> &'static str {
        match self.0.uri.to_str() {
            Ok(uri) => uri,
            Err(_) => unreachable!(),
        }
    }

    /// [`uri`](Self::uri) as a C string.
    pub(crate) const fn c_uri(&self) -> &'static CStr {
        self.0.uri
    }

    /// `doap:name`.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.0.name
    }

    /// One sentence: `rdfs:comment` and `doap:shortdesc`.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        self.0.description
    }

    /// `dg:abbreviation`: one to three characters.
    #[must_use]
    pub const fn abbreviation(&self) -> &'static str {
        self.0.abbreviation
    }

    /// The plugin class.
    #[must_use]
    pub const fn category(&self) -> Category {
        self.0.category
    }

    /// Audio channels, each processed in place: 1 for mono, 2 for stereo.
    #[must_use]
    pub const fn channels(&self) -> u32 {
        self.0.channels
    }

    /// Which license the plugin asks for.
    #[must_use]
    pub const fn licensing(&self) -> Licensing {
        self.0.licensing
    }

    /// The IRI of the license text; required for a license outside the SPDX list (`LicenseRef-` or
    /// `DocumentRef-`).
    #[must_use]
    pub const fn license_document(&self) -> Option<&'static str> {
        self.0.license_document
    }

    /// Cargo `name`, which names `<package>.lv2`, `.so` and `.ttl`.
    #[must_use]
    pub const fn package(&self) -> &'static str {
        self.0.package
    }

    /// Cargo `repository`, the maintainer's home page.
    #[must_use]
    pub const fn repository(&self) -> &'static str {
        self.0.repository
    }

    /// Why [`info!`](macro@crate::info) refuses this description, if it does.
    pub(super) const fn problem(&self) -> Option<InfoProblem> {
        let Fields {
            uri,
            name,
            channels,
            abbreviation,
            license_document,
            license,
            authors,
            repository,
            version,
            ..
        } = self.0;

        let document = match license_document {
            Some(document) => InfoProblem::of_license_document(document),
            None => None,
        };
        let field = InfoProblem::first([
            InfoProblem::of_uri(uri),
            if is_blank(name) {
                Some(InfoProblem::BlankName)
            } else {
                None
            },
            InfoProblem::of_channels(channels),
            InfoProblem::of_abbreviation(abbreviation),
            document,
        ]);

        let email = match maintainer_of(authors) {
            Some((_, email)) => email,
            None => "",
        };

        if field.is_some() {
            field
        } else if license_document.is_none() && names_unlisted_license(license) {
            Some(InfoProblem::UnlistedLicenseWithoutDocument)
        } else if email.is_empty() {
            Some(InfoProblem::MaintainerWithoutEmail)
        } else if !fits_iri(email.as_bytes()) {
            Some(InfoProblem::MaintainerEmailNotIri)
        } else if !is_iri(repository.as_bytes()) {
            Some(InfoProblem::RepositoryNotIri)
        } else if license.is_empty() {
            Some(InfoProblem::NoLicense)
        } else if minor(version).len() > 3 {
            Some(InfoProblem::MinorVersionPastLimit)
        } else {
            None
        }
    }

    /// `<package>.lv2`.
    #[must_use]
    pub fn bundle(&self) -> String {
        format!("{}.lv2", self.0.package)
    }

    /// `<package>.so`.
    #[must_use]
    pub fn binary(&self) -> String {
        format!("{}.so", self.0.package)
    }

    /// `<package>.ttl`.
    #[must_use]
    pub fn turtle(&self) -> String {
        format!("{}.ttl", self.0.package)
    }

    /// Name and e-mail of the first author.
    #[must_use]
    pub const fn maintainer(&self) -> Maintainer {
        let Some((name, email)) = maintainer_of(self.0.authors) else {
            unreachable!()
        };

        Maintainer { name, email }
    }

    /// Cargo `version`.
    #[must_use]
    pub const fn version(&self) -> &'static str {
        self.0.version
    }

    /// The package version as the LV2 version `(1000 × major + minor).patch`: `1.2.0` is `1002.0`.
    #[must_use]
    pub fn lv2_version(&self) -> Version {
        let mut parts = self
            .0
            .version
            .split(['.', '-', '+'])
            .map(str::parse::<u32>)
            .map(Result::unwrap_or_default);
        let mut next = || parts.next().unwrap_or_default();
        let (major, minor, patch) = (next(), next(), next());

        Version {
            minor: major.saturating_mul(1000).saturating_add(minor),
            micro: patch,
        }
    }

    fn license_ids(&self) -> impl Iterator<Item = &'static str> {
        iter::successors(next_license(self.0.license), |(_, rest)| next_license(rest))
            .map(|(id, _)| id)
    }

    /// An SPDX IRI per license of the `license` expression on the SPDX list; a `WITH` exception
    /// gets none.
    pub fn license_iris(&self) -> impl Iterator<Item = String> {
        self.license_ids()
            .filter(|it| !is_unlisted(it.as_bytes()))
            .map(|it| format!("http://spdx.org/licenses/{it}.html"))
    }
}
