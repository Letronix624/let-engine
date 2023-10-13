// Default shaders.

pub mod vertexshader {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "src/shaders/default.vert",
    }
}

pub mod fragmentshader {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "src/shaders/default.frag"
    }
}

pub mod textured_fragmentshader {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "src/shaders/default_textured.frag"
    }
}

pub mod texture_array_fragmentshader {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "src/shaders/default_texture_array.frag"
    }
}

pub mod text_fragmentshader {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "src/shaders/text.frag"
    }
}
