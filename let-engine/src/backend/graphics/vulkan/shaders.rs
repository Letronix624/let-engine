//! Default shaders.

use let_engine_core::resources::material::GraphicsShaders;

pub fn default_shader() -> GraphicsShaders {
    GraphicsShaders::new(
        include_bytes!(concat!(env!("OUT_DIR"), "/default.vert")).to_vec(),
        "main".to_string(),
        include_bytes!(concat!(env!("OUT_DIR"), "/default.frag")).to_vec(),
        "main".to_string(),
    )
}

pub fn default_textured_shader() -> GraphicsShaders {
    GraphicsShaders::new(
        include_bytes!(concat!(env!("OUT_DIR"), "/default.vert")).to_vec(),
        "main".to_string(),
        include_bytes!(concat!(env!("OUT_DIR"), "/textured.frag")).to_vec(),
        "main".to_string(),
    )
}
