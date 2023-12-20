extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::{
    parse_macro_input, Data, DataStruct, DeriveInput, Field, Fields, FieldsNamed, Lit, Meta,
    NestedMeta,
};

#[proc_macro_derive(FromBytes, attributes(Header, Body))]
pub fn from_bytes_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match input.data {
        Data::Struct(ref data_struct) => process_struct(data_struct, &input.ident),
        _ => unimplemented!(), // Handle other cases like enums or unions
    }
}

fn process_struct(data_struct: &DataStruct, struct_name: &Ident) -> TokenStream {
    match data_struct.fields {
        Fields::Named(ref fields) => process_named_fields(fields, struct_name),
        _ => unimplemented!(), // Handle other cases like unnamed fields
    }
}

fn process_named_fields(fields: &FieldsNamed, struct_name: &Ident) -> TokenStream {
    let mut iter = fields.named.iter().peekable();
    let mut field_inits = Vec::new();
    while let Some(f) = iter.next() {
        field_inits.push(process_field(f, iter.peek().is_none()));
    }
    let expanded = quote! {
        impl FromBytes for #struct_name {
            fn from_bytes(bytes: &[u8]) -> Result<Self, ParseError> {
                let mut index: usize = 0;
                let mut headers = std::collections::HashMap::<usize, usize>::new();
                Ok(#struct_name {
                    #( #field_inits, )*
                })
            }
        }
    };
    TokenStream::from(expanded)
}

fn process_field(field: &Field, is_last: bool) -> proc_macro2::TokenStream {
    let name = &field.ident;
    let ty = &field.ty;

    let (mut width, mut size) = if is_last {
        (quote! { (index..) }, quote! { parsed.size_of_val() })
    } else {
        (
            quote! { (index..index+<#ty>::size_of()) },
            quote! { <#ty>::size_of() },
        )
    };
    let mut header_quote = quote! {};

    // Attributes may omdify header_quote and width_quote
    for attr in &field.attrs {
        let Ok(Meta::List(meta_list)) = attr.parse_meta() else {
            continue;
        };
        for nested_meta in meta_list.nested {
            let NestedMeta::Lit(Lit::Int(lit_int)) = nested_meta else {
                continue;
            };
            let value: usize = lit_int.base10_parse::<usize>().unwrap();
            if attr.path.is_ident("Header") {
                header_quote = quote! {
                    headers.insert(#value, parsed.get_body_length());
                };
            } else if attr.path.is_ident("Body") {
                width = quote! {
                    (index..(index+headers.get(&#value).unwrap()))
                }
            }
        }
    }

    quote! {
        #name: {
            let parsed = <#ty as FromBytes>::from_bytes(&bytes[#width])?;
            #header_quote
            index += #size;
            parsed
        }
    }
}
