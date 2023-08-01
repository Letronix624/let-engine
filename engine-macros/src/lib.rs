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
            fn set_isometry(&mut self, position: let_engine::Vec2, rotation: f32) {
                self.transform.position = position;
                self.transform.rotation = rotation;
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
            fn init_to_layer(&mut self, id: usize, object: &let_engine::NObject, _layer: &let_engine::Layer) {
                self.id = id;
                let parent = object.lock().parent.clone().unwrap().clone().upgrade().unwrap();
                let parent = &parent.lock().object;
                self.parent_transform = parent.public_transform();
                self.reference = Some(std::sync::Arc::downgrade(object));
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

/// Implements colliders and rigidbodies onto an object.
/// Does the same as object but also has a collider and rigidbody field to edit.
#[proc_macro_attribute]
pub fn collider(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut ast = parse_macro_input!(input as DeriveInput);
    let name = &ast.ident;

    if let syn::Data::Struct(ref mut struct_data) = ast.data {
        if let syn::Fields::Named(fields) = &mut struct_data.fields {
            fields.named.push(
                syn::Field::parse_named
                    .parse2(quote! {
                        physics: let_engine::physics::ObjectPhysics
                    })
                    .expect("collider failed"),
            );
        }
    } else {
        panic!("`collider` has to be used with structs.");
    };

    quote! {
        impl let_engine::Collider for #name {
            fn collider_handle(&self) -> Option<let_engine::rapier2d::geometry::ColliderHandle> {
                self.physics.collider_handle
            }
        }
        #[let_engine::objectinit_without_implements]
        #ast
        impl let_engine::GameObject for #name {
            fn transform(&self) -> Transform {
                self.transform
            }
            fn set_isometry(&mut self, position: let_engine::Vec2, rotation: f32) {
                self.transform.position = position;
                self.transform.rotation = rotation;
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
            fn init_to_layer(&mut self, id: usize, object: &let_engine::NObject, layer: &let_engine::Layer) {
                self.id = id;
                let parent = object.lock().parent.clone().unwrap().clone().upgrade().unwrap();
                let parent = &parent.lock().object;
                self.parent_transform = parent.public_transform();
                self.reference = Some(std::sync::Arc::downgrade(object));
                self.physics.physics = Some(layer.physics.clone());
                self.physics.update(Self::public_transform(self), id as u128);
            }
            fn remove_event(&mut self) {
                self.physics.remove()
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
                node.lock().update_children_position(transform);
                self.physics.update(transform, self.id as u128);
                let arc = self.reference.clone().unwrap().upgrade().unwrap();
                let mut object = arc.lock();
                object.object = Box::new(self.clone());
            }
            pub fn collider(&self) -> Option<&let_engine::physics::Collider> {
                self.physics.collider.as_ref()
            }
            pub fn set_collider(&mut self, collider: Option<let_engine::physics::Collider>) {
                self.physics.collider = collider;
            }
            pub fn collider_mut(&mut self) -> &mut Option<let_engine::physics::Collider> {
                &mut self.physics.collider
            }
            pub fn rigid_body(&self) -> Option<&let_engine::physics::RigidBody> {
                self.physics.rigid_body.as_ref()
            }
            pub fn set_rigid_body(&mut self, rigid_body: Option<let_engine::physics::RigidBody>) {
                self.physics.rigid_body = rigid_body;
            }
            pub fn rigid_body_mut(&mut self) -> &mut Option<let_engine::physics::RigidBody> {
                &mut self.physics.rigid_body
            }
        }
    }
    .into()
}
