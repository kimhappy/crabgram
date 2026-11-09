use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, format_ident, quote, quote_spanned};
use syn::{
    Attribute, Data, DeriveInput, Expr, ExprLit, ExprUnary, Field, Fields, Ident, Lit, LitFloat,
    LitStr, Path, Result, Token, Type, UnOp, Visibility, ext::IdentExt, meta::ParseNestedMeta,
    spanned::Spanned,
};

use crate::{problem::Problem, set_once};

enum Kind {
    Float,
    Integer,
    Toggle,
    Choice,
}

fn kind(ty: &Type) -> Result<Kind> {
    let path = match ty {
        Type::Path(path) => path,
        Type::Group(group) => return kind(&group.elem),
        Type::Paren(paren) => return kind(&paren.elem),
        _ => {
            return Err(Problem::UnsupportedType.at(ty));
        }
    };
    let last = path
        .path
        .segments
        .last()
        .map_or_default(|it| it.ident.to_string());

    Ok(match last.as_str() {
        "f32" => Kind::Float,
        "bool" => Kind::Toggle,
        "i8" | "i16" | "i32" | "u8" | "u16" | "u32" => Kind::Integer,
        "f64" | "i64" | "u64" | "i128" | "u128" | "isize" | "usize" => {
            return Err(Problem::WideNumber.at(ty));
        }
        _ => Kind::Choice,
    })
}

#[derive(Default)]
struct Options {
    name: Option<LitStr>,
    output: Option<Path>,
    logarithmic: Option<Path>,
    expensive: Option<Path>,
    hidden: Option<Path>,
    saved: Option<Path>,
    blockable: Option<Path>,
    quick_pot: Option<Path>,
    default: Option<Expr>,
    min: Option<Expr>,
    max: Option<Expr>,
    unit: Option<Ident>,
}

impl Options {
    fn parse(attributes: &[Attribute]) -> Result<Self> {
        let mut options = Self::default();

        attributes
            .iter()
            .filter(|it| it.path().is_ident("control"))
            .try_for_each(|attribute| attribute.parse_nested_meta(|meta| options.set(&meta)))?;

        Ok(options)
    }

    fn set(&mut self, meta: &ParseNestedMeta<'_>) -> Result<()> {
        let flag = |slot: &mut Option<Path>| {
            if meta.input.peek(Token![=]) {
                return Err(meta.error(Problem::FlagWithValue {
                    flag: meta.path.to_token_stream().to_string(),
                }));
            }

            set_once(slot, meta, meta.path.clone())
        };

        match meta.path.get_ident().map(ToString::to_string).as_deref() {
            Some("output") => flag(&mut self.output),
            Some("logarithmic") => flag(&mut self.logarithmic),
            Some("expensive") => flag(&mut self.expensive),
            Some("hidden") => flag(&mut self.hidden),
            Some("saved") => flag(&mut self.saved),
            Some("blockable") => flag(&mut self.blockable),
            Some("quick_pot") => flag(&mut self.quick_pot),
            Some("name") => set_once(&mut self.name, meta, meta.value()?.parse()?),
            Some("default") => set_once(&mut self.default, meta, meta.value()?.parse()?),
            Some("min") => set_once(&mut self.min, meta, meta.value()?.parse()?),
            Some("max") => set_once(&mut self.max, meta, meta.value()?.parse()?),
            Some("unit") => {
                let path: syn::Path = meta.value()?.parse()?;
                let unit = path
                    .get_ident()
                    .cloned()
                    .ok_or_else(|| Problem::UnitNotVariant.at(&path))?;

                set_once(&mut self.unit, meta, unit)
            }
            _ => Err(meta.error(Problem::UnknownControlOption)),
        }
    }

    /// Refuses the quick pot next to `flag`; the quick pot is no flag of a `Control`, so only the
    /// derive sees it.
    fn reject_quick_pot_with(&self, flag: Option<&Path>, problem: Problem) -> Result<()> {
        self.quick_pot
            .as_ref()
            .zip(flag)
            .map_or(Ok(()), |(it, _)| Err(problem.at(it)))
    }

    /// Refuses the flags only an input takes on an output, and `saved` without `hidden`.
    fn reject_misplaced_flags(&self) -> Result<()> {
        if self.output.is_some() {
            [
                (&self.expensive, "expensive"),
                (&self.saved, "saved to presets"),
                (&self.blockable, "blocked"),
            ]
            .into_iter()
            .try_for_each(|(flag, what)| {
                reject(flag.as_ref(), what, &|what| Problem::FlagOnOutput { what })
            })
        } else if self.hidden.is_none() {
            reject(self.saved.as_ref(), "saved", &|_| {
                Problem::SavedWithoutHidden
            })
        } else {
            Ok(())
        }
    }

