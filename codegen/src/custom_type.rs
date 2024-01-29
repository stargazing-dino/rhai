use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{
    punctuated::Punctuated, spanned::Spanned, DeriveInput, Expr, Fields, MetaNameValue, Path, Token,
};

const ATTR_ROOT: &str = "rhai_type";
const ATTR_NAME: &str = "name";
const ATTR_SKIP: &str = "skip";
const ATTR_GET: &str = "get";
const ATTR_GET_MUT: &str = "get_mut";
const ATTR_SET: &str = "set";
const ATTR_READONLY: &str = "readonly";
const ATTR_EXTRA: &str = "extra";

/// Derive the `CustomType` trait for a struct.
pub fn derive_custom_type_impl(input: DeriveInput) -> TokenStream {
    let type_name = input.ident;
    let mut pretty_print_name = quote! { stringify!(#type_name) };
    let mut extras = Vec::new();
    let mut errors = Vec::new();

    for attr in input.attrs.iter().filter(|a| a.path().is_ident(ATTR_ROOT)) {
        let config_list: Result<Punctuated<Expr, Token![,]>, _> =
            attr.parse_args_with(Punctuated::parse_terminated);

        match config_list {
            Ok(list) => {
                for expr in list {
                    match expr {
                        // Key-value
                        syn::Expr::Assign(..) => {
                            let MetaNameValue { path, value, .. } =
                                syn::parse2::<MetaNameValue>(expr.to_token_stream()).unwrap();

                            if path.is_ident(ATTR_NAME) {
                                // Type name
                                pretty_print_name = value.to_token_stream();
                            } else if path.is_ident(ATTR_EXTRA) {
                                match syn::parse2::<Path>(value.to_token_stream()) {
                                    Ok(path) => extras.push(path.to_token_stream()),
                                    Err(err) => errors.push(err.into_compile_error()),
                                }
                            } else {
                                let key = path.get_ident().unwrap().to_string();
                                let msg = format!("invalid option: '{}'", key);
                                errors.push(syn::Error::new(path.span(), msg).into_compile_error());
                            }
                        }
                        // skip
                        syn::Expr::Path(path) if path.path.is_ident(ATTR_SKIP) => {
                            println!("SKIPPED");
                        }
                        // any other identifier
                        syn::Expr::Path(path) if path.path.get_ident().is_some() => {
                            let key = path.path.get_ident().unwrap().to_string();
                            let msg = format!("invalid option: '{}'", key);
                            errors.push(syn::Error::new(path.span(), msg).into_compile_error());
                        }
                        // Error
                        _ => errors.push(
                            syn::Error::new(expr.span(), "expecting identifier")
                                .into_compile_error(),
                        ),
                    }
                }
            }
            Err(err) => errors.push(err.into_compile_error()),
        }
    }

    let accessors = match input.data {
        // struct Foo;
        syn::Data::Struct(syn::DataStruct {
            fields: Fields::Unit,
            ..
        }) => quote! {},

        // struct Foo { ... }
        syn::Data::Struct(syn::DataStruct { fields, .. }) => {
            let fields = match fields {
                Fields::Named(ref f) => f.named.iter(),
                Fields::Unnamed(ref f) => f.unnamed.iter(),
                Fields::Unit => unreachable!(),
            };

            let iter = fields.enumerate().map(|(i, field)| {
                let mut name = None;
                let mut get_fn = None;
                let mut get_mut_fn = None;
                let mut set_fn = None;
                let mut readonly = false;
                let mut skip = false;

                for attr in field.attrs.iter().filter(|a| a.path().is_ident(ATTR_ROOT)) {
                    let config_list: Result<Punctuated<Expr, Token![,]>, _> =
                        attr.parse_args_with(Punctuated::parse_terminated);

                    let list = match config_list {
                        Ok(list) => list,
                        Err(err) => {
                            errors.push(err.into_compile_error());
                            continue;
                        }
                    };

                    for expr in list {
                        let ident = match expr {
                            // skip
                            syn::Expr::Path(path) if path.path.is_ident(ATTR_SKIP) => {
                                skip = true;

                                // `skip` cannot be used with any other attributes.
                                if get_fn.is_some()
                                    || get_mut_fn.is_some()
                                    || set_fn.is_some()
                                    || name.is_some()
                                    || readonly
                                {
                                    let msg =
                                        format!("cannot use '{ATTR_SKIP}' with other attributes");
                                    errors.push(
                                        syn::Error::new(path.span(), msg).into_compile_error(),
                                    );
                                }

                                continue;
                            }
                            // readonly
                            syn::Expr::Path(path) if path.path.is_ident(ATTR_READONLY) => {
                                readonly = true;

                                if set_fn.is_some() {
                                    let msg =
                                        format!("cannot use '{ATTR_READONLY}' with '{ATTR_SET}'");
                                    errors.push(
                                        syn::Error::new(path.path.span(), msg).into_compile_error(),
                                    );
                                }

                                path.path.get_ident().unwrap().clone()
                            }
                            // Key-value
                            syn::Expr::Assign(..) => {
                                let MetaNameValue { path, value, .. } =
                                    syn::parse2::<MetaNameValue>(expr.to_token_stream()).unwrap();

                                if path.is_ident(ATTR_NAME) {
                                    // Type name
                                    name = Some(value.to_token_stream());
                                } else if path.is_ident(ATTR_GET) {
                                    match syn::parse2::<Path>(value.to_token_stream()) {
                                        Ok(path) => get_fn = Some(path.to_token_stream()),
                                        Err(err) => errors.push(err.into_compile_error()),
                                    }
                                } else if path.is_ident(ATTR_GET_MUT) {
                                    match syn::parse2::<Path>(value.to_token_stream()) {
                                        Ok(path) => get_mut_fn = Some(path.to_token_stream()),
                                        Err(err) => errors.push(err.into_compile_error()),
                                    }
                                } else if path.is_ident(ATTR_SET) {
                                    match syn::parse2::<Path>(value.to_token_stream()) {
                                        Ok(path) => set_fn = Some(path.to_token_stream()),
                                        Err(err) => errors.push(err.into_compile_error()),
                                    }
                                } else if path.is_ident(ATTR_SKIP) || path.is_ident(ATTR_READONLY) {
                                    let key = path.get_ident().unwrap().to_string();
                                    let msg = format!("'{key}' cannot have value");
                                    errors.push(
                                        syn::Error::new(path.span(), msg).into_compile_error(),
                                    );
                                    continue;
                                } else {
                                    let key = path.get_ident().unwrap().to_string();
                                    let msg = format!("invalid option: '{key}'");
                                    errors.push(
                                        syn::Error::new(path.span(), msg).into_compile_error(),
                                    );
                                    continue;
                                }

                                path.get_ident().unwrap().clone()
                            }
                            // any other identifier
                            syn::Expr::Path(path) if path.path.get_ident().is_some() => {
                                let key = path.path.get_ident().unwrap().to_string();
                                let msg = format!("invalid option: '{key}'");
                                errors.push(syn::Error::new(path.span(), msg).into_compile_error());
                                continue;
                            }
                            // Error
                            _ => {
                                errors.push(
                                    syn::Error::new(expr.span(), "expecting identifier")
                                        .into_compile_error(),
                                );
                                continue;
                            }
                        };

                        if skip {
                            let msg = format!("cannot use '{ident}' with '{ATTR_SKIP}'");
                            errors.push(
                                syn::Error::new(attr.path().span(), msg).into_compile_error(),
                            );
                        }
                    }
                }

                if !skip {
                    let field_name = if let Some(ref field_name) = field.ident {
                        quote! { #field_name }
                    } else {
                        if name.is_none() {
                            let map_name = format!("field{i}");
                            name = Some(quote! { #map_name });
                        }
                        let index = proc_macro2::Literal::usize_unsuffixed(i);
                        quote! { #index }
                    };

                    generate_accessor_fns(field_name, name, get_fn, get_mut_fn, set_fn, readonly)
                } else {
                    quote! {}
                }
            });

            quote! { #(#iter;)* }
        }

        syn::Data::Enum(_) => {
            return syn::Error::new(Span::call_site(), "enums are not yet implemented")
                .into_compile_error()
        }
        syn::Data::Union(_) => {
            return syn::Error::new(Span::call_site(), "unions are not yet supported")
                .into_compile_error()
        }
    };

    quote! {
        impl CustomType for #type_name {
            fn build(mut builder: TypeBuilder<Self>) {
                #(#errors;)*
                builder.with_name(#pretty_print_name);
                #accessors;
                #(#extras(&mut builder);)*
            }
        }
    }
}

/// Generate a `TypeBuilder` accessor function.
fn generate_accessor_fns(
    field: TokenStream,
    name: Option<TokenStream>,
    get: Option<TokenStream>,
    get_mut: Option<TokenStream>,
    set: Option<TokenStream>,
    readonly: bool,
) -> proc_macro2::TokenStream {
    let get = match (get_mut, get) {
        (Some(func), _) => func,
        (None, Some(func)) => quote! { |obj: &mut Self| #func(&*obj) },
        (None, None) => quote! { |obj: &mut Self| obj.#field.clone() },
    };

    let set = set.unwrap_or_else(|| quote! { |obj: &mut Self, val| obj.#field = val });
    let name = name.unwrap_or_else(|| quote! { stringify!(#field) });

    if readonly {
        quote! { builder.with_get(#name, #get) }
    } else {
        quote! { builder.with_get_set(#name, #get, #set) }
    }
}
