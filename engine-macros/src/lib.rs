extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse::Parser, parse_macro_input, DeriveInput};

/// Implements GameObject on a struct and automaically adds the fields transform, appearance, id
/// and layer to update from and to.
/// Also adds 2 functions. Update and Sync.
/// Update updates the object from the layer system and sync syncs the object to the layer.
/// Those functions panic when the object isn't initialized to the layer yet.
#[proc_macro_attribute]
pub fn object(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut ast = parse_macro_input!(input as DeriveInput);
    let name = &ast.ident;
    let implements = match &mut ast.data {
        syn::Data::Struct(ref mut struct_data) => {
            if let syn::Fields::Named(fields) = &mut struct_data.fields {
                fields.named.push(
                    syn::Field::parse_named
                        .parse2(quote! {
                            pub transform: let_engine::Transform
                        })
                        .expect("transform failed"),
                );
                fields.named.push(
                    syn::Field::parse_named
                        .parse2(quote! {
                            pub appearance: let_engine::Appearance
                        })
                        .expect("appearance failed"),
                );
                fields.named.push(
                    syn::Field::parse_named
                        .parse2(quote! {
                            id: usize
                        })
                        .expect("id failed"),
                );
                fields.named.push(
                    syn::Field::parse_named
                        .parse2(quote! {
                            layer: Option<let_engine::Layer>
                        })
                        .expect("id failed"),
                );
            }
            quote! {
                impl let_engine::GameObject for #name {
                    fn transform(&self) -> Transform {
                        self.transform
                    }
                    fn appearance(&self) -> &Appearance {
                        &self.appearance
                    }
                    fn id(&self) -> usize {
                        self.id
                    }
                    fn init(&mut self, id: usize, layer: &let_engine::Layer) {
                        self.id = id;
                        self.layer = Some(layer.clone());
                    }
                }
                impl #name {
                    pub fn update(&mut self) {
                        let object = self.layer.as_ref().unwrap().fetch(self.id());
                        self.transform = object.transform;
                        self.appearance = object.appearance;
                    }
                    pub fn sync(&self) {
                        self.layer.as_ref().unwrap().update(self)
                    }
                }
            }
        }
        _ => panic!("`object` has to be used with structs."),
    };

    quote! {
        #[derive(Clone)]
        #ast
        #implements
    }
    .into()
}

/// Implements GameObject and Camera to an object. Marks an object to be able to be used as a
/// camera and automatically adds a camera field which holds the mode and zoom.
#[proc_macro_attribute]
pub fn camera_object(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut ast = parse_macro_input!(input as DeriveInput);
    let name = &ast.ident;

    let implements = if let syn::Data::Struct(ref mut struct_data) = ast.data {
        if let syn::Fields::Named(fields) = &mut struct_data.fields {
            fields.named.push(
                syn::Field::parse_named
                    .parse2(quote! {
                        pub camera: let_engine::CameraSettings
                    })
                    .unwrap(),
            );
        }
        quote! {
            impl let_engine::Camera for #name {
                fn settings(&self) -> let_engine::CameraSettings {
                    self.camera
                }
            }
            impl let_engine::CameraObject for #name {}
        }
    } else {
        panic!("`object` has to be used with structs.");
    };

    quote! {
        #[object]
        #ast
        #implements
    }
    .into()
}
