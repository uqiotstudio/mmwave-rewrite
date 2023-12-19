extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

#[proc_macro_derive(FromBytes)]
pub fn from_bytes_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match input.data {
        Data::Struct(ref data_struct) => {
            let struct_name = input.ident;
            match data_struct.fields {
                Fields::Named(ref fields) => {
                    let mut field_init = Vec::new();
                    let index_quote = quote! { let mut index: usize = 0; };

                    let mut iterator = fields.named.iter().peekable();
                    while let Some(f) = iterator.next() {
                        let name = &f.ident;
                        let ty = &f.ty;
                        let width = if iterator.peek().is_none() {
                            // The next element is none so use ALL bytes
                            quote! { index.. }
                        } else {
                            quote! { index..index+<#ty>::size_of() }
                        };
                        let field_quote = quote! {
                            #name: {
                                let parsed = <#ty as FromBytes>::from_bytes(&bytes[#width]);
                                index += <#ty>::size_of();
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
