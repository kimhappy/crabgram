use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Data, DeriveInput, Fields, LitStr, Result, ext::IdentExt};

use crate::problem::Problem;

/// The label of `variant`; the rules of a label are checked where the labels are built.
fn label(variant: &syn::Variant) -> Result<String> {
    let mut label: Option<LitStr> = None;

    variant
        .attrs
        .iter()
        .filter(|it| it.path().is_ident("choice"))
        .try_for_each(|attribute| {
            attribute.parse_nested_meta(|meta| {
                if meta.path.is_ident("label") {
                    crate::set_once(&mut label, &meta, meta.value()?.parse()?)
                } else {
                    Err(meta.error(Problem::UnknownChoiceOption))
                }
            })
        })?;

    Ok(label.map_or_else(|| variant.ident.unraw().to_string(), |it| it.value()))
}

fn check_unit(variant: &syn::Variant) -> Result<()> {
    if !matches!(variant.fields, Fields::Unit) {
        return Err(Problem::ChoiceVariantWithFields.at(variant));
    }

    variant
        .discriminant
        .as_ref()
        .map_or(Ok(()), |(_, discriminant)| {
            Err(Problem::ChoiceDiscriminant.at(discriminant))
        })
}

pub fn derive(input: &DeriveInput) -> Result<TokenStream> {
    crate::reject_generics(input, "Choice")?;
    crate::reject_attribute(&input.attrs, "choice", "a variant")?;

    let Data::Enum(data) = &input.data else {
        return Err(Problem::ChoiceOfNoEnum.at(&input.ident));
    };

    data.variants.iter().try_for_each(check_unit)?;

    let krate = crate::crabgram(Span::call_site());
    let name = &input.ident;
    let variants = data.variants.iter().map(|it| &it.ident);
    let labels = data
        .variants
        .iter()
        .map(label)
        .collect::<Result<Vec<_>>>()?;

    Ok(quote! {
        #[automatically_derived]
        impl #krate::__private::SealedValue for #name {}

        #[automatically_derived]
        impl ::core::clone::Clone for #name {
            fn clone(&self) -> Self {
                *self
            }
        }

        #[automatically_derived]
        impl ::core::marker::Copy for #name {}

        #[automatically_derived]
        impl #name {
            /// The position of this variant in `Choice::ALL`, its port value, in `const` too.
            #[must_use]
            pub const fn index(self) -> ::core::primitive::usize {
                self as ::core::primitive::usize
            }

            /// The label of this variant, in `const` too.
            #[must_use]
            pub const fn label(self) -> &'static ::core::primitive::str {
                <Self as #krate::Choice>::LABELS[self.index()]
            }
        }

        #[automatically_derived]
        impl #krate::Choice for #name {
            const ALL: &'static [Self] = &[#(Self::#variants),*];
            const LABELS: &'static [&'static ::core::primitive::str] =
                #krate::__private::control::labels(&[#(#labels),*]);
        }

        const _: &[&::core::primitive::str] = <#name as #krate::Choice>::LABELS;

        #[automatically_derived]
        impl #krate::Value for #name {
            fn from_port(raw: ::core::primitive::f32, control: &#krate::Control) -> Self {
                let all = <Self as #krate::Choice>::ALL;

                all[control.label_index(raw).min(all.len() - 1)]
            }

            fn to_port(self) -> ::core::primitive::f32 {
                self.index() as ::core::primitive::f32
            }
        }
    })
}
