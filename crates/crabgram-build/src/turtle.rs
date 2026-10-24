//! Turtle files of a device bundle: `manifest.ttl`, the plugin description and its styling.

use core::{ffi::CStr, iter};
use std::borrow::Cow;

use crabgram::{
    Control, Controls, Kind, Plugin,
    anagram::{MAX_BPM, MIN_BPM},
    lv2::{CHANGE_REQUEST_URI, STATE_UPDATE_URI},
    port::{self, Port, Role},
};
use crabgram_styling::{
    Styling,
    turtle::{self, Prefix, Term, entry, nonempty, prefixes, statement},
};

/// The file [`files`] writes the manifest to.
pub const MANIFEST: &str = "manifest.ttl";

/// The Turtle files of a device bundle by name: [`MANIFEST`], listing the others, the description
/// [`plugin`] writes to [`Info::turtle`](crabgram::Info::turtle), and [`turtle::FILE`] when
/// [`Styling::turtle`] has something to write.
#[must_use]
pub fn files<P: Plugin>(styling: &Styling) -> Vec<(String, String)> {
    let described: Vec<_> = iter::once((P::INFO.turtle(), plugin::<P>(styling)))
        .chain(
            styling
                .turtle(P::INFO.uri())
                .map(|text| (turtle::FILE.to_owned(), text)),
        )
        .collect();
    let manifest = manifest::<P>(described.iter().map(|(name, _)| name.clone()));

    iter::once((MANIFEST.to_owned(), manifest))
        .chain(described)
        .collect()
}

/// `manifest.ttl`, listing the Turtle files `see_also`.
fn manifest<P: Plugin>(see_also: impl Iterator<Item = String>) -> String {
    let info = &P::INFO;

    format!(
        "{}\n{}",
        prefixes(&[Prefix::LV2, RDFS]),
        statement(
            info.uri(),
            [
                ("a", Term::Name("lv2:Plugin")),
                ("lv2:binary", Term::Iri(info.binary())),
                (
                    "rdfs:seeAlso",
                    Term::List(see_also.map(Term::Iri).collect())
                ),
            ],
        )
    )
}

const fn integer(role: &Role) -> bool {
    match role {
        Role::AudioIn(_) | Role::AudioOut(_) | Role::Tempo => false,
        Role::Enabled | Role::Reset => true,
        Role::Control(control) => match control.kind() {
            Kind::Float { .. } => false,
            Kind::Integer { .. } | Kind::Toggle { .. } | Kind::Choice { .. } => true,
        },
    }
}

fn port_properties(role: &Role) -> Vec<&'static str> {
    let rest = match role {
        Role::AudioIn(_) | Role::AudioOut(_) | Role::Tempo => Vec::new(),
        Role::Enabled => vec!["lv2:toggled"],
        Role::Reset => vec!["lv2:toggled", "pprops:trigger"],
        Role::Control(control) => {
            let kind = match control.kind() {
                Kind::Float { logarithmic, .. } => logarithmic.then_some("pprops:logarithmic"),
                Kind::Integer { .. } => None,
                Kind::Toggle { .. } => Some("lv2:toggled"),
                Kind::Choice { .. } => Some("lv2:enumeration"),
            };

            kind.into_iter()
                .chain(control.is_expensive().then_some("pprops:expensive"))
                .chain(control.is_hidden().then_some("pprops:notOnGUI"))
                .chain(control.is_saved().then_some("dg:savedToPreset"))
                .chain(control.is_blockable().then_some("dg:mayUpdateBlockedState"))
                .collect()
        }
    };

    integer(role)
        .then_some("lv2:integer")
        .into_iter()
        .chain(rest)
        .collect()
}

fn designation(role: &Role, quick_pot: Option<&str>) -> Option<&'static str> {
    match role {
        Role::Enabled => Some("lv2:enabled"),
        Role::Reset => Some("kx:Reset"),
        Role::Tempo => Some("time:beatsPerMinute"),
        Role::Control(control) if quick_pot == Some(control.symbol()) => Some("dg:quickPot"),
        Role::Control(_) | Role::AudioIn(_) | Role::AudioOut(_) => None,
    }
}

