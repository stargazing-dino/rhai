use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{spanned::Spanned, DeriveInput, Fields};

const ATTR_NAME: &str = "rhai_type_name";
const ATTR_SKIP: &str = "rhai_type_skip";
const ATTR_GET: &str = "rhai_type_get";
const ATTR_GET_MUT: &str = "rhai_type_get_mut";
const ATTR_SET: &str = "rhai_type_set";
const ATTR_READONLY: &str = "rhai_type_readonly";
const ATTR_EXTRA: &str = "rhai_type_extra";

pub fn derive_custom_type_impl(input: DeriveInput) -> TokenStream {
    let type_name = input.ident;
    let mut pretty_print_name = quote! { stringify!(#type_name) };
    let mut extras = Vec::new();

    for attr in input.attrs.iter() {
        if attr.path().is_ident(ATTR_NAME) {
            // Type name
            match attr.parse_args::<TokenStream>() {
                Ok(name) => pretty_print_name = quote! { #name },
                Err(e) => return e.into_compile_error(),
            }
        } else if attr.path().is_ident(ATTR_EXTRA) {
            extras.push(
                attr.parse_args()
                    .unwrap_or_else(syn::Error::into_compile_error),
            );
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

                for attr in field.attrs.iter() {
                    if attr.path().is_ident(ATTR_SKIP) {
                        skip = true;

                        if get_fn.is_some()
                            || get_mut_fn.is_some()
                            || set_fn.is_some()
                            || name.is_some()
                            || readonly
                        {
                            return syn::Error::new(
                                attr.path().span(),
                                format!("cannot use '{ATTR_SKIP}' with other attributes"),
                            )
                            .into_compile_error();
                        }

                        continue;
                    }

                    if attr.path().is_ident(ATTR_NAME) {
                        name = Some(
                            attr.parse_args()
                                .unwrap_or_else(syn::Error::into_compile_error),
                        );
                    } else if attr.path().is_ident(ATTR_GET) {
                        get_fn = Some(
                            attr.parse_args()
                                .unwrap_or_else(syn::Error::into_compile_error),
                        );
                    } else if attr.path().is_ident(ATTR_GET_MUT) {
                        get_mut_fn = Some(
                            attr.parse_args()
                                .unwrap_or_else(syn::Error::into_compile_error),
                        );
                    } else if attr.path().is_ident(ATTR_SET) {
                        if readonly {
                            return syn::Error::new(
                                attr.path().span(),
                                format!("cannot use '{ATTR_SET}' with '{ATTR_READONLY}'"),
                            )
                            .into_compile_error();
                        }
                        set_fn = Some(
                            attr.parse_args()
                                .unwrap_or_else(syn::Error::into_compile_error),
                        );
                    } else if attr.path().is_ident(ATTR_READONLY) {
                        if set_fn.is_some() {
                            return syn::Error::new(
                                attr.path().span(),
                                format!("cannot use '{ATTR_READONLY}' with '{ATTR_SET}'"),
                            )
                            .into_compile_error();
                        }
                        readonly = true;
                    }

                    if skip {
                        let attr_name = attr.path().get_ident().unwrap();
                        return syn::Error::new(
                            attr.path().span(),
                            format!("cannot use '{}' with '{ATTR_SKIP}'", attr_name),
                        )
                        .into_compile_error();
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

            quote! { #(#iter ;)* }
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
                builder.with_name(#pretty_print_name);
                #accessors;
                #(#extras(&mut builder);)*
            }
        }
    }
}

fn generate_accessor_fns(
    field: TokenStream,
    name: Option<TokenStream>,
    get: Option<TokenStream>,
    get_mut: Option<TokenStream>,
    set: Option<TokenStream>,
    readonly: bool,
) -> proc_macro2::TokenStream {
    let get = match (get_mut, get) {
        (Some(func), _) => quote! { #func },
        (None, Some(func)) => quote! { |obj: &mut Self| #func(&*obj) },
        (None, None) => quote! { |obj: &mut Self| obj.#field.clone() },
    };

    let set = set.map_or_else(
        || quote! { |obj: &mut Self, val| obj.#field = val },
        |func| quote! { #func },
    );
    let name = name.map_or_else(|| quote! { stringify!(#field) }, |expr| quote! { #expr });

    if readonly {
        quote! { builder.with_get(#name, #get) }
    } else {
        quote! { builder.with_get_set(#name, #get, #set) }
    }
}
