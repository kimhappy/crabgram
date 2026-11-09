use super::{
    Category, Info, InfoProblem, Licensing,
    make::{Fields, checked},
    text::{first_author, minor, names_unlisted_license},
};

fn fields(license: &'static str, version: &'static str) -> Fields {
    Fields {
        uri: c"urn:crabgram:test",
        name: "Test",
        description: "",
        abbreviation: "TST",
        category: Category::Utility,
        channels: 1,
        licensing: Licensing::Off,
        license_document: None,
        package: "test",
        authors: "Ada <ada@example.com>",
        repository: "https://example.com",
        license,
        version,
    }
}

const fn unchecked(fields: Fields) -> Info {
    Info(fields)
}

fn version_of(info: Info) -> (u32, u32) {
    let version = info.lv2_version();

    (version.minor(), version.micro())
}

fn info(license: &'static str, version: &'static str) -> Info {
    unchecked(fields(license, version))
}

#[test]
fn the_first_author_ends_at_the_first_colon() {
    assert_eq!(first_author(b"Ada <a@b>:Bob:Cy"), b"Ada <a@b>");
    assert_eq!(first_author(b"Ada <a@b>"), b"Ada <a@b>");
    assert_eq!(first_author(b""), b"");
}

#[test]
fn the_minor_part_is_the_second_number() {
    assert_eq!(minor("1.2.3"), b"2");
    assert_eq!(minor("0.1000.0-rc.1"), b"1000");
    assert_eq!(minor("1"), b"");
    assert_eq!(minor("1."), b"");
}

fn problem_of(fields: Fields) -> Option<InfoProblem> {
    unchecked(fields).problem()
}

fn maintainer_problem(authors: &'static str) -> Option<InfoProblem> {
    problem_of(Fields {
        authors,
        ..fields("MIT", "0.1.0")
    })
}

#[test]
fn a_first_author_with_an_email_passes_whatever_follows() {
    assert_eq!(maintainer_problem("Ada <ada@example.com>:Bob"), None);
}

#[test]
fn an_email_on_a_later_author_does_not_count() {
    assert_eq!(
        maintainer_problem("Ada:Bob <bob@example.com>"),
        Some(InfoProblem::MaintainerWithoutEmail)
    );
}

#[test]
fn a_uri_that_is_no_text_is_a_problem() {
    assert_eq!(
        problem_of(Fields {
            uri: c"urn:\xff",
            ..fields("MIT", "0.1.0")
        }),
        Some(InfoProblem::UriNotUtf8)
    );
}

#[test]
fn a_uri_without_a_scheme_or_with_a_forbidden_character_is_a_problem() {
    let problems = [c"", c"gain", c"plugins/gain", c"urn:a gain", c"urn:a<b"].map(|uri| {
        problem_of(Fields {
            uri,
            ..fields("MIT", "0.1.0")
        })
    });

    assert_eq!(
        problems,
        [
            Some(InfoProblem::UriWithoutScheme),
            Some(InfoProblem::UriWithoutScheme),
            Some(InfoProblem::UriWithoutScheme),
            Some(InfoProblem::UriNotIri),
            Some(InfoProblem::UriNotIri),
        ]
    );
}

#[test]
fn a_license_document_is_an_iri_with_a_scheme() {
    let problems = [
        "",
        "license.html",
        "https://example.com/a license",
        "https://example.com/l",
    ]
    .map(|document| {
        problem_of(Fields {
            license_document: Some(document),
            ..fields("MIT", "0.1.0")
        })
    });

    assert_eq!(
        problems,
        [
            Some(InfoProblem::LicenseDocumentNotIri),
            Some(InfoProblem::LicenseDocumentNotIri),
            Some(InfoProblem::LicenseDocumentNotIri),
            None,
        ]
    );
}

#[test]
fn a_maintainer_email_with_a_forbidden_character_is_a_problem() {
    assert_eq!(
        problem_of(Fields {
            authors: "Someone <some one@example.com>",
            ..fields("MIT", "0.1.0")
        }),
        Some(InfoProblem::MaintainerEmailNotIri)
    );
}

#[test]
fn a_blank_name_is_a_problem() {
    assert_eq!(
        problem_of(Fields {
            name: " \t",
            ..fields("MIT", "0.1.0")
        }),
        Some(InfoProblem::BlankName)
    );
}

#[test]
fn a_package_without_an_absolute_repository_or_a_license_is_a_problem() {
    let problems = ["", "github.com/x", "https://example.com/a b"].map(|repository| {
        problem_of(Fields {
            repository,
            ..fields("MIT", "0.1.0")
        })
    });

    assert_eq!(problems, [Some(InfoProblem::RepositoryNotIri); 3]);
    assert_eq!(
        problem_of(fields("", "0.1.0")),
        Some(InfoProblem::NoLicense)
    );
}

