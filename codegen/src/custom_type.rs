use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{DeriveInput, Fields};

pub fn derive_custom_type_impl(input: DeriveInput) -> TokenStream {
    let name = input.ident;

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
                let mut set_fn = None;
                let mut readonly = false;
                let mut skip = false;

                for attr in field.attrs.iter() {
                    if attr.path().is_ident("rhai_custom_type_skip") {
                        skip = true;
                    } else if attr.path().is_ident("rhai_custom_type_name") {
                        name = Some(
                            attr.parse_args()
                                .unwrap_or_else(syn::Error::into_compile_error),
                        );
                    } else if attr.path().is_ident("rhai_custom_type_get") {
                        get_fn = Some(
                            attr.parse_args()
                                .unwrap_or_else(syn::Error::into_compile_error),
                        );
                    } else if attr.path().is_ident("rhai_custom_type_set") {
                        if readonly {
                            return syn::Error::new(
                                Span::call_site(),
                                "cannot use 'rhai_custom_type_set' with 'rhai_custom_type_readonly'",
                            )
                            .into_compile_error();
                        }
                        set_fn = Some(
                            attr.parse_args()
                                .unwrap_or_else(syn::Error::into_compile_error),
                        );
                    } else if attr.path().is_ident("rhai_custom_type_readonly") {
                        if set_fn.is_some() {
                            return syn::Error::new(
                                Span::call_site(),
                                "cannot use 'rhai_custom_type_readonly' with 'rhai_custom_type_set'",
                            )
                            .into_compile_error();
                        }
                        readonly = true;
                    }
                }

                if skip && (get_fn.is_some() || set_fn.is_some() || name.is_some() || readonly) {
                    return syn::Error::new(
                        Span::call_site(),
                        "cannot use 'rhai_custom_type_skip' with other attributes",
                    )
                    .into_compile_error();
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

                    generate_accessor_fns(field_name, name, get_fn, set_fn, readonly)
                } else {
                    quote! {}
                }
            });

            quote! { #(#iter ;)* }
        }

        syn::Data::Enum(_) => {
            syn::Error::new(Span::call_site(), "enums are not yet implemented").into_compile_error()
        }
        syn::Data::Union(_) => {
            syn::Error::new(Span::call_site(), "unions are not supported").into_compile_error()
        }
    };

    quote! {
        impl CustomType for #name {
            fn build(mut builder: TypeBuilder<Self>) {
                #accessors;
            }
        }
    }
}

fn generate_accessor_fns(
    field: TokenStream,
    name: Option<TokenStream>,
    get: Option<TokenStream>,
    set: Option<TokenStream>,
    readonly: bool,
) -> proc_macro2::TokenStream {
    let get = get.map_or_else(
        || quote! { |obj: &mut Self| obj.#field.clone() },
        |func| quote! { #func },
    );
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
