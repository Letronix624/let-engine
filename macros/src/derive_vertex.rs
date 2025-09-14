use crate::bail;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    Data, DataStruct, Fields, Ident, LitStr, Result, Token,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

pub fn derive_vertex(crate_ident: &Ident, ast: syn::DeriveInput) -> Result<TokenStream> {
    let struct_name = &ast.ident;

    let fields = match &ast.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => bail!("expected a struct with named fields"),
    };

    let mut members = quote! {
        let mut offset = 0;
        let mut members = ::std::collections::HashMap::default();
    };

    for field in fields.iter() {
        let field_name = field.ident.to_owned().unwrap();
        let field_name_lit = LitStr::new(&field_name.to_string(), Span::call_site());
        let field_ty = &field.ty;
        let mut names = vec![field_name_lit.clone()];
        let mut format = quote! {};
        for attr in &field.attrs {
            let attr_ident = if let Some(ident) = attr.path().get_ident() {
                ident
            } else {
                continue;
            };
            if attr_ident == "name" {
                let meta = attr.parse_args_with(NameMeta::parse)?;
                names = meta.lit_str_list.into_iter().collect();
            } else if attr_ident == "format" {
                let format_ident = attr.parse_args_with(Ident::parse)?;
                format = quote! {
                    ::#crate_ident::resources::Format::#format_ident;
                };
            }
        }
        if format.is_empty() {
            bail!(
                field_name,
                "expected `#[format(...)]`-attribute with valid `let_engine_core::resources::Format`",
            );
        }
        for name in &names {
            members = quote! {
                #members

                {
                    let field_align = ::std::mem::align_of::<#field_ty>();
                    offset = (offset + field_align - 1) & !(field_align - 1);

                    let field_size = ::std::mem::size_of::<#field_ty>();
                    let format = #format;
                    let format_size = usize::try_from(format.block_size()).unwrap();
                    let num_elements = field_size / format_size;
                    let remainder = field_size % format_size;
                    ::std::assert!(
                        remainder == 0,
                        "struct field `{}` size does not fit multiple of format size",
                        #field_name_lit,
                    );

                    members.insert(
                        #name.to_string(),
                        ::#crate_ident::resources::model::VertexMemberInfo {
                            offset: offset.try_into().unwrap(),
                            format,
                            num_elements: num_elements.try_into().unwrap(),
                            stride: format_size.try_into().unwrap(),
                        },
                    );

                    offset += field_size;
                }
            };
        }
    }

    let function_body = quote! {
        #members

        ::#crate_ident::resources::model::VertexBufferDescription {
            members,
            stride: ::std::mem::size_of::<#struct_name>() as u32,
        }
    };

    Ok(quote! {
        #[allow(unsafe_code)]
        unsafe impl ::#crate_ident::resources::model::Vertex for #struct_name {
            #[inline(always)]
            fn description() -> ::#crate_ident::resources::model::VertexBufferDescription {
                #function_body
            }

        }
    })
}

struct NameMeta {
    lit_str_list: Punctuated<LitStr, Token![,]>,
}

impl Parse for NameMeta {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            lit_str_list: input.parse_terminated(<LitStr as Parse>::parse, Token![,])?,
        })
    }
}
