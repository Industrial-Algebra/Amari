//! Procedural macros for amari-flynn probabilistic contracts
//!
//! These macros provide syntactic sugar for probabilistic contract verification,
//! integrating with the Flynn verification framework.
//!
//! Supports zero-parameter, single-parameter, and multi-parameter functions.
//! For multi-parameter functions, the verification helper accepts a generator
//! that returns a tuple of all parameter types.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, Expr, FnArg, Ident, ItemFn, LitFloat, Token};

/// Parsed attribute for probabilistic requires/ensures
struct ProbabilisticAttr {
    condition: Expr,
    _comma: Token![,],
    bound: LitFloat,
}

impl Parse for ProbabilisticAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(ProbabilisticAttr {
            condition: input.parse()?,
            _comma: input.parse()?,
            bound: input.parse()?,
        })
    }
}

/// Parsed attribute for expected value constraints
struct ExpectedValueAttr {
    expression: Expr,
    _comma: Token![,],
    expected: LitFloat,
    _comma2: Token![,],
    epsilon: LitFloat,
}

impl Parse for ExpectedValueAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(ExpectedValueAttr {
            expression: input.parse()?,
            _comma: input.parse()?,
            expected: input.parse()?,
            _comma2: input.parse()?,
            epsilon: input.parse()?,
        })
    }
}

/// Result of extracting parameters from a function signature.
///
/// - `param_type`: The type the generator returns (single type or tuple)
/// - `param_bindings`: `let` bindings to destructure inputs
/// - `param_names`: Individual parameter idents for fn calls
struct ExtractedParams {
    param_type: TokenStream2,
    param_bindings: TokenStream2,
    param_names: Vec<Ident>,
}

