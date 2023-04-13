pub mod vertexshader {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "src/game/vulkan/shaders/obj.vert",
    }
}

pub mod fragmentshader {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "src/game/vulkan/shaders/obj.frag"
    }
}
