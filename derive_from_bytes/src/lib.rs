extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::{
    parse_macro_input, Data, DataStruct, DeriveInput, Field, Fields, FieldsNamed, Lit, Meta,
    NestedMeta,
};

#[proc_macro_derive(FromBytes)]
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

    quote! {
        #name: {
            let parsed = <#ty as FromBytes>::from_bytes(&bytes[#width])?;
            index += #size;
            parsed
        }
    }
}