/// Extract parameter types, destructuring bindings, and names from function inputs.
///
/// Returns `None` if any parameter has a non-ident pattern (e.g. `(a, b): (T, U)`)
/// or is a receiver (`self`/`&self`).
fn extract_multi_params(
    inputs: &syn::punctuated::Punctuated<FnArg, Token![,]>,
) -> Option<ExtractedParams> {
    let mut types = Vec::new();
    let mut names = Vec::new();

    for input in inputs {
        match input {
            FnArg::Receiver(_) => return None, // skip methods
            FnArg::Typed(pat_type) => {
                let ty = &pat_type.ty;
                if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                    types.push(quote! { #ty });
                    names.push(pat_ident.ident.clone());
                } else {
                    return None; // non-ident pattern, can't destructure
                }
            }
        }
    }

    if names.is_empty() {
        return None;
    }

    let param_type = quote! { (#(#types),*) };
    let param_bindings = quote! { let (#(#names),*) = inputs; };

    Some(ExtractedParams {
        param_type,
        param_bindings,
        param_names: names,
    })
}

/// Probabilistic precondition
///
/// Generates documentation and verification infrastructure for preconditions.
///
/// # Syntax
///
/// ```ignore
/// #[prob_requires(condition, probability_bound)]
/// ```
///
/// - `condition`: Boolean expression that should hold for inputs
/// - `probability_bound`: MINIMUM probability the condition HOLDS
///   (hold frame; 0.0 to 1.0). For the violation frame use
///   [`prob_violation_at_most`].
///
/// # Example
///
/// ```ignore
/// use amari_flynn_macros::prob_requires;
///
/// #[prob_requires(x > 0.0, 0.95)]
/// fn compute(x: f64) -> f64 {
///     x.sqrt()
/// }
/// ```
///
/// This documents that the function expects x > 0.0 to hold with probability >= 0.95.
///
/// Multi-parameter functions are also supported:
///
/// ```ignore
/// #[prob_requires(x > 0.0 && y > 0.0, 0.9)]
/// fn compute_pair(x: f64, y: f64) -> f64 {
///     (x * y).sqrt()
/// }
/// ```
#[proc_macro_attribute]
pub fn prob_requires(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    // Parse the attribute ONCE; malformed contracts fail expansion loudly
    // (never silently degrade to documentation defaults — audit U5/F10).
    let parsed = if attr.is_empty() {
        None
    } else {
        match syn::parse::<ProbabilisticAttr>(attr.clone()) {
            Ok(parsed) => Some(parsed),
            Err(err) => {
                return compile_error_spanned(err.to_compile_error());
            }
        }
    };

    let fn_name = &input.sig.ident;
    let verify_fn_name =
        syn::Ident::new(&format!("verify_{}_precondition", fn_name), fn_name.span());

    let (doc_string, verification_helper) = match &parsed {
        None => (
            "Probabilistic precondition: contract not yet specified".to_string(),
            quote! {},
        ),
        Some(parsed_attr) => {
            let condition = &parsed_attr.condition;
            let bound = parsed_attr
                .bound
                .base10_parse::<f64>()
                .expect("probability bound literal must be a valid f64");
            let doc_string = format!(
                "Probabilistic precondition: Requires `{}` with P >= {bound}",
                quote!(#condition)
            );
            let (param_type, param_bindings) = extract_binding_shape(&input);
            let helper = quote! {
                #[cfg(test)]
                #[doc = concat!("Verification helper for ", stringify!(#fn_name), " precondition (hold frame: P(condition) >= ", stringify!(#bound), ")")]
                #[allow(dead_code)]
                fn #verify_fn_name<F>(
                    input_generator: F,
                    samples: usize,
                ) -> amari_flynn::contracts::VerificationResult
                where
                    F: Fn() -> #param_type,
                {
                    let verifier = amari_flynn::backend::monte_carlo::MonteCarloVerifier::new(samples);
                    verifier.verify_probability_at_least(
                        || {
                            let inputs = input_generator();
                            #param_bindings
                            #condition
                        },
                        #bound,
                    )
                }
            };
            (doc_string, helper)
        }
    };

    let output = quote! {
        #[doc = #doc_string]
        #input

        #verification_helper
    };

    output.into()
}

fn compile_error_spanned(tokens: proc_macro2::TokenStream) -> TokenStream {
    tokens.into()
}

/// Extract (generator return type, destructure bindings) for the function.
fn extract_binding_shape(input: &ItemFn) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    if input.sig.inputs.is_empty() {
        (quote! { () }, quote! {})
    } else if input.sig.inputs.len() == 1 {
        if let Some(FnArg::Typed(pat_type)) = input.sig.inputs.first() {
            let param_type = &pat_type.ty;
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                let param_name = &pat_ident.ident;
                return (quote! { #param_type }, quote! { let #param_name = inputs; });
            }
        }
        (quote! { core::marker::PhantomData<()> }, quote! {})
    } else {
        match extract_multi_params(&input.sig.inputs) {
            Some(extracted) => (extracted.param_type, extracted.param_bindings),
            None => (quote! { core::marker::PhantomData<()> }, quote! {}),
        }
    }
}

/// Probabilistic postcondition
///
/// Generates documentation and verification infrastructure for postconditions.
///
/// # Syntax
///
/// ```ignore
/// #[prob_ensures(condition, probability_bound)]
/// ```
///
/// - `condition`: Boolean expression that should hold for outputs
/// - `probability_bound`: MINIMUM probability the condition HOLDS
///   (hold frame; 0.0 to 1.0). For the violation frame use
///   [`prob_violation_at_most`].
///
/// # Example
///
/// ```ignore
/// use amari_flynn_macros::prob_ensures;
///
/// #[prob_ensures(result >= 0.0, 0.99)]
/// fn compute(x: f64) -> f64 {
///     x.abs()
/// }
/// ```
///
/// This documents that the result should be non-negative with probability >= 0.99.
///
/// Multi-parameter functions are also supported:
///
/// ```ignore
/// #[prob_ensures(result >= 0.0, 0.99)]
/// fn sum_abs(x: f64, y: f64) -> f64 {
///     x.abs() + y.abs()
/// }
/// ```
#[proc_macro_attribute]
pub fn prob_ensures(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    let parsed = if attr.is_empty() {
        None
    } else {
        match syn::parse::<ProbabilisticAttr>(attr.clone()) {
            Ok(parsed) => Some(parsed),
            Err(err) => return err.to_compile_error().into(),
        }
    };

    let fn_name = &input.sig.ident;
    let verify_fn_name =
        syn::Ident::new(&format!("verify_{}_postcondition", fn_name), fn_name.span());

    let (doc_string, verification_helper) = match &parsed {
        None => (
            "Probabilistic postcondition: contract not yet specified".to_string(),
            quote! {},
        ),
        Some(parsed_attr) => {
            let condition = &parsed_attr.condition;
            let bound = parsed_attr
                .bound
                .base10_parse::<f64>()
                .expect("probability bound literal must be a valid f64");
            let doc_string = format!(
                "Probabilistic postcondition: Ensures `{}` with P >= {bound}",
                quote!(#condition)
            );
            let (param_type, param_bindings, fn_call) = extract_call_shape(&input);
            let helper = quote! {
                #[cfg(test)]
                #[doc = concat!("Verification helper for ", stringify!(#fn_name), " postcondition (hold frame: P(condition) >= ", stringify!(#bound), ")")]
                #[allow(dead_code)]
                fn #verify_fn_name<F>(
                    input_generator: F,
                    samples: usize,
                ) -> amari_flynn::contracts::VerificationResult
                where
                    F: Fn() -> #param_type,
                {
                    let verifier = amari_flynn::backend::monte_carlo::MonteCarloVerifier::new(samples);
                    verifier.verify_probability_at_least(
                        || {
                            let inputs = input_generator();
                            #param_bindings
                            let result = #fn_call;
                            #condition
                        },
                        #bound,
                    )
                }
            };
            (doc_string, helper)
        }
    };

    let output = quote! {
        #[doc = #doc_string]
        #input

        #verification_helper
    };

    output.into()
}

/// (generator type, destructure bindings, fn call) for ensures-style
/// helpers that must CALL the annotated function.
fn extract_call_shape(
    input: &ItemFn,
) -> (
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
) {
    let fn_name = &input.sig.ident;
    if input.sig.inputs.is_empty() {
        (quote! { () }, quote! {}, quote! { #fn_name() })
    } else if input.sig.inputs.len() == 1 {
        if let Some(FnArg::Typed(pat_type)) = input.sig.inputs.first() {
            let param_type = &pat_type.ty;
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                let param_name = &pat_ident.ident;
                return (
                    quote! { #param_type },
                    quote! { let #param_name = inputs; },
                    quote! { #fn_name(inputs) },
                );
            }
        }
        (
            quote! { core::marker::PhantomData<()> },
            quote! {},
            quote! { #fn_name(inputs) },
        )
    } else {
        match extract_multi_params(&input.sig.inputs) {
            Some(extracted) => {
                let names = &extracted.param_names;
                (
                    extracted.param_type,
                    extracted.param_bindings,
                    quote! { #fn_name(#(#names),*) },
                )
            }
            None => (
                quote! { core::marker::PhantomData<()> },
                quote! {},
                quote! { #fn_name(inputs) },
            ),
        }
    }
}

/// Expected value constraint
///
/// Generates documentation and verification for expected value bounds.
///
/// # Syntax
///
/// ```ignore
/// #[ensures_expected(expression, expected_value, epsilon)]
/// ```
///
/// - `expression`: Expression to evaluate
/// - `expected_value`: Expected mean value
/// - `epsilon`: Maximum deviation from expected value
///
/// # Example
///
/// ```ignore
/// use amari_flynn_macros::ensures_expected;
///
/// #[ensures_expected(result, 5.0, 0.1)]
/// fn random_around_five() -> f64 {
///     5.0 + (rand::random::<f64>() - 0.5) * 0.2
/// }
/// ```
///
/// This documents that the result should have expected value 5.0 +/- 0.1.
///
/// Multi-parameter functions are also supported:
///
/// ```ignore
/// #[ensures_expected(result, 0.0, 0.5)]
/// fn weighted_diff(x: f64, y: f64) -> f64 {
///     x - y
/// }
/// ```
/// Probabilistic violation bound (the named violation frame, D1(c)).
///
/// # Syntax
///
/// ```ignore
/// #[prob_violation_at_most(violation_condition, delta)]
/// ```
///
/// - `violation_condition`: Boolean expression that is TRUE when the BAD
///   event occurs
/// - `delta`: maximum allowed probability of the violation (0.0 to 1.0)
///
/// The generated test helper verifies `P(violation) <= delta` via
/// `MonteCarloVerifier::verify_violation_bound`.
#[proc_macro_attribute]
pub fn prob_violation_at_most(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    let parsed = if attr.is_empty() {
        None
    } else {
        match syn::parse::<ProbabilisticAttr>(attr.clone()) {
            Ok(parsed) => Some(parsed),
            Err(err) => return err.to_compile_error().into(),
        }
    };

    let fn_name = &input.sig.ident;
    let verify_fn_name = syn::Ident::new(
        &format!("verify_{}_violation_bound", fn_name),
        fn_name.span(),
    );

    let (doc_string, verification_helper) = match &parsed {
        None => (
            "Probabilistic violation bound: contract not yet specified".to_string(),
            quote! {},
        ),
        Some(parsed_attr) => {
            let condition = &parsed_attr.condition;
            let bound = parsed_attr
                .bound
                .base10_parse::<f64>()
                .expect("delta literal must be a valid f64");
            let doc_string = format!(
                "Probabilistic violation bound: `{}` with P <= {bound} (violation frame)",
                quote!(#condition)
            );
            let (param_type, param_bindings) = extract_binding_shape(&input);
            let helper = quote! {
                #[cfg(test)]
                #[doc = concat!("Verification helper for ", stringify!(#fn_name), " violation bound (P(violation) <= ", stringify!(#bound), ")")]
                #[allow(dead_code)]
                fn #verify_fn_name<F>(
                    input_generator: F,
                    samples: usize,
                ) -> amari_flynn::contracts::VerificationResult
                where
                    F: Fn() -> #param_type,
                {
                    let verifier = amari_flynn::backend::monte_carlo::MonteCarloVerifier::new(samples);
                    verifier.verify_violation_bound(
                        || {
                            let inputs = input_generator();
                            #param_bindings
                            #condition
                        },
                        #bound,
                    )
                }
            };
            (doc_string, helper)
        }
    };

    let output = quote! {
        #[doc = #doc_string]
        #input

        #verification_helper
    };

    output.into()
}

#[proc_macro_attribute]
pub fn ensures_expected(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    let parsed = if attr.is_empty() {
        None
    } else {
        match syn::parse::<ExpectedValueAttr>(attr.clone()) {
            Ok(parsed) => Some(parsed),
            Err(err) => return err.to_compile_error().into(),
        }
    };

    let fn_name = &input.sig.ident;
    let verify_fn_name = syn::Ident::new(
        &format!("verify_{}_expected_value", fn_name),
        fn_name.span(),
    );

    let (doc_string, verification_helper) = match &parsed {
        None => (
            "Expected value constraint: contract not yet specified".to_string(),
            quote! {},
        ),
        Some(parsed_attr) => {
            let expression = &parsed_attr.expression;
            let expected = parsed_attr
                .expected
                .base10_parse::<f64>()
                .expect("expected value literal must be a valid f64");
            let epsilon = parsed_attr
                .epsilon
                .base10_parse::<f64>()
                .expect("epsilon literal must be a valid f64");
            let doc_string = format!(
                "Expected value constraint (SMOKE CHECK — fixed-threshold mean comparison, \
                 not a confidence-interval verdict): E[{}] = {expected} +/- {epsilon}",
                quote!(#expression)
            );
            let (param_type, param_bindings, fn_call) = extract_call_shape(&input);
            let helper = quote! {
                #[cfg(test)]
                #[doc = concat!("Verification helper for ", stringify!(#fn_name), " expected value (smoke check: |mean - expected| <= epsilon)")]
                #[allow(dead_code)]
                fn #verify_fn_name<F>(
                    input_generator: F,
                    samples: usize,
                ) -> bool
                where
                    F: Fn() -> #param_type,
                {
                    let mut sum = 0.0f64;
                    for _ in 0..samples {
                        let inputs = input_generator();
                        #param_bindings
                        let result = #fn_call;
                        sum += (#expression);
                    }
                    let mean = sum / samples as f64;
                    (mean - #expected).abs() <= #epsilon
                }
            };
            (doc_string, helper)
        }
    };

    let output = quote! {
        #[doc = #doc_string]
        #input

        #verification_helper
    };

    output.into()
}