    /// Refuses the options only a number takes.
    fn reject_numeric(&self, refusal: impl Fn(&'static str) -> Problem) -> Result<()> {
        reject(self.min.as_ref(), "min", &refusal)?;
        reject(self.max.as_ref(), "max", &refusal)?;
        reject(self.unit.as_ref(), "unit", &refusal)
    }
}

fn reject(
    option: Option<&impl ToTokens>,
    what: &'static str,
    refusal: &impl Fn(&'static str) -> Problem,
) -> Result<()> {
    option.map_or(Ok(()), |it| Err(refusal(what).at(it)))
}

fn title(symbol: &str) -> String {
    symbol
        .split('_')
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut letters = word.chars();
            letters
                .next()
                .map_or_default(|first| first.to_uppercase().chain(letters).collect::<String>())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The doc comments of a field, or a line naming its control when it has none.
fn docs(attributes: &[Attribute], symbol: &str) -> TokenStream {
    let written: Vec<&Attribute> = attributes
        .iter()
        .filter(|it| it.path().is_ident("doc"))
        .collect();

    if written.is_empty() {
        let fallback = format!("The `{symbol}` control.");

        quote!(#[doc = #fallback])
    } else {
        quote!(#(#written)*)
    }
}

/// The port symbol of a field: its name without `r#`. The rules of a symbol are checked where
/// the control is built.
fn symbol(ident: &Ident) -> String {
    ident.unraw().to_string()
}

/// `value` with an unsuffixed integer literal, negated or not, written as a float literal.
fn floated(value: Expr) -> Expr {
    match value {
        Expr::Lit(ExprLit {
            attrs,
            lit: Lit::Int(int),
        }) if int.suffix().is_empty() => Expr::Lit(ExprLit {
            attrs,
            lit: Lit::Float(LitFloat::new(
                &format!("{}.0", int.base10_digits()),
                int.span(),
            )),
        }),
        Expr::Unary(ExprUnary {
            attrs,
            op: UnOp::Neg(neg),
            expr,
        }) => Expr::Unary(ExprUnary {
            attrs,
            op: UnOp::Neg(neg),
            expr: Box::new(floated(*expr)),
        }),
        other => other,
    }
}

/// Every call below spans at the field, so rustc reports a refused control there.
fn float_kind(span: Span, krate: &TokenStream, options: &Options) -> TokenStream {
    let min = options
        .min
        .clone()
        .map_or_else(|| syn::parse_quote!(0.0), floated);
    let max = options
        .max
        .clone()
        .map_or_else(|| syn::parse_quote!(1.0), floated);
    let default = options.default.clone().map_or_else(|| min.clone(), floated);
    let logarithmic = options.logarithmic.is_some();

    quote_spanned!(span=> #krate::__private::control::float(#default, #min, #max, #logarithmic))
}

fn integer_kind(
    span: Span,
    krate: &TokenStream,
    field: &Field,
    options: &Options,
) -> Result<TokenStream> {
    let missing = || Problem::IntegerWithoutRange.at(field);
    let min = options.min.clone().ok_or_else(missing)?;
    let max = options.max.clone().ok_or_else(missing)?;
    let default = options.default.clone().unwrap_or_else(|| min.clone());
    let ty = &field.ty;
    let widened = |value: &Expr| {
        quote!({
            #[deny(overflowing_literals)]
            let __crabgram_value: #ty = #value;
            __crabgram_value as ::core::primitive::i64
        })
    };
    let (default, min, max) = (widened(&default), widened(&min), widened(&max));

    Ok(quote_spanned!(span=> #krate::__private::control::integer(#default, #min, #max)))
}

fn toggle_kind(span: Span, krate: &TokenStream, options: &Options) -> Result<TokenStream> {
    options.reject_numeric(|option| Problem::NumericOnToggle { option })?;

    let default = options
        .default
        .clone()
        .unwrap_or_else(|| syn::parse_quote!(false));

    Ok(quote_spanned!(span=> #krate::__private::control::toggle(#default)))
}

fn choice_kind(
    span: Span,
    krate: &TokenStream,
    ty: &Type,
    options: &Options,
) -> Result<TokenStream> {
    let name = ty.to_token_stream().to_string().replace(' ', "");

    options.reject_numeric(|option| Problem::NumericOnChoice {
        option,
        ty: name.clone(),
    })?;

    let default = options.default.as_ref().map_or_else(
        || quote!(0),
        |it| {
            quote!({
                let __crabgram_value: #ty = #it;
                __crabgram_value as ::core::primitive::usize
            })
        },
    );

    Ok(quote_spanned! {span=>
        #krate::__private::control::choice(#default, <#ty as #krate::Choice>::LABELS)
    })
}

/// The expression of the `Control`: one call that checks every rule at once.
fn control(field: &Field, symbol: &str, options: &Options, crabgram: &str) -> Result<TokenStream> {
    let span = field.ident.span();
    let krate = crate::crabgram_at(crabgram, span);
    let name = options.name.as_ref().map_or_else(
        || {
            let title = title(symbol);

            quote!(#title)
        },
        |it| {
            let krate = crate::crabgram_at(crabgram, it.span());

            quote_spanned!(it.span()=> #krate::__private::control::name(#it))
        },
    );
    let direction = if options.output.is_some() {
        quote!(#krate::__private::Direction::Output)
    } else {
        quote!(#krate::__private::Direction::Input)
    };
    let kind = kind(&field.ty)?;

    if !matches!(kind, Kind::Float) {
        reject(options.logarithmic.as_ref(), "logarithmic", &|_| {
            Problem::LogarithmicNotFloat
        })?;
    }

    let kind = match kind {
        Kind::Float => float_kind(span, &krate, options),
        Kind::Integer => integer_kind(span, &krate, field, options)?,
        Kind::Toggle => toggle_kind(span, &krate, options)?,
        Kind::Choice => choice_kind(span, &krate, &field.ty, options)?,
    };
    let unit = options.unit.as_ref().map_or_else(
        || quote!(::core::option::Option::None),
        |it| quote!(::core::option::Option::Some(#krate::Unit::#it)),
    );
    let [expensive, hidden, saved, blockable] = [
        &options.expensive,
        &options.hidden,
        &options.saved,
        &options.blockable,
    ]
    .map(Option::is_some);

    Ok(quote_spanned! {span=>
        #krate::__private::control::control(
            #symbol,
            #name,
            #direction,
            #kind,
            #unit,
            #krate::__private::Flags {
                expensive: #expensive,
                hidden: #hidden,
                saved: #saved,
                blockable: #blockable,
            },
        )
    })
}

fn access(options: &Options) -> Ident {
    let marker = if options.output.is_some() {
        "Output"
    } else if options.blockable.is_some() {
        "Blockable"
    } else {
        "Input"
    };

    Ident::new(marker, Span::call_site())
}

struct Port<'a> {
    field: &'a Ident,
    vis: &'a Visibility,
    ty: &'a Type,
    constant: Ident,
    control: TokenStream,
    access: Ident,
    docs: TokenStream,
    quick_pot: Option<Span>,
}

impl<'a> Port<'a> {
    /// The port of `field`, naming the `crabgram` crate `crabgram`.
    fn new(field: &'a Field, crabgram: &str) -> Result<Self> {
        let ident = field.ident.as_ref().expect("named fields have names");
        let symbol = symbol(ident);

        let options = Options::parse(&field.attrs)?;

        options.reject_quick_pot_with(options.output.as_ref(), Problem::QuickPotOnOutput)?;
        options.reject_quick_pot_with(options.hidden.as_ref(), Problem::HiddenQuickPot)?;
        options.reject_misplaced_flags()?;

        Ok(Self {
            field: ident,
            vis: &field.vis,
            ty: &field.ty,
            constant: format_ident!("{}", symbol.to_uppercase()),
            control: control(field, &symbol, &options, crabgram)?,
            access: access(&options),
            docs: docs(&field.attrs, &symbol),
            quick_pot: options.quick_pot.as_ref().map(Spanned::span),
        })
    }

    fn constant_item(&self, krate: &TokenStream, position: usize) -> TokenStream {
        let Self {
            vis,
            ty,
            constant,
            access,
            docs,
            ..
        } = self;

        quote! {
            #docs
            #vis const #constant: #krate::Field<Self, #krate::access::#access, #ty> =
                #krate::__private::field(#position);
        }
    }

    fn read_field(&self, krate: &TokenStream, position: usize) -> TokenStream {
        let Self {
            field,
            ty,
            constant,
            ..
        } = self;

        quote!(#field: <#ty as #krate::Value>::from_port(raw[#position], ::core::convert::AsRef::<#krate::Control>::as_ref(&Self::#constant)))
    }

    fn write_field(&self, krate: &TokenStream) -> TokenStream {
        let field = self.field;

        quote!(#krate::Value::to_port(self.#field))
    }
}

/// Fails at a constant that hides a `Controls` constant or repeats an earlier one.
fn check_constants(ports: &[Port<'_>]) -> Result<()> {
    if let Some(port) = ports
        .iter()
        .find(|port| port.constant == "CONTROLS" || port.constant == "QUICK_POT")
    {
        return Err(Problem::ConstantHidesTrait {
            constant: port.constant.to_string(),
        }
        .at(port.field));
    }

    ports
        .iter()
        .enumerate()
        .find(|(position, port)| {
            ports[..*position]
                .iter()
                .any(|earlier| earlier.constant == port.constant)
        })
        .map_or(Ok(()), |(_, port)| {
            Err(Problem::RepeatedConstant {
                constant: port.constant.to_string(),
            }
            .at(port.field))
        })
}

fn quick_pot(krate: &TokenStream, ports: &[Port<'_>]) -> Result<TokenStream> {
    let mut marked = ports
        .iter()
        .enumerate()
        .filter_map(|(position, port)| port.quick_pot.map(|span| (position, span)));

    match (marked.next(), marked.next()) {
        (None, _) => Ok(quote!(::core::option::Option::None)),
        (Some((position, _)), None) => Ok(quote! {
            ::core::option::Option::Some(&<Self as #krate::Controls>::CONTROLS[#position])
        }),
        (Some(_), Some((_, span))) => Err(Problem::SecondQuickPot.at_span(span)),
    }
}

pub fn derive(input: &DeriveInput) -> Result<TokenStream> {
    crate::reject_generics(input, "Controls")?;
    crate::reject_attribute(&input.attrs, "control", "a field")?;

    let Data::Struct(data) = &input.data else {
        return Err(Problem::ControlsOfNoStruct.at(&input.ident));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(Problem::ControlsOfUnnamedFields.at(&input.ident));
    };

    if fields.named.is_empty() {
        return Err(Problem::NoControls.at(&input.ident));
    }

    let crabgram = crate::crabgram_name();
    let ports = fields
        .named
        .iter()
        .map(|field| Port::new(field, &crabgram))
        .collect::<Result<Vec<_>>>()?;

    check_constants(&ports)?;

    let krate = crate::crabgram_at(&crabgram, Span::call_site());
    let name = &input.ident;
    let count = ports.len();
    let quick_pot = quick_pot(&krate, &ports)?;
    let constants = ports
        .iter()
        .enumerate()
        .map(|(position, port)| port.constant_item(&krate, position));
    let controls = ports.iter().map(|port| &port.control);
    let reads = ports
        .iter()
        .enumerate()
        .map(|(position, port)| port.read_field(&krate, position));
    let writes = ports.iter().map(|port| port.write_field(&krate));
    let defaults = ports.iter().map(|port| {
        let constant = &port.constant;

        quote!(::core::convert::AsRef::<#krate::Control>::as_ref(&Self::#constant).default_value())
    });

    Ok(quote! {
        #[automatically_derived]
        impl #name {
            #(#constants)*
        }

        #[automatically_derived]
        impl #krate::__private::SealedControls for #name {}

        #[automatically_derived]
        impl #krate::Controls for #name {
            const CONTROLS: &'static [#krate::Control] = &[#(#controls),*];
            const QUICK_POT: ::core::option::Option<&'static #krate::Control> = #quick_pot;

            type Values = [::core::primitive::f32; #count];

            fn from_ports(raw: &Self::Values) -> Self {
                Self { #(#reads,)* }
            }

            fn to_ports(&self) -> Self::Values {
                [#(#writes),*]
            }
        }

        const _: &[#krate::Control] = <#name as #krate::Controls>::CONTROLS;

        #[automatically_derived]
        impl ::core::default::Default for #name {
            fn default() -> Self {
                <Self as #krate::Controls>::from_ports(&[#(#defaults),*])
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_skips_empty_words() {
        assert_eq!(title("gain"), "Gain");
        assert_eq!(title("_gain"), "Gain");
        assert_eq!(title("gain__db_"), "Gain Db");
    }

    #[test]
    fn a_type_passed_through_a_macro_keeps_its_kind() {
        let grouped: Type = Type::Group(syn::TypeGroup {
            group_token: syn::token::Group::default(),
            elem: Box::new(syn::parse_quote!(u8)),
        });

        assert!(matches!(kind(&grouped), Ok(Kind::Integer)));
    }
}
