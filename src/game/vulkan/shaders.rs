pub mod vertexshader {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "src/game/vulkan/shaders/obj.vs",
        types_meta: {
            use bytemuck::{Pod, Zeroable};

            #[derive(Clone, Copy, Zeroable, Pod)]
        }
    }
}

pub mod fragmentshader {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "src/game/vulkan/shaders/obj.fs"
    }
}

pub mod text_vertexshader {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "src/game/vulkan/shaders/text.vs"
    }
}

pub mod text_fragmentshader {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "src/game/vulkan/shaders/text.fs"
    }
}
