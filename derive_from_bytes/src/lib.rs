extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Lit, Meta, NestedMeta};

#[proc_macro_derive(FromBytes, attributes(Header, Body))]
pub fn from_bytes_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match input.data {
        Data::Struct(ref data_struct) => {
            let struct_name = input.ident;
            match data_struct.fields {
                Fields::Named(ref fields) => {
                    let mut field_init = Vec::new();
                    let index_quote = quote! {
                        let mut index: usize = 0;
                    };
                    let mut iterator = fields.named.iter().peekable();
                    let headers_quote = quote! {
                        let mut headers = std::collections::HashMap::<usize, usize>::new();
                    };

                    // Build up the entries
                    while let Some(f) = iterator.next() {
                        let name = &f.ident;
                        let ty = &f.ty;
                        let mut width = if iterator.peek().is_none() {
                            // The next element is none so use ALL bytes
                            quote! { index.. }
                        } else {
                            quote! { index..index+<#ty>::size_of() }
                        };

                        let mut header_quote = quote! {};

                        for attr in &f.attrs {
                            if let Ok(Meta::List(meta_list)) = attr.parse_meta() {
                                for nested_meta in meta_list.nested {
                                    if let NestedMeta::Lit(Lit::Int(lit_int)) = nested_meta {
                                        let value = lit_int.base10_parse::<u32>().unwrap();
                                        if attr.path.is_ident("Header") {
                                            header_quote = quote! {
                                                headers.insert(value, parsed.get_body_length());
                                            };
                                        } else if attr.path.is_ident("Body") {
                                            width = quote! {
                                                index..(index+headers.get(#value).unwrap().get_body_length())
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        let field_quote = quote! {
                            #name: {
                                let parsed = <#ty as FromBytes>::from_bytes(&bytes[#width]);
                                #header_quote
                                index += (#width.end - 1) - #width.start;
                                parsed?
                            }
                        };
                        field_init.push(field_quote);
                    }

                    let expanded = quote! {
                        impl FromBytes for #struct_name {
                            fn from_bytes(bytes: &[u8]) -> Result<Self, ParseError> {
                                {
                                    #index_quote
                                    #headers_quote
                                    Ok(#struct_name {
                                        #( #field_init, )*
                                    })
                                }
                            }
                        }
                    };

                    TokenStream::from(expanded)
                }
                _ => unimplemented!(), // Handle other cases like unnamed fields
            }
        }
        _ => unimplemented!(), // Handle other cases like enums or unions
    }
}
