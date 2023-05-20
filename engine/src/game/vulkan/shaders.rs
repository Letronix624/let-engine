pub mod vertexshader {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "src/game/vulkan/shaders/default.vert",
    }
}

pub mod fragmentshader {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "src/game/vulkan/shaders/default.frag"
    }
}

pub mod textured_fragmentshader {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "src/game/vulkan/shaders/default_textured.frag"
    }
}

pub mod texture_array_fragmentshader {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "src/game/vulkan/shaders/default_texture_array.frag"
    }
}

pub mod text_fragmentshader {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "src/game/vulkan/shaders/text.frag"
    }
}

//vert:
//  set 0 binding 0 = mvp matrix,
//frag:
//  set 0 binding 1 = color, layer.
//  set 1 binding 0 = texture.
