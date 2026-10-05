//! Finite reference views generate the existing OptionRef constructors.
use super::Naming;
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use std::collections::{BTreeMap, BTreeSet};
use syn::{Fields, Ident, Item, ItemMod, LitStr, PathArguments, Type, spanned::Spanned};

struct View {
    item: syn::ItemStruct,
    root: bool,
    value: bool,
    naming: Naming,
}

enum Leaf {
    Scalar,
    Opaque,
    Branch(Ident),
}

fn classify(ty: &Type, locals: &BTreeSet<String>) -> syn::Result<Leaf> {
    if let Type::Path(ty) = ty
        && ty.qself.is_none()
    {
        let path = &ty.path;
        let names: Vec<_> = path.segments.iter().map(|s| s.ident.to_string()).collect();
        let last = path.segments.last().unwrap();

        if names.len() == 1 && locals.contains(&names[0]) {
            if !matches!(last.arguments, PathArguments::None) {
                return Err(syn::Error::new_spanned(
                    ty,
                    "option views cannot have type arguments",
                ));
            }
            return Ok(Leaf::Branch(last.ident.clone()));
        }

        let qualified = names[..names.len() - 1].join("::");
        let expected_namespace = match last.ident.to_string().as_str() {
            "bool" | "i64" => "std::primitive",
            "String" => "std::string",
            "NixValue" => "rusnix_ir::interop",
            "Option" => "std::option",
            "Vec" => "std::vec",
            "BTreeMap" | "HashMap" => "std::collections",
            _ => "",
        };
        let accepted = qualified.is_empty() || qualified == expected_namespace;

        if accepted {
            match last.ident.to_string().as_str() {
                "bool" | "String" | "i64" if matches!(last.arguments, PathArguments::None) => {
                    return Ok(Leaf::Scalar);
                }
                "NixValue" if matches!(last.arguments, PathArguments::None) => {
                    return Ok(Leaf::Opaque);
                }
                "Option" | "Vec" | "BTreeMap" | "HashMap" => {
                    if let PathArguments::AngleBracketed(args) = &last.arguments {
                        let expected =
                            if matches!(last.ident.to_string().as_str(), "BTreeMap" | "HashMap") {
                                2
                            } else {
                                1
                            };
                        if args.args.len() == expected
                            && args
                                .args
                                .iter()
                                .all(|a| matches!(a, syn::GenericArgument::Type(_)))
                        {
                            return Ok(Leaf::Opaque);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    Err(syn::Error::new_spanned(
        ty,
        "unsupported option-view type; use bool, String, i64, NixValue, Option<T>, Vec<T>, BTreeMap<K, V>, HashMap<K, V>, or a local named view; external aliases require explicit OptionRef",
    ))
}

fn parse_view(mut item: syn::ItemStruct) -> syn::Result<View> {
    if !item.generics.params.is_empty() || item.generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(
            &item.generics,
            "option views do not support generics",
        ));
    }

    if !matches!(item.fields, Fields::Named(_)) {
        return Err(syn::Error::new_spanned(
            &item,
            "option views require named structs",
        ));
    }

    let mut root = false;
    let mut value = false;
    let mut naming = None;
    let mut attrs = Vec::new();

    for attr in item.attrs.drain(..) {
        if !attr.path().is_ident("rusnix") {
            if attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr") {
                return Err(syn::Error::new_spanned(
                    attr,
                    "conditional option views are not supported; declare a separate view",
                ));
            }
            if !attr.path().is_ident("doc") {
                return Err(syn::Error::new_spanned(
                    attr,
                    "option views are reference declarations; only documentation and rusnix mapping attributes are supported",
                ));
            }

            attrs.push(attr);
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("root") || meta.path.is_ident("value") {
                let flag = if meta.path.is_ident("root") {
                    &mut root
                } else {
                    &mut value
                };
                if *flag {
                    return Err(meta.error("duplicate option-view marker"));
                }
                *flag = true;
                Ok(())
            } else if meta.path.is_ident("rename_all") {
                if naming.is_some() {
                    return Err(meta.error("duplicate rename_all"));
                }
                naming = Some(Naming::parse(&meta.value()?.parse::<LitStr>()?)?);
                Ok(())
            } else {
                Err(meta.error("option-view mappings are root, value, and rename_all"))
            }
        })?;
    }

    if root && value {
        return Err(syn::Error::new_spanned(
            &item.ident,
            "whole-root access is not supported; mark a specific subtree with #[rusnix(value)]",
        ));
    }

    item.attrs = attrs;

    Ok(View {
        item,
        root,
        value,
        naming: naming.unwrap_or_default(),
    })
}

fn key(field: &syn::Field, naming: Naming) -> syn::Result<LitStr> {
    let mut rename = None;

    for attr in &field.attrs {
        if attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr") {
            return Err(syn::Error::new_spanned(
                attr,
                "conditional option-view fields are not supported",
            ));
        }

        if attr.path().is_ident("rusnix") {
            attr.parse_nested_meta(|meta| {
                if !meta.path.is_ident("rename") {
                    return Err(meta.error("option-view fields support rename only"));
                }
                if rename.is_some() {
                    return Err(meta.error("duplicate rename"));
                }
                rename = Some(meta.value()?.parse()?);
                Ok(())
            })?;
        }
    }

    naming.mapped_field(field.ident.as_ref().unwrap(), rename)
}

fn check_cycles(
    name: &str,
    graph: &BTreeMap<String, Vec<(String, proc_macro2::Span)>>,
    active: &mut BTreeSet<String>,
    done: &mut BTreeSet<String>,
) -> syn::Result<()> {
    if done.contains(name) {
        return Ok(());
    }

    active.insert(name.into());

    for (child, span) in &graph[name] {
        if active.contains(child) {
            return Err(syn::Error::new(
                *span,
                "recursive option views are not supported; declare a finite set of paths",
            ));
        }

        check_cycles(child, graph, active, done)?;
    }

    active.remove(name);
    done.insert(name.into());

    Ok(())
}

pub(super) fn expand(mut module: ItemMod) -> syn::Result<TokenStream> {
    let Some((_, items)) = module.content.take() else {
        return Err(syn::Error::new_spanned(
            module,
            "options requires an inline module; external types use explicit OptionRef",
        ));
    };

    let mut locals = BTreeSet::new();

    for item in &items {
        if let Item::Struct(item) = item {
            let name = item.ident.to_string();
            if matches!(
                name.as_str(),
                "String" | "NixValue" | "Option" | "Vec" | "BTreeMap" | "HashMap"
            ) || !locals.insert(name)
            {
                return Err(syn::Error::new_spanned(
                    &item.ident,
                    "ambiguous option-view type name",
                ));
            }
        }
    }

    let mut views = Vec::new();
    let mut retained = Vec::new();

    for item in items {
        match item {
            Item::Struct(item) => views.push(parse_view(item)?),
            Item::Use(_) => retained.push(item),
            _ => {
                return Err(syn::Error::new_spanned(
                    item,
                    "options modules contain named view structs and imports only; keep domain code outside the reference declarations",
                ));
            }
        }
    }

    let roots: Vec<_> = views.iter().filter(|v| v.root).collect();
    if roots.len() != 1 {
        return Err(syn::Error::new_spanned(
            &module.ident,
            "options requires exactly one #[rusnix(root)] struct",
        ));
    }

    let root = roots[0].item.ident.clone();
    let mut graph = BTreeMap::new();

    for view in &views {
        let mut keys = BTreeSet::new();
        let mut children = Vec::new();

        for field in &view.item.fields {
            let name = field.ident.as_ref().unwrap();
            if matches!(name.to_string().as_str(), "__rusnix_path" | "__rusnix_at")
                || (view.value && name == "as_value")
            {
                return Err(syn::Error::new_spanned(
                    name,
                    "field collides with a generated option-view member",
                ));
            }

            let path_key = key(field, view.naming)?;
            if !keys.insert(path_key.value()) {
                return Err(syn::Error::new_spanned(
                    field,
                    "ambiguous option path: fields map to the same attribute",
                ));
            }

            if let Leaf::Branch(child) = classify(&field.ty, &locals)? {
                if child == root {
                    return Err(syn::Error::new_spanned(
                        &field.ty,
                        "a root cannot be nested in an option view",
                    ));
                }
                children.push((child.to_string(), field.ty.span()));
            }
        }

        graph.insert(view.item.ident.to_string(), children);
    }

    let mut done = BTreeSet::new();

    for name in graph.keys() {
        check_cycles(name, &graph, &mut BTreeSet::new(), &mut done)?;
    }

    let mut generated = Vec::new();

    for view in views {
        let name = &view.item.ident;
        let attrs = &view.item.attrs;
        let type_docs = (!attrs.iter().any(|attr| attr.path().is_ident("doc"))).then(|| {
            quote!(#[doc = "Navigation over declared final NixOS option dependencies; values remain symbolic."])
        });
        let mut branches = Vec::new();
        let mut initialize = Vec::new();
        let mut methods = Vec::new();

        for field in &view.item.fields {
            let field_name = field.ident.as_ref().unwrap();
            let ty = &field.ty;
            let literal = key(field, view.naming)?;
            let mut docs: Vec<syn::Attribute> = field
                .attrs
                .iter()
                .filter(|attr| attr.path().is_ident("doc"))
                .cloned()
                .collect();

            if docs.is_empty() {
                let description = match classify(ty, &locals)? {
                    Leaf::Branch(_) => {
                        "Navigate to this declared option subtree without reading or evaluating it."
                    }
                    Leaf::Scalar => {
                        "Create a typed symbolic dependency; NixOS resolves and validates the final value after merging."
                    }
                    _ => {
                        "Create an opaque symbolic dependency; this does not materialize a Rust collection or value."
                    }
                };
                docs.push(syn::parse_quote!(#[doc = #description]));
            }

            match classify(ty, &locals)? {
                Leaf::Branch(child) => {
                    branches
                        .push(quote_spanned!(field.span()=> #(#docs)* pub #field_name: #child,));
                    initialize.push(quote!(#field_name: {
                        let mut child_path = path.clone();
                        child_path.push(#literal.into());
                        #child::__rusnix_at(child_path)
                    },));
                }
                leaf => {
                    let (result, conversion) = match leaf {
                        Leaf::Scalar => (quote!(::rusnix_ir::Expr<#ty>), quote!(into_expr)),
                        _ => (quote!(::rusnix_ir::interop::NixValue), quote!(into_value)),
                    };
                    methods.push(quote_spanned!(field.span()=>
                        #(#docs)*
                        #[track_caller]
                        pub fn #field_name(&self) -> #result {
                            let mut path = self.__rusnix_path.clone();
                            path.push(#literal.into());
                            ::rusnix_ir::nixos::OptionRef::<#ty>::from_segments(path).#conversion()
                        }
                    ));
                }
            }
        }

        let as_value = view.value.then(|| {
            quote!(
                /// Reference the whole declared subtree as a deferred opaque Nix value.
                #[track_caller]
                pub fn as_value(&self) -> ::rusnix_ir::interop::NixValue {
                    ::rusnix_ir::nixos::OptionRef::<::rusnix_ir::interop::NixValue>::from_segments(
                        self.__rusnix_path.clone(),
                    )
                    .into_value()
                }
            )
        });

        generated.push(quote!(
            #(#attrs)*
            #type_docs
            pub struct #name {
                __rusnix_path: ::std::vec::Vec<::std::string::String>,
                #(#branches)*
            }

            impl #name {
                fn __rusnix_at(path: ::std::vec::Vec<::std::string::String>) -> Self {
                    Self { #(#initialize)* __rusnix_path: path }
                }

                #(#methods)*

                #as_value
            }
        ));
    }

    let attrs = &module.attrs;
    let visibility = &module.vis;
    let module_name = &module.ident;

    Ok(quote!(
        #(#attrs)*
        #visibility mod #module_name {
            #(#retained)*

            #(#generated)*

            /// Begin navigation; leaf accessor calls capture their own Rust caller origins.
            pub fn root() -> #root {
                #root::__rusnix_at(::std::vec::Vec::new())
            }
        }
    ))
}

#[cfg(test)]
mod tests {
    use super::expand;

    #[test]
    fn public_views_are_documented_and_preserve_authored_docs() {
        let output = expand(syn::parse_quote! {
            mod options {
                #[rusnix(root)]
                struct Root {
                    settings: Settings,
                }

                /// Settings explicitly used by this adapter.
                #[rusnix(value)]
                struct Settings {
                    /// Port selected by ordinary NixOS merging.
                    port: i64,
                    enabled: bool,
                }
            }
        })
        .unwrap();
        let module: syn::ItemMod = syn::parse2(output).unwrap();
        let mut documented = 0;
        let mut authored = Vec::new();
        let mut check = |attrs: &[syn::Attribute]| {
            let docs: Vec<_> = attrs
                .iter()
                .filter(|attr| attr.path().is_ident("doc"))
                .collect();
            assert!(!docs.is_empty());
            documented += 1;
            authored.extend(
                docs.into_iter()
                    .map(|attr| quote::quote!(#attr).to_string()),
            );
        };

        for item in module.content.unwrap().1 {
            match item {
                syn::Item::Struct(item) => {
                    check(&item.attrs);
                    for field in item.fields {
                        if matches!(field.vis, syn::Visibility::Public(_)) {
                            check(&field.attrs);
                        }
                    }
                }
                syn::Item::Impl(item) => {
                    for member in item.items {
                        if let syn::ImplItem::Fn(method) = member
                            && matches!(method.vis, syn::Visibility::Public(_))
                        {
                            check(&method.attrs);
                        }
                    }
                }
                syn::Item::Fn(item) => check(&item.attrs),
                _ => {}
            }
        }

        assert_eq!(documented, 7);
        assert!(
            authored
                .iter()
                .any(|doc| doc.contains("Settings explicitly used by this adapter."))
        );
        assert!(
            authored
                .iter()
                .any(|doc| doc.contains("Port selected by ordinary NixOS merging."))
        );
    }

    fn rejection(source: &str) -> String {
        expand(syn::parse_str(source).unwrap())
            .unwrap_err()
            .to_string()
    }

    #[test]
    fn external_modules_and_missing_roots_are_rejected() {
        assert!(rejection("mod options;").contains("inline module"));
        assert!(rejection("mod options { struct Value { port: i64 } }").contains("exactly one"));
    }

    #[test]
    fn invalid_naming_and_reserved_members_are_rejected() {
        assert!(rejection(r#"mod options { #[rusnix(root, rename_all = "kebab-case")] struct Root { port: i64 } }"#).contains("rename_all must be"));
        assert!(rejection(r#"mod options { #[rusnix(root)] struct Root { #[rusnix(rename = "")] port: i64 } }"#).contains("nonempty"));
        assert!(
            rejection("mod options { #[rusnix(root)] struct Root { __rusnix_path: i64 } }")
                .contains("collides")
        );
    }

    #[test]
    fn indirect_recursion_and_ambiguous_type_names_are_rejected() {
        assert!(rejection("mod options { #[rusnix(root)] struct Root { a: A } struct A { b: B } struct B { a: A } }").contains("recursive"));
        assert!(rejection("mod options { #[rusnix(root)] struct Root { value: String } struct String { field: i64 } }").contains("ambiguous"));
    }

    #[test]
    fn generic_conditional_and_flattened_views_are_rejected() {
        assert!(
            rejection("mod options { #[rusnix(root)] struct Root<T> { value: T } }")
                .contains("generics")
        );
        assert!(
            rejection("mod options { #[rusnix(root)] struct Root { #[cfg(unix)] value: i64 } }")
                .contains("conditional")
        );
        assert!(
            rejection(
                "mod options { #[rusnix(root)] struct Root { #[rusnix(flatten)] value: i64 } }"
            )
            .contains("rename only")
        );
    }

    #[test]
    fn default_derives_cannot_create_unrooted_reference_views() {
        assert!(
            rejection(
                "mod options { #[rusnix(root)] #[derive(Default)] struct Root { value: i64 } }"
            )
            .contains("reference declarations")
        );
    }
}
