//! The macros of `crabgram`, which re-exports them; they follow a renamed dependency.

mod choice;
mod controls;
mod info;
mod problem;

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{ToTokens, quote_spanned};
use syn::{
    Attribute, DeriveInput, Error, Ident, Result, Token, meta::ParseNestedMeta, parse_macro_input,
    punctuated::Punctuated,
};

use crate::problem::Problem;

/// Derives `crabgram::Controls`; documented there.
#[proc_macro_derive(Controls, attributes(control))]
pub fn derive_controls(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    controls::derive(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

/// Derives `crabgram::Choice` and `crabgram::Value`; documented on `Choice`.
#[proc_macro_derive(Choice, attributes(choice))]
pub fn derive_choice(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    choice::derive(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

/// An `Info` from the fields `Info` shows and, from `Cargo.toml`, the package name, the first author
/// (`"Name <email>"`), `repository`, `license` and `version`.
#[proc_macro]
pub fn info(input: TokenStream) -> TokenStream {
    let entries =
        parse_macro_input!(input with Punctuated::<info::Entry, Token![,]>::parse_terminated);

    info::expand(entries, &crabgram(Span::call_site()))
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

/// The name the package using a macro gives the `crabgram` crate, read from its manifest.
pub(crate) fn crabgram_name() -> String {
    match crate_name("crabgram") {
        Ok(FoundCrate::Name(name)) => name,
        Ok(FoundCrate::Itself) | Err(_) => "crabgram".to_owned(),
    }
}

/// The path of the `crabgram` crate by its `name`, spanned at `span`.
pub(crate) fn crabgram_at(name: &str, span: Span) -> TokenStream2 {
    let name = Ident::new(name, span);

    quote_spanned!(span=> ::#name)
}

/// The path of the `crabgram` crate as the package using a macro names it, spanned at `span`.
pub(crate) fn crabgram(span: Span) -> TokenStream2 {
    crabgram_at(&crabgram_name(), span)
}

fn reject_generics(input: &DeriveInput, derive: &'static str) -> Result<()> {
    if input.generics.params.is_empty() {
        Ok(())
    } else {
        Err(Problem::Generics { derive }.at(&input.generics))
    }
}

pub(crate) fn reject_attribute(
    attributes: &[Attribute],
    attribute: &'static str,
    place: &'static str,
) -> Result<()> {
    attributes
        .iter()
        .find(|it| it.path().is_ident(attribute))
        .map_or(Ok(()), |it| {
            Err(Problem::MisplacedAttribute { attribute, place }.at(it))
        })
}

pub(crate) fn set_once<T>(
    slot: &mut Option<T>,
    meta: &ParseNestedMeta<'_>,
    value: T,
) -> Result<()> {
    slot.replace(value).map_or(Ok(()), |_| {
        Err(meta.error(Problem::SetTwice {
            option: meta.path.to_token_stream().to_string(),
        }))
    })
}
