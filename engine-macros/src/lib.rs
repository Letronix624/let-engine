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
    let ast = parse_macro_input!(input as DeriveInput);
    let name = &ast.ident;
    quote! {
        #[let_engine::objectinit]
        #ast
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
            fn init_to_layer(&mut self, id: usize, weak: let_engine::WeakObject) {
                self.id = id;
                self.reference = Some(weak);
            }
        }
    }
    .into()
}

#[proc_macro_attribute]
pub fn objectinit(_args: TokenStream, input: TokenStream) -> TokenStream {
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
                            reference: Option<let_engine::WeakObject>
                        })
                        .expect("weak object failed"),
                );
            }
            quote! {
                impl #name {
                    pub fn update(&mut self) {
                        let arc = self.reference.clone().unwrap().upgrade().unwrap();
                        let object = &arc.lock().object;
                        self.transform = object.transform();
                        self.appearance = object.appearance().clone();
                    }
                    pub fn sync(&self) {
                        let arc = self.reference.clone().unwrap().upgrade().unwrap();
                        let mut object = arc.lock();
                        object.object = Box::new(self.clone());
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

#[proc_macro_attribute]
pub fn objectinit_without_implements(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut ast = parse_macro_input!(input as DeriveInput);
    match &mut ast.data {
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
                            reference: Option<let_engine::WeakObject>
                        })
                        .expect("weak object failed"),
                );
            }
        }
        _ => panic!("`object` has to be used with structs."),
    };

    quote! {
        #[derive(Clone)]
        #ast
    }
    .into()
}

/// Implements GameObject and Camera to an object. Marks an object to be able to be used as a
/// camera and automatically adds a camera field which holds the mode and zoom.
#[proc_macro_attribute]
pub fn camera(_args: TokenStream, input: TokenStream) -> TokenStream {
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
                fn init_to_layer(&mut self, id: usize, weak: let_engine::WeakObject) {
                    self.reference = Some(weak);
                    self.id = id;
                }
            }
            impl let_engine::Camera for #name {
                fn settings(&self) -> let_engine::CameraSettings {
                    self.camera
                }
            }
        }
    } else {
        panic!("`object` has to be used with structs.");
    };

    quote! {
        #[let_engine::objectinit]
        #ast
        #implements
    }
    .into()
}

/// Implements colliders onto an object for buttons or sensor areas
#[proc_macro_attribute]
pub fn collider(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut ast = parse_macro_input!(input as DeriveInput);
    let name = &ast.ident;

    match &mut ast.data {
        syn::Data::Struct(ref mut struct_data) => {
            if let syn::Fields::Named(fields) = &mut struct_data.fields {
                fields.named.push(
                    syn::Field::parse_named
                        .parse2(quote! {
                            pub collider: Option<let_engine::physics::Collider>
                        })
                        .expect("collider failed"),
                );
                fields.named.push(
                    syn::Field::parse_named
                        .parse2(quote! {
                            collider_handle: Option<let_engine::rapier2d::geometry::ColliderHandle>
                        })
                        .expect("collider handle failed"),
                );
            }
        }
        _ => panic!("`collider` has to be used with structs."),
    };

    quote! {
        impl let_engine::Collider for #name {
            fn collider_handle(&self) -> Option<let_engine::rapier2d::geometry::ColliderHandle> {
                self.collider_handle
            }
        }
        #[let_engine::objectinit_without_implements]
        #ast
        impl #name {
            pub fn update(&mut self) { // receive
                /*  When updating the local object to the object in the node system of the game
                 *  engine library the collider set doesn't get any change at all. The only change
                 *  happening is the transform of the object from the collider.
                 * */
                let arc = self.reference.clone().unwrap().upgrade().unwrap();
                let object = &arc.lock().object;
                self.transform = object.transform();
                self.appearance = object.appearance().clone();
            }
            pub fn sync(&self) { // send
                /*  When updating a collider it needs to do similar things as the initialisation.
                 *  The collider gets updated with the position it has in the node structure. Sync
                 *  reinitializes the collider and updates the collider handle. The old collider
                 *  gets replaced.
                 * */
                let arc = self.reference.clone().unwrap().upgrade().unwrap();
                let mut object = arc.lock();
                object.object = Box::new(self.clone());
            }
        }
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
            fn init_to_layer(&mut self, id: usize, weak: let_engine::WeakObject) {
                self.id = id;
                self.reference = Some(weak);
                /*  What should a collider do that a normal object doesn't do when initializing it?
                 *  Colliders get added to a collider set when initialized. The position of the
                 *  collider should be the position it has in the node structure.
                 * */
                //if let Some(collider) = self.collider.clone() {
                //    let mut physics = layer.physics.lock();
                //    self.collider_handle = Some(physics.collider_set.insert(collider.collider));
                //}
            }
        }
    }
    .into()
}

/// Implements rigidbody and colliders onto an object for physics
#[proc_macro_attribute]
pub fn rigidbody(_args: TokenStream, input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    quote! {
        
        #[let_engine::collider]
        #ast
    }
    .into()
}
