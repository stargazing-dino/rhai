use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::DeriveInput;

pub fn derive_custom_type_impl(input: DeriveInput) -> TokenStream {
    let name = input.ident;

    let accessors = match input.data {
        syn::Data::Struct(syn::DataStruct {
            fields: syn::Fields::Named(ref fields),
            ..
        }) => {
            let iter = fields.named.iter().map(|field| {
                let mut name = None;
                let mut get_fn = None;
                let mut set_fn = None;
                let mut readonly = false;
                let mut skip = false;

                for attr in field.attrs.iter() {
                    if attr.path().is_ident("rhai_custom_type_skip") {
                        if get_fn.is_some() || set_fn.is_some() || name.is_some() {
                            return syn::Error::new(
                                Span::call_site(),
                                "cannot use 'rhai_custom_type_skip' with other attributes",
                            )
                            .into_compile_error();
                        }

                        skip = true;
                        continue;
                    }

                    if skip {
                        return syn::Error::new(
                            Span::call_site(),
                            "cannot use 'rhai_custom_type_skip' with other attributes",
                        )
                        .into_compile_error();
                    }

                    if attr.path().is_ident("rhai_custom_type_name") {
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

                if !skip {
                    generate_accessor_fns(
                        &field.ident.as_ref().unwrap(),
                        name,
                        get_fn,
                        set_fn,
                        readonly,
                    )
                } else {
                    quote! {}
                }
            });

            quote! {#(#iter)*}
        }

        syn::Data::Struct(syn::DataStruct {
            fields: syn::Fields::Unnamed(_),
            ..
        }) => syn::Error::new(Span::call_site(), "tuple structs are not yet implemented")
            .into_compile_error(),

        syn::Data::Struct(syn::DataStruct {
            fields: syn::Fields::Unit,
            ..
        }) => quote! {},

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
    field: &Ident,
    name: Option<TokenStream>,
    get: Option<TokenStream>,
    set: Option<TokenStream>,
    readonly: bool,
) -> proc_macro2::TokenStream {
    let get = get
        .map(|func| quote! {#func})
        .unwrap_or_else(|| quote! {|obj: &mut Self| obj.#field.clone()});

    let set = set
        .map(|func| quote! {#func})
        .unwrap_or_else(|| quote! {|obj: &mut Self, val| obj.#field = val});

    let name = name
        .map(|field| quote! { #field })
        .unwrap_or_else(|| quote! { stringify!(#field) });

    if readonly {
        quote! {
            builder.with_get(#name, #get);
        }
    } else {
        quote! {
            builder.with_get_set(#name, #get, #set);
        }
    }
}
