use proc_macro2::Span;
use quote::ToTokens;
use syn::Error;

/// Why a derive refuses its input. Each becomes a compile error at the tokens to blame.
#[derive(Debug, thiserror::Error)]
pub enum Problem {
    #[error("{derive} derives only for types without generic parameters")]
    Generics { derive: &'static str },
    #[error("`#[{attribute}]` goes on {place}")]
    MisplacedAttribute {
        attribute: &'static str,
        place: &'static str,
    },
    #[error("`{option}` is set twice")]
    SetTwice { option: String },
    #[error("`info!` has no field `{key}`: it takes {allowed}")]
    UnknownInfoKey { key: String, allowed: String },
    #[error("`info!` needs {missing}: it takes {allowed}")]
    MissingInfoKeys { missing: String, allowed: String },
    #[error("`uri` is a C string literal: write `c\"...\"`")]
    UriNotCString,
    #[error("`{key}` names a variant of `{enumeration}` alone, as `{key}: {example}`")]
    InfoVariantNotIdent {
        key: &'static str,
        enumeration: &'static str,
        example: &'static str,
    },
    #[error("expected `label = \"...\"`")]
    UnknownChoiceOption,
    #[error(
        "expected one of `name`, `output`, `default`, `min`, `max`, `unit`, `logarithmic`, \
         `expensive`, `hidden`, `saved`, `blockable`, `quick_pot`"
    )]
    UnknownControlOption,
    #[error("`unit` names a variant of `Unit` alone, as `unit = Hertz`")]
    UnitNotVariant,
    #[error("`{flag}` is a flag and takes no value")]
    FlagWithValue { flag: String },
    #[error("Choice derives only for enums")]
    ChoiceOfNoEnum,
    #[error("a Choice variant holds no fields")]
    ChoiceVariantWithFields,
    #[error("a Choice takes its values from the variant order, not from discriminants")]
    ChoiceDiscriminant,
    #[error("Controls derives only for structs")]
    ControlsOfNoStruct,
    #[error("Controls needs named fields: each names a port")]
    ControlsOfUnnamedFields,
    #[error("a plugin needs at least one control")]
    NoControls,
    #[error(
        "a control field is `f32`, `bool`, an integer up to 32 bits or an enum deriving `Choice`"
    )]
    UnsupportedType,
    #[error("LV2 control ports are 32-bit floats: use f32, or an integer up to 32 bits")]
    WideNumber,
    #[error("the quick pot takes an input, not an output")]
    QuickPotOnOutput,
    #[error("the quick pot cannot be hidden")]
    HiddenQuickPot,
    #[error("only an input can be {what}")]
    FlagOnOutput { what: &'static str },
    #[error("`saved` is for a `hidden` input")]
    SavedWithoutHidden,
    #[error("only one control can be the quick pot")]
    SecondQuickPot,
    #[error("only an f32 control can be logarithmic")]
    LogarithmicNotFloat,
    #[error("an integer control needs `min` and `max`")]
    IntegerWithoutRange,
    #[error("a bool control takes no `{option}`")]
    NumericOnToggle { option: &'static str },
    #[error(
        "a Choice control takes no `{option}`; the derive knows types by name, so if `{ty}` is a \
         type alias, write the type it names"
    )]
    NumericOnChoice { option: &'static str, ty: String },
    #[error(
        "the field makes the constant `{constant}`, which would hide `Controls::{constant}`: \
         rename the field"
    )]
    ConstantHidesTrait { constant: String },
    #[error("another field already makes the constant `{constant}`: rename one")]
    RepeatedConstant { constant: String },
}

impl Problem {
    pub fn at(self, tokens: impl ToTokens) -> Error {
        Error::new_spanned(tokens, self)
    }

    pub fn at_span(self, span: Span) -> Error {
        Error::new(span, self)
    }
}
