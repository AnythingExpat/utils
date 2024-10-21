extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, punctuated::Punctuated, Data, DeriveInput, Expr, Field, Fields, Ident, Lit, Meta, Token, Type};

struct EnvField {
    ident: Ident,
    ty: Type,
    var_or_file: bool,
    name: Option<String>
}

fn handle_field(field: &Field) -> EnvField {
    let mut var_or_file = false;
    let mut name: Option<String> = None;

    for attr in &field.attrs {
        let path = attr.path();
        if !path.is_ident("utils") {
            continue;
        }

        let args = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated).expect("Error parsing arguments to 'utils' attribute");
        for arg in args {
            match arg {
                Meta::Path(path) => {
                    if path.is_ident("var_or_file") {
                        var_or_file = true;
                        continue;
                    }
                },
                Meta::NameValue(name_value) => {
                    if name_value.path.is_ident("name") {
                        if let Expr::Lit(lit) = name_value.value {
                            if let Lit::Str(value) = lit.lit {
                                name = Some(value.value());
                                continue;
                            }
                        }
                    }
                },
                _ => {}
            }

            panic!("Encountered unknown or invalid arguments in 'utils' attribute");
        }
    }

    EnvField {
        ident: field.ident.clone().unwrap(),
        ty: field.ty.clone(),
        var_or_file,
        name
    }
}

#[proc_macro_derive(FromEnv, attributes(utils))]
pub fn derive_env_config(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    let s = ast.ident;

    let data = match ast.data {
        Data::Struct(ref data) => data,
        _ => panic!("FromEnv can only be derived for structs")
    };

    let named_fields = match data.fields {
        Fields::Named(ref named_fields) => named_fields,
        _ => panic!("Fields must be named")
    };

    let fields: Vec<proc_macro2::TokenStream> = named_fields.named.iter()
    .map(|field| handle_field(field))
    .map(|field| {
            let EnvField { ident, ty, .. } = field;

            let name_quote = if let Some(name) = field.name {
                quote! {
                    #name
                }
            } else {
                quote! {
                    &utils::__join_idents(ident, stringify!(#ident))
                }
            };

            if field.var_or_file {
                quote! {
                    #ident: <#ty as FromEnv>::load_or_file(#name_quote)?
                }
            } else {
                quote! {
                    #ident: <#ty as FromEnv>::load(#name_quote)?
                }
            }
        })
        .collect();
    
    quote! {
        impl utils::FromEnv for #s {
            fn from_env(value: &str) -> Result<Self, utils::EnvErrorType> {
                Err(utils::EnvErrorType::Other(String::from("'from' method not implemented for derive(FromEnv)")))
            }

            fn load(ident: &str) -> Result<Self, utils::EnvError> {
                Ok(#s {
                    #(#fields),*
                })
            }
        }
    }.into()
}