/// Default, min and max.
fn range(role: &Role) -> Option<[Term; 3]> {
    let values = match role {
        Role::AudioIn(_) | Role::AudioOut(_) => None,
        Role::Tempo => Some([MIN_BPM, MIN_BPM, MAX_BPM]),
        Role::Enabled => Some([1.0, 0.0, 1.0]),
        Role::Reset => Some([0.0, 0.0, 1.0]),
        Role::Control(control) => Some([control.default_value(), control.min(), control.max()]),
    }?;

    Some(if integer(role) {
        values.map(|value| Term::Integer(value as i64))
    } else {
        values.map(Term::Decimal)
    })
}

fn scale_points(labels: &[&str]) -> Vec<Term> {
    labels
        .iter()
        .enumerate()
        .map(|(value, label)| {
            Term::Node(vec![
                ("rdfs:label", Term::Literal((*label).to_owned())),
                ("rdf:value", Term::Integer(value as i64)),
            ])
        })
        .collect()
}

fn port(port: &Port, quick_pot: Option<&str>) -> Term {
    let role = &port.role();
    let classes = match role {
        Role::AudioIn(_) => ["lv2:InputPort", "lv2:AudioPort"],
        Role::AudioOut(_) => ["lv2:OutputPort", "lv2:AudioPort"],
        Role::Control(control) if control.is_output() => ["lv2:OutputPort", "lv2:ControlPort"],
        Role::Enabled | Role::Reset | Role::Tempo | Role::Control(_) => {
            ["lv2:InputPort", "lv2:ControlPort"]
        }
    };
    let properties = port_properties(role);
    let range = range(role).into_iter().flat_map(|range| {
        ["lv2:default", "lv2:minimum", "lv2:maximum"]
            .into_iter()
            .zip(range)
    });
    let properties = entry("lv2:portProperty", nonempty(properties), |it| {
        Term::names(&it)
    });
    let control = match role {
        Role::Control(control) => Some(control),
        _ => None,
    };
    let points = entry(
        "lv2:scalePoint",
        control
            .map(|it| scale_points(it.labels()))
            .and_then(nonempty),
        Term::List,
    );
    let unit = entry("unit:unit", control.and_then(Control::unit), |it| {
        Term::Name(it.curie())
    });
    let designation = entry("lv2:designation", designation(role, quick_pot), Term::Name);

    Term::node(
        &classes,
        [
            ("lv2:index", Term::Integer(port.index().into())),
            (
                "lv2:symbol",
                Term::Literal(port.role().symbol().into_owned()),
            ),
            ("lv2:name", Term::Literal(port.role().name().into_owned())),
        ]
        .into_iter()
        .chain(range)
        .chain(properties)
        .chain(points)
        .chain(unit)
        .chain(designation),
    )
}

