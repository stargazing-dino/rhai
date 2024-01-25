use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::DeriveInput;

pub fn derive_custom_type_impl(input: DeriveInput) -> TokenStream {
    let name = input.ident;

    let accessors = match input.data {
        syn::Data::Struct(ref data) => match data.fields {
            syn::Fields::Named(ref fields) => {
                let iter = fields.named.iter().map(|field| {
                    let mut get_fn = None;
                    let mut set_fn = None;
                    let mut readonly = false;
                    for attr in field.attrs.iter() {
                        if attr.path().is_ident("get") {
                            get_fn = Some(
                                attr.parse_args()
                                    .unwrap_or_else(syn::Error::into_compile_error),
                            );
                        } else if attr.path().is_ident("set") {
                            set_fn = Some(
                                attr.parse_args()
                                    .unwrap_or_else(syn::Error::into_compile_error),
                            );
                        } else if attr.path().is_ident("readonly") {
                            readonly = true;
                        }
                    }

                    generate_accessor_fns(&field.ident.as_ref().unwrap(), get_fn, set_fn, readonly)
                });
                quote! {#(#iter)*}
            }
            syn::Fields::Unnamed(_) => {
                syn::Error::new(Span::call_site(), "tuple structs are not yet implemented")
                    .into_compile_error()
            }
            syn::Fields::Unit => quote! {},
        },
        syn::Data::Enum(_) => {
            syn::Error::new(Span::call_site(), "enums are not yet implemented").into_compile_error()
        }
        syn::Data::Union(_) => {
            syn::Error::new(Span::call_site(), "unions are not supported").into_compile_error()
        }
    };

    quote! {
        impl ::rhai::CustomType for #name {
            fn build(mut builder: ::rhai::TypeBuilder<Self>) {
                #accessors;
            }
        }
    }
}

fn generate_accessor_fns(
    field: &Ident,
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

    if readonly {
        quote! {
            builder.with_get("#field", #get);
        }
    } else {
        quote! {
            builder.with_get_set(
                "#field",
                #get,
                #set,
            );
        }
    }
}
