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
            fn public_transform(&self) -> Transform {
                self.transform.combine(self.parent_transform)
            }
            fn set_parent_transform(&mut self, transform: Transform) {
                self.parent_transform = transform;
            }
            fn appearance(&self) -> &Appearance {
                &self.appearance
            }
            fn id(&self) -> usize {
                self.id
            }
            fn init_to_layer(&mut self, id: usize, weak: let_engine::WeakObject, _layer: &let_engine::Layer) {
                self.id = id;
                let node = weak.clone().upgrade().unwrap();
                let parent = node.lock().parent.clone().unwrap().clone().upgrade().unwrap();
                let parent = &parent.lock().object;
                self.parent_transform = parent.public_transform();
                self.reference = Some(weak);
            }
            fn remove_event(&mut self) {}
        }
    }
    .into()
}

#[proc_macro_attribute]
pub fn objectinit(_args: TokenStream, input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let name = &ast.ident;
    quote! {
        #[let_engine::objectinit_without_implements]
        #ast
        impl #name {
            pub fn update(&mut self) {
                let arc = self.reference.clone().unwrap().upgrade().unwrap();
                let object = &arc.lock().object;
                self.transform = object.transform();

                self.appearance = object.appearance().clone();
            }
            pub fn sync(&self) {
                // update public position of children
                let transform = Self::public_transform(self);
                let node = self.reference.clone().unwrap().upgrade().unwrap();
                node.lock().update_children_position(transform);

                let arc = self.reference.clone().unwrap().upgrade().unwrap();
                let mut object = arc.lock();
                object.object = Box::new(self.clone());
            }
        }
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
                            parent_transform: let_engine::Transform
                        })
                        .expect("public transform failed"),
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
        #[let_engine::object]
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

    if let syn::Data::Struct(ref mut struct_data) = ast.data {
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
            fields.named.push(
                syn::Field::parse_named
                    .parse2(quote! {
                        physics: Option<let_engine::physics::APhysics>
                    })
                    .expect("physics failed"),
            );
        }
    } else {
        panic!("`collider` has to be used with structs.");
    };

    quote! {
        impl let_engine::Collider for #name {
            fn collider_handle(&self) -> Option<let_engine::rapier2d::geometry::ColliderHandle> {
                self.collider_handle
            }
        }
        #[let_engine::objectinit_without_implements]
        #ast
        impl let_engine::GameObject for #name {
            fn transform(&self) -> Transform {
                self.transform
            }
            fn public_transform(&self) -> Transform {
                self.transform.combine(self.parent_transform)
            }
            fn set_parent_transform(&mut self, transform: Transform) {
                self.parent_transform = transform;
            }
            fn appearance(&self) -> &Appearance {
                &self.appearance
            }
            fn id(&self) -> usize {
                self.id
            }
            fn init_to_layer(&mut self, id: usize, weak: let_engine::WeakObject, layer: &let_engine::Layer) {
                self.id = id;
                let node = weak.clone().upgrade().unwrap();
                let parent = node.lock().parent.clone().unwrap().clone().upgrade().unwrap();
                let parent = &parent.lock().object;
                self.parent_transform = parent.public_transform();
                self.reference = Some(weak);
                self.physics = Some(layer.physics.clone());


                if let Some(mut collider) = self.collider.clone() {
                    let transform = Self::public_transform(self);
                    collider.collider.set_position((transform.position, transform.rotation).into());
                    collider.collider.user_data = id as u128;
                    self.collider_handle = Some(layer.physics.clone().lock().collider_set.insert(collider.collider));
                }
            }
            fn remove_event(&mut self) {
                let physics = self.physics.clone().unwrap();
                if let Some(collider_handle) = self.collider_handle {
                    let mut rigid_body_set = physics.lock().rigid_body_set.clone();
                    let mut island_manager = physics.lock().island_manager.clone();
                    physics.lock().collider_set.remove(
                        collider_handle,
                        &mut island_manager,
                        &mut rigid_body_set,
                        true
                    );
                    let mut physics = physics.lock();
                    physics.rigid_body_set = rigid_body_set;
                    physics.island_manager = island_manager;
                }
            }
        }
        impl #name {
            pub fn update(&mut self) { // receive
                let arc = self.reference.clone().unwrap().upgrade().unwrap();
                let object = &arc.lock().object;
                self.transform = object.transform();
                self.appearance = object.appearance().clone();
            }
            pub fn sync(&mut self) { // send
                // update public position of all children recursively
                let transform = Self::public_transform(self);
                let node = self.reference.clone().unwrap().upgrade().unwrap();
                let physics = self.physics.clone().unwrap();
                node.lock().update_children_position(transform);
                if let Some(mut collider) = self.collider.clone() {
                    if let Some(collider_handle) = self.collider_handle {
                        if let Some(mut public_collider) = self.physics.clone()
                        .unwrap().lock().collider_set.get_mut(collider_handle) {
                            *public_collider = collider.collider.clone();
                            public_collider.set_position((transform.position, transform.rotation).into());
                        }
                    } else {
                        collider.collider.set_position((transform.position, transform.rotation).into());
                        collider.collider.user_data = self.id as u128;
                        self.collider_handle = Some(physics.lock().collider_set.insert(collider.collider));
                    }
                } else if let Some(collider_handle) = self.collider_handle {
                    let mut rigid_body_set = physics.lock().rigid_body_set.clone();
                    let mut island_manager = physics.lock().island_manager.clone();
                    physics.lock().collider_set.remove(
                        collider_handle,
                        &mut island_manager,
                        &mut rigid_body_set,
                        true
                    );
                    let mut physics = physics.lock();
                    physics.rigid_body_set = rigid_body_set;
                    physics.island_manager = island_manager;
                }
                let arc = self.reference.clone().unwrap().upgrade().unwrap();
                let mut object = arc.lock();
                object.object = Box::new(self.clone());
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