#[test]
fn a_long_first_author_passes_in_a_const() {
    const AUTHORS: &str = "Ada Augusta King, Countess of Lovelace, Augusta King, Countess of Lovelace, Augusta King, Countess of Lovelace, Augusta King, Countess of Lovelace, Augusta King, Countess of Lovelace, <ada@example.com>:Bob";
    const MAINTAINER: &[u8] = first_author(AUTHORS.as_bytes());
    const INFO: Info = checked(Fields {
        uri: c"urn:crabgram:test",
        name: "Test",
        description: "",
        abbreviation: "TST",
        category: Category::Utility,
        channels: 1,
        licensing: Licensing::Off,
        license_document: None,
        package: "test",
        authors: AUTHORS,
        repository: "https://example.com",
        license: "MIT",
        version: "0.1.0",
    });

    assert_eq!(MAINTAINER.len(), AUTHORS.len() - ":Bob".len());
    assert_eq!(INFO.maintainer().email(), "ada@example.com");
}

#[test]
fn an_empty_blank_or_reversed_email_does_not_count() {
    for authors in ["Ada <>", "Ada < >", "Ada >a<", "Ada <a"] {
        assert_eq!(
            maintainer_problem(authors),
            Some(InfoProblem::MaintainerWithoutEmail),
            "{authors}"
        );
    }
}

#[test]
fn a_license_exception_names_no_license() {
    assert_eq!(
        info("(Apache-2.0 WITH LLVM-exception) OR MIT", "0.1.0")
            .license_iris()
            .collect::<Vec<_>>(),
        [
            "http://spdx.org/licenses/Apache-2.0.html",
            "http://spdx.org/licenses/MIT.html"
        ]
    );
}

#[test]
fn a_license_outside_the_spdx_list_has_no_spdx_iri() {
    let proprietary = info("LicenseRef-Proprietary", "0.1.0");
    let either = info("MIT OR DocumentRef-terms:LicenseRef-Custom", "0.1.0");

    assert_eq!(proprietary.license_iris().count(), 0);
    assert!(
        either
            .license_iris()
            .eq(["http://spdx.org/licenses/MIT.html".to_owned()])
    );
}

#[test]
fn an_unlisted_license_is_found_anywhere_but_in_an_exception() {
    let expressions = [
        ("MIT", false),
        ("LicenseRef-Proprietary", true),
        ("(MIT OR LicenseRef-Own)", true),
        ("Apache-2.0/DocumentRef-terms:LicenseRef-Custom", true),
        ("GPL-2.0-only WITH LicenseRef-Exception", false),
        (
            "MIT AND (GPL-2.0-only WITH Classpath-exception-2.0) OR LicenseRef-X",
            true,
        ),
        ("LicenseRefX", false),
        ("", false),
    ];

    for (license, unlisted) in expressions {
        assert_eq!(names_unlisted_license(license), unlisted, "{license}");
    }
}

#[test]
fn a_license_outside_the_spdx_list_takes_a_document() {
    let document = Some("https://example.com/license");
    let documented = Fields {
        license_document: document,
        ..fields("MIT OR LicenseRef-Proprietary", "0.1.0")
    };

    assert_eq!(checked(documented).license_document(), document);
}

#[test]
fn a_license_outside_the_spdx_list_needs_a_document() {
    assert_eq!(
        problem_of(fields("MIT OR LicenseRef-Proprietary", "0.1.0")),
        Some(InfoProblem::UnlistedLicenseWithoutDocument)
    );
}

#[test]
fn a_zero_major_version_keeps_its_minor_and_patch_parts() {
    assert_eq!(version_of(info("MIT", "0.2.3-rc.1")), (2, 3));
    assert_eq!(version_of(info("MIT", "0.2")), (2, 0));
}

#[test]
fn a_major_version_counts_a_thousand_minor_versions() {
    let version = |cargo| version_of(info("MIT", cargo));

    assert_eq!(version("1.0.0"), (1000, 0));
    assert_eq!(version("1.2.3+build.7"), (1002, 3));
    assert!(
        version("2.0.0").0 > version("1.999.999").0,
        "a new major version compares as newer"
    );
}

#[test]
fn a_minor_version_of_a_thousand_does_not_fit() {
    assert_eq!(
        problem_of(fields("MIT", "1.1000.0")),
        Some(InfoProblem::MinorVersionPastLimit)
    );
}

#[test]
fn the_maintainer_is_the_trimmed_name_and_email_of_the_first_author() {
    let info = unchecked(Fields {
        authors: " Ada Lovelace < ada@example.com >:Bob <bob@example.com>",
        ..fields("MIT", "0.1.0")
    });

    assert_eq!(
        (info.maintainer().name(), info.maintainer().email()),
        ("Ada Lovelace", "ada@example.com")
    );
}

#[test]
fn a_blank_abbreviation_is_a_problem() {
    assert_eq!(
        problem_of(Fields {
            abbreviation: " \t",
            ..fields("MIT", "0.1.0")
        }),
        Some(InfoProblem::BlankAbbreviation)
    );
}
