// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded procedural macros for `amari-discovery` wire contracts.
//!
//! The derive added by this crate intentionally supports only the Serde DTO
//! shapes used by executable Amari probes. Unsupported shapes fail at compile
//! time so wire-contract authority cannot silently drift into an underspecified
//! schema representation.

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{parenthesized, Data, DeriveInput, Fields, Ident, LitStr, Meta, Token};

struct WireContractArgs {
    id: LitStr,
    role: LitStr,
    compatibility: Option<LitStr>,
    constraints: Vec<ConstraintArg>,
    examples: Vec<ExampleArg>,
}

impl Parse for WireContractArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut id = None;
        let mut role = None;
        let mut compatibility = None;
        let mut constraints = Vec::new();
        let mut examples = Vec::new();

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            match key.to_string().as_str() {
                "id" => {
                    input.parse::<Token![=]>()?;
                    id = Some(input.parse()?);
                }
                "role" => {
                    input.parse::<Token![=]>()?;
                    role = Some(input.parse()?);
                }
                "compatibility" => {
                    input.parse::<Token![=]>()?;
                    compatibility = Some(input.parse()?);
                }
                "constraints" => {
                    let content;
                    parenthesized!(content in input);
                    while !content.is_empty() {
                        constraints.push(content.parse()?);
                        if content.is_empty() {
                            break;
                        }
                        content.parse::<Token![,]>()?;
                    }
                }
                "example" => {
                    let content;
                    parenthesized!(content in input);
                    examples.push(content.parse()?);
                }
                unknown => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unsupported wire_contract argument `{unknown}`"),
                    ));
                }
            }

            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }

        Ok(Self {
            id: id.ok_or_else(|| {
                syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "wire_contract requires `id = \"...\"`",
                )
            })?,
            role: role.ok_or_else(|| {
                syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "wire_contract requires `role = \"input\"` or `role = \"output\"`",
                )
            })?,
            compatibility,
            constraints,
            examples,
        })
    }
}

struct ConstraintArg {
    id: Ident,
    description: LitStr,
}

impl Parse for ConstraintArg {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let id: Ident = input.parse()?;
        input.parse::<Token![=]>()?;
        let description: LitStr = input.parse()?;
        if id.to_string().trim().is_empty() || description.value().trim().is_empty() {
            return Err(syn::Error::new(
                id.span(),
                "wire constraints require a nonempty ID and description",
            ));
        }
        Ok(Self { id, description })
    }
}

struct ExampleArg {
    label: LitStr,
    value: LitStr,
}

impl Parse for ExampleArg {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut label = None;
        let mut value = None;
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            match key.to_string().as_str() {
                "label" => label = Some(input.parse::<LitStr>()?),
                "value" => value = Some(input.parse::<LitStr>()?),
                unknown => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unsupported wire example argument `{unknown}`"),
                    ));
                }
            }
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }

        let label: LitStr = label.ok_or_else(|| {
            syn::Error::new(input.span(), "wire example requires `label = \"...\"`")
        })?;
        let value: LitStr = value.ok_or_else(|| {
            syn::Error::new(input.span(), "wire example requires `value = \"...\"`")
        })?;
        if label.value().trim().is_empty() {
            return Err(syn::Error::new(
                label.span(),
                "wire example label must be nonempty",
            ));
        }
        serde_json::from_str::<serde_json::Value>(&value.value()).map_err(|error| {
            syn::Error::new(
                value.span(),
                format!("wire example value is not JSON: {error}"),
            )
        })?;
        Ok(Self { label, value })
    }
}

