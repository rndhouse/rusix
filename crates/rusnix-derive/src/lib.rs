//! Structural Rusnix lowering, not general-purpose serialization.
use proc_macro::TokenStream;
use quote::{quote, quote_spanned};
use syn::{Data, DeriveInput, Fields, LitStr, parse_macro_input, parse_quote, spanned::Spanned};

mod config;

/// Automatically lower local struct types in an inline configuration module.
#[proc_macro_attribute]
pub fn config(args: TokenStream, input: TokenStream) -> TokenStream {
    if !args.is_empty() {
        return syn::Error::new_spanned(
            proc_macro2::TokenStream::from(args),
            "config takes no arguments; mark root structs with #[rusnix(root)]",
        )
        .into_compile_error()
        .into();
    }
    config::expand(parse_macro_input!(input as syn::ItemMod))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(IntoConfig, attributes(rusnix))]
pub fn into_config(input: TokenStream) -> TokenStream {
    expand(parse_macro_input!(input as DeriveInput), true)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(IntoRusnixValue, attributes(rusnix))]
pub fn into_value(input: TokenStream) -> TokenStream {
    expand(parse_macro_input!(input as DeriveInput), false)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[derive(Clone, Copy, Default)]
enum Naming {
    #[default]
    LowerCamel,
    Pascal,
}

impl Naming {
    fn field(self, name: &str) -> String {
        let name = name.strip_prefix("r#").unwrap_or(name);
        let mut output = String::new();
        for (index, word) in name.split('_').filter(|word| !word.is_empty()).enumerate() {
            let mut chars = word.chars();
            if let Some(first) = chars.next() {
                if index == 0 && matches!(self, Self::LowerCamel) {
                    output.extend(first.to_lowercase());
                } else {
                    output.extend(first.to_uppercase());
                }
                output.extend(chars);
            }
        }
        output
    }
}

fn expand(input: DeriveInput, rooted: bool) -> syn::Result<proc_macro2::TokenStream> {
    let mut rename_all = None;
    for attr in &input.attrs {
        if attr.path().is_ident("rusnix") {
            attr.parse_nested_meta(|meta| {
                if !meta.path.is_ident("rename_all") {
                    return Err(meta.error(
                        "supported struct mapping is rename_all; nesting defines placement",
                    ));
                }
                if rename_all.is_some() {
                    return Err(meta.error("duplicate rename_all"));
                }
                let name: LitStr = meta.value()?.parse()?;
                let convention = match name.value().as_str() {
                    "lowerCamelCase" => Naming::LowerCamel,
                    "PascalCase" => Naming::Pascal,
                    _ => {
                        return Err(syn::Error::new_spanned(
                            name,
                            "rename_all must be \"lowerCamelCase\" or \"PascalCase\"",
                        ));
                    }
                };
                rename_all = Some((convention, name));
                Ok(())
            })?;
        }
    }
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "derive supports structs; map enums explicitly with IntoRusnixValue and exhaustive match",
        ));
    };
    if let Some((_, name)) = &rename_all
        && !matches!(data.fields, Fields::Named(_))
    {
        return Err(syn::Error::new_spanned(
            name,
            "rename_all requires named fields; newtypes are transparent",
        ));
    }
    let naming = rename_all.map(|(naming, _)| naming).unwrap_or_default();
    let mut generics = input.generics.clone();
    let body = match &data.fields {
        Fields::Named(fields) => {
            let mut values = Vec::new();
            for field in &fields.named {
                let mut rename: Option<LitStr> = None;
                let mut skip = false;
                let mut flatten = false;
                for attr in &field.attrs {
                    if !attr.path().is_ident("rusnix") {
                        continue;
                    }
                    attr.parse_nested_meta(|meta| {
                        if meta.path.is_ident("rename") {
                            if rename.is_some() {
                                return Err(meta.error("duplicate rename"));
                            }
                            let name: LitStr = meta.value()?.parse()?;
                            if name.value().is_empty() || name.value().contains('\0') {
                                return Err(meta.error("rename must be nonempty and NUL-free"));
                            }
                            rename = Some(name);
                        } else if meta.path.is_ident("skip") {
                            if skip {
                                return Err(meta.error("duplicate skip"));
                            }
                            skip = true;
                        } else if meta.path.is_ident("flatten") {
                            if flatten {
                                return Err(meta.error("duplicate flatten"));
                            }
                            flatten = true;
                        } else {
                            return Err(meta
                                .error("supported field mappings are rename, skip, and flatten"));
                        }
                        Ok(())
                    })?;
                }
                if (skip && (flatten || rename.is_some())) || (flatten && rename.is_some()) {
                    return Err(syn::Error::new_spanned(
                        field,
                        "skip, flatten, and rename cannot be combined on one field",
                    ));
                }
                if skip {
                    continue;
                }
                let name = field.ident.as_ref().unwrap();
                let ty = &field.ty;
                generics
                    .make_where_clause()
                    .predicates
                    .push(parse_quote!(#ty: ::rusnix_ir::IntoRusnixValue));
                let key = if flatten {
                    quote!(None)
                } else {
                    let logical = match rename {
                        Some(name) => name,
                        None => {
                            let text = naming.field(&name.to_string());
                            if text.is_empty() {
                                return Err(syn::Error::new_spanned(
                                    name,
                                    "field name maps to an empty attribute; use an explicit rename",
                                ));
                            }
                            LitStr::new(&text, name.span())
                        }
                    };
                    quote!(Some(#logical))
                };
                values.push(quote_spanned!(field.span()=>
                    (#key, ::rusnix_ir::IntoRusnixValue::into_value(self.#name))
                ));
            }
            quote!(::rusnix_ir::RusnixValue::__record(vec![#(#values),*]))
        }
        Fields::Unnamed(fields) if !rooted && fields.unnamed.len() == 1 => {
            let field = fields.unnamed.first().unwrap();
            if field.attrs.iter().any(|a| a.path().is_ident("rusnix")) {
                return Err(syn::Error::new_spanned(
                    field,
                    "a newtype is transparent; mapping attributes require named fields",
                ));
            }
            let ty = &field.ty;
            generics
                .make_where_clause()
                .predicates
                .push(parse_quote!(#ty: ::rusnix_ir::IntoRusnixValue));
            quote!(::rusnix_ir::IntoRusnixValue::into_value(self.0))
        }
        _ => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "IntoConfig requires named fields; IntoRusnixValue also supports single-field newtypes",
            ));
        }
    };
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let config_impl = rooted.then(|| config_impl(&input.ident, &generics));
    Ok(quote!(
        impl #impl_generics ::rusnix_ir::IntoRusnixValue for #name #ty_generics #where_clause {
            #[track_caller]
            fn into_value(self) -> ::rusnix_ir::RusnixValue { #body }
        }

        #config_impl
    ))
}

fn config_impl(name: &syn::Ident, generics: &syn::Generics) -> proc_macro2::TokenStream {
    let mut generics = generics.clone();
    generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(Self: ::rusnix_ir::IntoRusnixValue));
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    quote!(
        impl #impl_generics ::rusnix_ir::IntoConfig for #name #ty_generics #where_clause {
            #[track_caller]
            fn into_config(self) -> ::rusnix_ir::Config {
                ::rusnix_ir::Config::from_value(::rusnix_ir::IntoRusnixValue::into_value(self))
            }
        }
    )
}

#[cfg(test)]
mod tests {
    use super::Naming;

    #[test]
    fn lower_camel_joins_snake_case_words() {
        for (rust, nix) in [
            ("enable_feature", "enableFeature"),
            ("listen_port", "listenPort"),
            ("service_config", "serviceConfig"),
            ("http_port", "httpPort"),
            ("r#type", "type"),
            ("alreadyCamel", "alreadyCamel"),
            ("_local__value_", "localValue"),
        ] {
            assert_eq!(Naming::LowerCamel.field(rust), nix);
        }
    }

    #[test]
    fn pascal_capitalizes_each_snake_case_word() {
        for (rust, nix) in [
            ("exec_start", "ExecStart"),
            ("restart", "Restart"),
            ("user", "User"),
            ("working_directory", "WorkingDirectory"),
            ("r#type", "Type"),
        ] {
            assert_eq!(Naming::Pascal.field(rust), nix);
        }
    }
}
