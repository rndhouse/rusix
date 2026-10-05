//! Named interpolation expands into the existing deferred text operations.
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use std::collections::{BTreeMap, BTreeSet};
use syn::{Expr, Ident, LitStr, Path, Token, parse::Parse, spanned::Spanned};

pub struct Input {
    value_type: Path,
    template: LitStr,
    arguments: Vec<(Ident, Expr)>,
}

impl Parse for Input {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let value_type = input.parse()?;
        input.parse::<Token![;]>()?;
        let template = input.parse()?;
        let mut arguments = Vec::new();
        while !input.is_empty() {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
            let name = input.parse()?;
            input.parse::<Token![=]>()?;
            arguments.push((name, input.parse()?));
        }
        Ok(Self {
            value_type,
            template,
            arguments,
        })
    }
}

enum Part {
    Literal(String),
    Argument(usize),
}

pub fn expand(input: Input) -> syn::Result<TokenStream> {
    let Input {
        value_type,
        template,
        arguments,
    } = input;
    let mut names = BTreeMap::new();
    for (index, (name, _)) in arguments.iter().enumerate() {
        if names.insert(name.to_string(), index).is_some() {
            return Err(syn::Error::new(name.span(), "duplicate nix_text argument"));
        }
    }
    let fail = |message| syn::Error::new(template.span(), message);
    let text = template.value();
    let mut chars = text.chars().peekable();
    let mut literal = String::new();
    let mut parts = Vec::new();
    let mut used = BTreeSet::new();
    while let Some(ch) = chars.next() {
        match ch {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                literal.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                literal.push('}');
            }
            '{' => {
                let mut name = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some('{') => return Err(fail("nested brace in nix_text placeholder")),
                        Some(ch) => name.push(ch),
                        None => return Err(fail("unclosed nix_text placeholder")),
                    }
                }
                if syn::parse_str::<Ident>(&name).is_err() {
                    return Err(fail(
                        "nix_text supports only named placeholders like {name}; formatting syntax is unsupported",
                    ));
                }
                let Some(&index) = names.get(&name) else {
                    return Err(fail(&format!("unknown nix_text placeholder {{{name}}}")));
                };
                used.insert(index);
                if !literal.is_empty() {
                    parts.push(Part::Literal(std::mem::take(&mut literal)));
                }
                parts.push(Part::Argument(index));
            }
            '}' => {
                return Err(fail(
                    "unmatched closing brace in nix_text; use }} for a literal brace",
                ));
            }
            ch => literal.push(ch),
        }
    }
    if !literal.is_empty() {
        parts.push(Part::Literal(literal));
    }
    for (index, (name, _)) in arguments.iter().enumerate() {
        if !used.contains(&index) {
            return Err(syn::Error::new(name.span(), "unused nix_text argument"));
        }
    }
    let bindings: Vec<_> = (0..arguments.len())
        .map(|index| format_ident!("__rusnix_text_arg_{index}", span = Span::mixed_site()))
        .collect();
    let declarations = arguments.iter().zip(&bindings).map(|((_, expr), binding)| {
        quote_spanned! {expr.span()=> let #binding = #value_type::from(#expr).to_text(); }
    });
    let values = parts.into_iter().map(|part| match part {
        Part::Literal(text) => {
            let text = LitStr::new(&text, template.span());
            quote! { #value_type::from(#text) }
        }
        Part::Argument(index) => {
            let binding = &bindings[index];
            quote! { #binding.clone() }
        }
    });
    Ok(quote! {{
        #(#declarations)*
        #value_type::concat_text([#(#values),*])
    }})
}