/// Derives a bounded wire contract for an executable probe DTO.
///
/// The input must be a non-generic named-field struct or one of the bounded
/// enum representations used by discovery probe DTOs. The type must also
/// implement `schemars::JsonSchema`, normally through `#[derive(JsonSchema)]`.
#[proc_macro_derive(WireContract, attributes(wire_contract))]
pub fn derive_wire_contract(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    match expand_wire_contract(&input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand_wire_contract(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new(
            input.ident.span(),
            "WireContract does not support generic DTOs; derive it on a concrete wire type",
        ));
    }

    validate_data_shape(input)?;

    let args = input
        .attrs
        .iter()
        .find(|attribute| attribute.path().is_ident("wire_contract"))
        .ok_or_else(|| {
            syn::Error::new(
                input.ident.span(),
                "WireContract requires #[wire_contract(id = \"...\", role = \"input|output\")]",
            )
        })?
        .parse_args::<WireContractArgs>()?;

    let role = parse_role(&args.role)?;
    validate_schema_id(&args.id, &args.role)?;
    let compatibility = parse_compatibility(args.compatibility.as_ref())?;

    let name = &input.ident;
    let id = &args.id;
    let constraints = args.constraints.iter().map(|constraint| {
        let constraint_id = &constraint.id;
        let description = &constraint.description;
        quote! {
            crate::wire::WireSemanticConstraint::new(
                stringify!(#constraint_id),
                #description,
            )
        }
    });
    let examples = args.examples.iter().map(|example| {
        let label = &example.label;
        let value = &example.value;
        quote! {
            crate::wire::WireExample::new(
                #label,
                ::serde_json::from_str(#value)
                    .expect("wire contract example JSON was validated at compile time"),
            )
        }
    });

    Ok(quote! {
        impl crate::wire::WireContract for #name {
            const SCHEMA_ID: &'static str = #id;
            const ROLE: crate::wire::WireSchemaRole = #role;
            const COMPATIBILITY: crate::wire::WireCompatibility = #compatibility;

            fn structural_schema() -> ::serde_json::Value {
                ::serde_json::to_value(::schemars::schema_for!(#name))
                    .expect("schemars structural schema must serialize")
            }

            fn semantic_constraints() -> Vec<crate::wire::WireSemanticConstraint> {
                vec![#(#constraints),*]
            }

            fn examples() -> Vec<crate::wire::WireExample> {
                vec![#(#examples),*]
            }
        }
    })
}

fn parse_role(role: &LitStr) -> syn::Result<proc_macro2::TokenStream> {
    match role.value().as_str() {
        "input" => Ok(quote! { crate::wire::WireSchemaRole::Input }),
        "output" => Ok(quote! { crate::wire::WireSchemaRole::Output }),
        _ => Err(syn::Error::new(
            role.span(),
            "wire_contract role must be `input` or `output`",
        )),
    }
}

fn parse_compatibility(compatibility: Option<&LitStr>) -> syn::Result<proc_macro2::TokenStream> {
    match compatibility.map(LitStr::value).as_deref() {
        None | Some("additive_patch") => {
            Ok(quote! { crate::wire::WireCompatibility::AdditivePatch })
        }
        Some("versioned_change") => Ok(quote! { crate::wire::WireCompatibility::VersionedChange }),
        Some(_) => Err(syn::Error::new(
            compatibility.expect("matched Some").span(),
            "wire_contract compatibility must be `additive_patch` or `versioned_change`",
        )),
    }
}

fn validate_schema_id(id: &LitStr, role: &LitStr) -> syn::Result<()> {
    let id_value = id.value();
    let segments: Vec<&str> = id_value.split('/').collect();
    let valid = matches!(segments.as_slice(), [namespace, probe, slug, direction, version]
        if *namespace == "amari.discovery"
            && *probe == "probe"
            && !slug.is_empty()
            && slug.chars().all(|character| {
                character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
            })
            && *direction == role.value()
            && version.starts_with('v')
            && version.len() > 1
            && version[1..].chars().all(|character| character.is_ascii_digit()));

    if valid {
        Ok(())
    } else {
        Err(syn::Error::new(
            id.span(),
            "wire_contract id must be `amari.discovery/probe/<lowercase-name>/<role>/v<N>` and match the declared role",
        ))
    }
}

fn validate_data_shape(input: &DeriveInput) -> syn::Result<()> {
    match &input.data {
        Data::Struct(data) => {
            validate_container_serde(input, &["deny_unknown_fields"])?;
            if !matches!(data.fields, Fields::Named(_)) {
                return Err(syn::Error::new(
                    input.ident.span(),
                    "WireContract supports only named-field structs",
                ));
            }
            validate_field_attributes(data.fields.iter())?;
        }
        Data::Enum(data) => {
            let serde = serde_container_meta(input)?;
            let has_untagged = serde.iter().any(|meta| meta.path().is_ident("untagged"));
            if has_untagged {
                return Err(syn::Error::new(
                    input.ident.span(),
                    "WireContract does not support untagged enums",
                ));
            }

            for meta in &serde {
                let allowed = meta.path().is_ident("tag")
                    || meta.path().is_ident("rename_all")
                    || meta.path().is_ident("deny_unknown_fields");
                if !allowed {
                    return Err(syn::Error::new(
                        meta.span(),
                        "WireContract enums support only serde(tag), serde(rename_all), and serde(deny_unknown_fields)",
                    ));
                }
            }

            let internally_tagged = serde.iter().any(|meta| {
                meta.path().is_ident("tag")
                    && matches!(
                        meta,
                        Meta::NameValue(name_value)
                            if matches!(
                                &name_value.value,
                                syn::Expr::Lit(expression)
                                    if matches!(&expression.lit, syn::Lit::Str(tag) if tag.value() == "kind")
                            )
                    )
            });
            for variant in &data.variants {
                if variant
                    .attrs
                    .iter()
                    .any(|attribute| attribute.path().is_ident("serde"))
                {
                    return Err(syn::Error::new(
                        variant.ident.span(),
                        "WireContract does not support variant-level serde attributes",
                    ));
                }
                let supported = if internally_tagged {
                    matches!(variant.fields, Fields::Named(_))
                } else {
                    matches!(variant.fields, Fields::Unit)
                };
                if !supported {
                    return Err(syn::Error::new(
                        variant.ident.span(),
                        if internally_tagged {
                            "internally tagged WireContract enum variants must use named fields"
                        } else {
                            "externally tagged WireContract enums support only unit variants"
                        },
                    ));
                }
                validate_field_attributes(variant.fields.iter())?;
            }
        }
        Data::Union(_) => {
            return Err(syn::Error::new(
                input.ident.span(),
                "WireContract does not support unions",
            ));
        }
    }
    Ok(())
}

fn validate_container_serde(input: &DeriveInput, allowed: &[&str]) -> syn::Result<()> {
    for meta in serde_container_meta(input)? {
        if !allowed.iter().any(|allowed| meta.path().is_ident(allowed)) {
            return Err(syn::Error::new(
                meta.span(),
                format!(
                    "WireContract structs support only these serde container attributes: {}",
                    allowed.join(", ")
                ),
            ));
        }
    }
    Ok(())
}

fn serde_container_meta(input: &DeriveInput) -> syn::Result<Vec<Meta>> {
    let mut result = Vec::new();
    for attribute in input
        .attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("serde"))
    {
        let metas = attribute
            .parse_args_with(syn::punctuated::Punctuated::<Meta, Token![,]>::parse_terminated)?;
        result.extend(metas);
    }
    Ok(result)
}

fn validate_field_attributes<'a>(fields: impl Iterator<Item = &'a syn::Field>) -> syn::Result<()> {
    for field in fields {
        if field
            .attrs
            .iter()
            .any(|attribute| attribute.path().is_ident("serde"))
        {
            return Err(syn::Error::new(
                field.span(),
                "WireContract does not support field-level serde attributes",
            ));
        }
    }
    Ok(())
}
