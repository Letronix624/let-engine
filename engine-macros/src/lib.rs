extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse::Parser, parse_macro_input, DeriveInput};

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
                            pub appearance: let_engine::objects::Appearance
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
                fields.named.push(
                    syn::Field::parse_named
                        .parse2(quote! {
                            physics: let_engine::physics::ObjectPhysics
                        })
                        .expect("collider failed"),
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
                        pub camera: let_engine::camera::CameraSettings
                    })
                    .unwrap(),
            );
        }
        quote! {
            impl let_engine::camera::Camera for #name {
                fn settings(&self) -> let_engine::camera::CameraSettings {
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
        #[let_engine::objectinit_without_implements]
        #ast
        impl let_engine::objects::GameObject for #name {
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
            fn appearance(&self) -> &let_engine::objects::Appearance {
                &self.appearance
            }
            fn id(&self) -> usize {
                self.id
            }
            fn init_to_layer(&mut self, id: usize, parent: &let_engine::NObject, mut rigid_body_parent: let_engine::objects::RigidBodyParent, layer: &let_engine::Layer) -> let_engine::NObject {
                self.id = id;
                self.physics.physics = Some(layer.physics.clone());
                self.parent_transform = self.physics.update(&self.transform, parent, &mut rigid_body_parent, id as u128);
                let node: let_engine::NObject = std::sync::Arc::new(let_engine::Mutex::new(let_engine::objects::Node{
                    object: Box::new(self.clone()),
                    parent: Some(std::sync::Arc::downgrade(parent)),
                    rigid_body_parent: rigid_body_parent.clone(),
                    children: vec![],
                }));
                if let Some(value) = &rigid_body_parent {
                    if value.is_none() && self.physics.rigid_body.is_some() {
                        layer.rigid_body_roots.lock().insert(id, node.clone());
                    }
                }

                self.reference = Some(std::sync::Arc::downgrade(&node));
                node
            }
            fn remove_event(&mut self) {
                self.physics.remove()
            }
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn rigidbody_handle(&self) -> Option<let_engine::rapier2d::dynamics::RigidBodyHandle> {
                self.physics.rigid_body_handle
            }
            fn collider_handle(&self) -> Option<let_engine::rapier2d::geometry::ColliderHandle> {
                self.physics.collider_handle
            }
        }
        impl #name {
            pub fn update(&mut self) { // receive
                if let Some(arc) = self.reference.clone().unwrap().upgrade() {
                    let object = &arc.lock().object;
                    self.transform = object.transform();
                    self.appearance = object.appearance().clone();
                } else {
                    Self::remove_event(self);
                }
            }
            pub fn sync(&mut self) { // send
                // update public position of all children recursively
                let node = self.reference.clone().unwrap().upgrade().unwrap();
                {
                    let mut node = node.lock();
                    self.parent_transform = self.physics.update(
                        &self.transform,
                        &node.parent.clone().unwrap().upgrade().unwrap(),
                        &mut node.rigid_body_parent, self.id as u128
                    );
                }
                node.lock().update_children_position(Self::public_transform(self));
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
            pub fn local_collider_position(&self) -> let_engine::Vec2 {
                self.physics.local_collider_position
            }
            pub fn set_local_collider_position(&mut self, pos: let_engine::Vec2) {
                self.physics.local_collider_position = pos;
            }
        }
    }
    .into()
}