fn license_extensions(uri: &str) -> [(&'static str, Term); 3] {
    [
        ("lv2:extensionData", Term::Name("licns:interface")),
        ("lv2:requiredFeature", Term::Name("licns:feature")),
        ("licns:uri", Term::Iri(uri.to_owned())),
    ]
}

fn mailbox(email: &str) -> String {
    format!("mailto:{email}")
}

fn doap_licenses<P: Plugin>() -> impl Iterator<Item = String> {
    P::INFO
        .license_iris()
        .chain(P::INFO.license_document().map(str::to_owned))
}

/// State updates and change requests, both of an input.
fn optional_features<P: Plugin>() -> Term {
    let input = P::Controls::CONTROLS
        .iter()
        .any(|control| !control.is_output());

    let features = [STATE_UPDATE_URI, CHANGE_REQUEST_URI]
        .into_iter()
        .filter(|_| input)
        .map(CStr::to_string_lossy)
        .map(Cow::into_owned)
        .map(Term::Iri);

    Term::List(
        iter::once(Term::Name("lv2:hardRTCapable"))
            .chain(features)
            .collect(),
    )
}

const LICNS: Prefix = Prefix::new("licns", "http://www.darkglass.com/lv2/ns/lv2ext/license#");
const RDFS: Prefix = Prefix::new("rdfs", "http://www.w3.org/2000/01/rdf-schema#");

const PREFIXES: [Prefix; 11] = [
    Prefix::new("dg", "http://www.darkglass.com/lv2/ns#"),
    Prefix::new("doap", "http://usefulinc.com/ns/doap#"),
    Prefix::new("foaf", "http://xmlns.com/foaf/0.1/"),
    Prefix::new("kx", "http://kxstudio.sf.net/ns/lv2ext/props#"),
    LICNS,
    Prefix::LV2,
    Prefix::new("pprops", "http://lv2plug.in/ns/ext/port-props#"),
    Prefix::new("rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#"),
    RDFS,
    Prefix::new("time", "http://lv2plug.in/ns/ext/time#"),
    Prefix::new("unit", "http://lv2plug.in/ns/extensions/units#"),
];

/// The description `<package>.ttl` of a device bundle, with the license extension when
/// [`Info::licensing`](crabgram::Info::licensing) is on.
#[must_use]
pub fn plugin<P: Plugin>(styling: &Styling) -> String {
    let info = &P::INFO;
    let uri = info.uri();
    let version = info.lv2_version();
    let quick_pot = P::Controls::QUICK_POT.map(Control::symbol);
    let ports = port::ports::<P>()
        .iter()
        .map(|it| port(it, quick_pot))
        .collect();
    let icons = styling.icons().into_iter().flat_map(|icons| {
        [
            ("dg:blockImageOff", Term::Iri(icons.off().to_owned())),
            ("dg:blockImageOn", Term::Iri(icons.on().to_owned())),
        ]
    });
    let maintainer = Term::Node(vec![
        (
            "foaf:name",
            Term::Literal(info.maintainer().name().to_owned()),
        ),
        ("foaf:homepage", Term::Iri(info.repository().to_owned())),
        ("foaf:mbox", Term::Iri(mailbox(info.maintainer().email()))),
    ]);
    let description = || Term::Literal(info.description().to_owned());
    let licensing = info.licensing().checks().then(|| license_extensions(uri));

    let properties = [
        (
            "a",
            Term::names(&[info.category().curie(), "lv2:Plugin", "doap:Project"]),
        ),
        ("lv2:optionalFeature", optional_features::<P>()),
    ]
    .into_iter()
    .chain(licensing.into_iter().flatten())
    .chain([
        ("lv2:port", Term::List(ports)),
        ("rdfs:comment", description()),
        (
            "dg:abbreviation",
            Term::Literal(info.abbreviation().to_owned()),
        ),
    ])
    .chain(icons)
    .chain([
        ("doap:name", Term::Literal(info.name().to_owned())),
        ("doap:shortdesc", description()),
    ])
    .chain(entry(
        "doap:license",
        nonempty(doap_licenses::<P>().map(Term::Iri).collect()),
        Term::List,
    ))
    .chain([
        ("doap:maintainer", maintainer),
        ("lv2:project", Term::Iri(uri.to_owned())),
        ("lv2:microVersion", Term::Integer(version.micro().into())),
        ("lv2:minorVersion", Term::Integer(version.minor().into())),
    ]);

    format!("{}\n{}", prefixes(&PREFIXES), statement(uri, properties))
}

#[cfg(test)]
mod tests {
    use crabgram::{
        Controls,
        lv2::{CHANGE_REQUEST_URI, STATE_UPDATE_URI},
        port::Role,
    };
    use crabgram_styling::{BlockSettings, Styling, turtle::prefixes};

    use super::{LICNS, files, plugin, port_properties};
    use crate::fixture::{Blocking, Documented, Gain, Licensed};

    #[test]
    fn a_licensed_plugin_names_its_own_license() {
        let turtle = plugin::<Licensed>(&Styling::default());

        assert!(turtle.contains("licns:uri <urn:example:gain> ;"));
        assert!(turtle.contains("lv2:requiredFeature licns:feature"));
    }

    #[test]
    fn a_plugin_without_licensing_names_no_license() {
        let turtle = plugin::<Gain>(&Styling::default());

        assert!(!turtle.contains("licns:interface"));
        assert!(!turtle.contains("licns:uri"));
    }

    #[test]
    fn a_plugin_with_an_input_may_update_its_states_and_request_changes() {
        let [state_update, change_request] = [STATE_UPDATE_URI, CHANGE_REQUEST_URI]
            .map(|uri| format!("<{}>", uri.to_str().expect("an ASCII URI")));
        let expected =
            format!("lv2:optionalFeature lv2:hardRTCapable , {state_update} , {change_request} ;");

        assert!(plugin::<Gain>(&Styling::default()).contains(&expected));
        assert!(plugin::<Blocking>(&Styling::default()).contains(&expected));
    }

    #[test]
    fn every_plugin_lists_a_tempo_port_designated_as_the_beats_per_minute() {
        let turtle = plugin::<Gain>(&Styling::default());

        assert!(turtle.contains("lv2:designation time:beatsPerMinute"));
        assert!(turtle.contains("lv2:symbol \"lv2_bpm\""));
        assert!(turtle.contains("lv2:index 5"));
    }

    #[test]
    fn an_expensive_control_carries_the_port_property() {
        #[derive(Controls)]
        struct Mixed {
            plain: f32,
            #[control(expensive)]
            costly: f32,
        }

        assert_eq!(
            port_properties(&Role::Control(*Mixed::PLAIN.as_ref())),
            Vec::<&str>::new()
        );
        assert_eq!(
            port_properties(&Role::Control(*Mixed::COSTLY.as_ref())),
            ["pprops:expensive"]
        );
    }

    #[test]
    fn integer_ports_write_integers_and_the_rest_decimals() {
        let turtle = plugin::<Gain>(&Styling::default());

        assert!(turtle.contains("lv2:minimum 40.0 ;"));
        assert!(turtle.contains("lv2:maximum 300.0 ;"));
        assert!(turtle.contains("lv2:maximum 1 ;"));
        assert!(!turtle.contains("lv2:minimum 40 ;"));
    }

    #[test]
    fn a_licensed_plugin_declares_the_interface_the_binary_answers() {
        let turtle = plugin::<Licensed>(&Styling::default());
        let interface = crabgram::lv2::LICENSE_INTERFACE_URI
            .to_str()
            .expect("an ASCII URI");
        let namespace = format!(
            "<{}>",
            interface.strip_suffix("interface").expect("a licns: term")
        );

        assert!(prefixes(&[LICNS]).contains(&namespace));
        assert!(turtle.contains(&namespace));
        assert!(turtle.contains("lv2:extensionData licns:interface"));
    }

    #[test]
    fn the_manifest_comes_first_and_the_styling_last() {
        let names = |styling: &Styling| -> Vec<String> {
            files::<Gain>(styling)
                .into_iter()
                .map(|(name, _)| name)
                .collect()
        };

        let description = <Gain as crabgram::Plugin>::INFO.turtle();

        assert_eq!(names(&Styling::default()), ["manifest.ttl", &description]);
        assert_eq!(
            names(&Styling::default().with_block_settings(BlockSettings::default())),
            ["manifest.ttl", &description, "styling.ttl"]
        );
    }

    #[test]
    fn a_quick_pot_is_designated_as_such() {
        #[derive(Controls)]
        struct Potted {
            #[control(quick_pot)]
            gain: f32,
        }

        let description = plugin::<Gain<Potted>>(&Styling::default());

        assert!(description.contains("lv2:designation dg:quickPot"));
    }

    #[test]
    fn the_manifest_lists_the_styling_file_only_when_there_is_styling_to_write() {
        let styled = Styling::default().with_block_settings(BlockSettings::default());

        let manifest = |styling| files::<Gain>(&styling).swap_remove(0).1;

        assert!(manifest(styled).contains("<styling.ttl>"));
        assert!(!manifest(Styling::default()).contains("<styling.ttl>"));
    }

    #[test]
    fn the_license_document_follows_the_spdx_licenses() {
        let turtle = plugin::<Documented>(&Styling::default());

        assert!(turtle.contains(
            "doap:license <http://spdx.org/licenses/MIT.html> , <https://example.com/gain/license> ;"
        ));
    }
}
