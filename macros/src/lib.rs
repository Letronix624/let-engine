use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use syn::{DeriveInput, Error, parse_macro_input};

mod derive_vertex;

/// Derives the `Vertex` trait.
#[proc_macro_derive(Vertex, attributes(name, format))]
pub fn derive_vertex(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let crate_ident = crate_ident();

    derive_vertex::derive_vertex(&crate_ident, ast)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

fn crate_ident() -> syn::Ident {
    let found_crate = crate_name("let-engine-core").unwrap();
    let name = match &found_crate {
        FoundCrate::Itself => "let_engine_core",
        FoundCrate::Name(name) => name,
    };

    syn::Ident::new(name, proc_macro2::Span::call_site())
}

macro_rules! bail {
    ($msg:expr $(,)?) => {
        return Err(syn::Error::new(proc_macro2::Span::call_site(), $msg))
    };
    ($span:expr, $msg:expr $(,)?) => {
        return Err(syn::Error::new_spanned($span, $msg))
    };
}
use bail;
