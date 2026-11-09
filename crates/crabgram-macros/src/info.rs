use std::collections::HashMap;

use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{
    Expr, ExprLit, Ident, Lit, Result, Token,
    parse::{Parse, ParseStream},
};

use crate::problem::Problem;

const REQUIRED: [&str; 6] = [
    "uri",
    "name",
    "description",
    "abbreviation",
    "category",
    "channels",
];
const OPTIONAL: [&str; 2] = ["licensing", "license_document"];

pub struct Entry {
    key: Ident,
    value: Expr,
}

impl Parse for Entry {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let key = input.parse()?;

        input.parse::<Token![:]>()?;

        Ok(Self {
            key,
            value: input.parse()?,
        })
    }
}

fn allowed() -> String {
    format!(
        "{} (required) and {} (optional)",
        REQUIRED.join(", "),
        OPTIONAL.join(", ")
    )
}

fn values(entries: impl IntoIterator<Item = Entry>) -> Result<HashMap<String, Expr>> {
    entries
        .into_iter()
        .try_fold(HashMap::new(), |mut values, entry| {
            let key = entry.key.to_string();

            if !REQUIRED.contains(&key.as_str()) && !OPTIONAL.contains(&key.as_str()) {
                Err(Problem::UnknownInfoKey {
                    key,
                    allowed: allowed(),
                }
                .at(&entry.key))
            } else if values.insert(key.clone(), entry.value).is_some() {
                Err(Problem::SetTwice { option: key }.at(&entry.key))
            } else {
                Ok(values)
            }
        })
}

fn check_required(values: &HashMap<String, Expr>) -> Result<()> {
    let missing: Vec<_> = REQUIRED
        .iter()
        .filter(|key| !values.contains_key(**key))
        .map(|key| format!("`{key}`"))
        .collect();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(Problem::MissingInfoKeys {
            missing: missing.join(", "),
            allowed: allowed(),
        }
        .at_span(Span::call_site()))
    }
}

/// The C string literal of `uri`, the one way to write it.
fn uri(value: &Expr) -> Result<&Expr> {
    match value {
        Expr::Lit(ExprLit {
            lit: Lit::CStr(_), ..
        }) => Ok(value),
        _ => Err(Problem::UriNotCString.at(value)),
    }
}

/// The variant of `enumeration` that `value` names alone, the one way to write it.
fn variant(
    value: &Expr,
    key: &'static str,
    enumeration: &'static str,
    example: &'static str,
    krate: &TokenStream,
) -> Result<TokenStream> {
    let problem = || {
        Problem::InfoVariantNotIdent {
            key,
            enumeration,
            example,
        }
        .at(value)
    };
    let Expr::Path(path) = value else {
        return Err(problem());
    };
    let variant = path.path.get_ident().ok_or_else(problem)?;
    let enumeration = Ident::new(enumeration, variant.span());

    Ok(quote_spanned!(variant.span()=> #krate::#enumeration::#variant))
}

pub fn expand(
    entries: impl IntoIterator<Item = Entry>,
    krate: &TokenStream,
) -> Result<TokenStream> {
    let values = values(entries)?;

    check_required(&values)?;

    let uri = uri(&values["uri"])?;
    let [name, description, abbreviation, channels] =
        ["name", "description", "abbreviation", "channels"].map(|key| &values[key]);
    let category = variant(
        &values["category"],
        "category",
        "Category",
        "Utility",
        krate,
    )?;
    let licensing = values.get("licensing").map_or_else(
        || Ok(quote!(#krate::Licensing::Off)),
        |it| variant(it, "licensing", "Licensing", "Commercial", krate),
    )?;
    let license_document = values.get("license_document").map_or_else(
        || quote!(::core::option::Option::None),
        |it| quote!(::core::option::Option::Some(#it)),
    );

    Ok(quote! {
        #krate::__private::info::checked(#krate::__private::info::Fields {
            uri: #uri,
            name: #name,
            description: #description,
            abbreviation: #abbreviation,
            category: #category,
            channels: #channels,
            licensing: #licensing,
            license_document: #license_document,
            package: ::core::env!("CARGO_PKG_NAME"),
            authors: ::core::env!("CARGO_PKG_AUTHORS"),
            repository: ::core::env!("CARGO_PKG_REPOSITORY"),
            license: ::core::env!("CARGO_PKG_LICENSE"),
            version: ::core::env!("CARGO_PKG_VERSION"),
        })
    })
}
