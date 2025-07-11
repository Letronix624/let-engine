//! Default shaders.

use let_engine_core::resources::material::GraphicsShaders;

pub fn default_shaders() -> GraphicsShaders {
    GraphicsShaders::new(
        include_bytes!(concat!(env!("OUT_DIR"), "/default.vert")).to_vec(),
        "main".to_string(),
        include_bytes!(concat!(env!("OUT_DIR"), "/default.frag")).to_vec(),
        "main".to_string(),
    )
}

pub fn default_textured_shaders() -> GraphicsShaders {
    GraphicsShaders::new(
        include_bytes!(concat!(env!("OUT_DIR"), "/textured.vert")).to_vec(),
        "main".to_string(),
        include_bytes!(concat!(env!("OUT_DIR"), "/textured.frag")).to_vec(),
        "main".to_string(),
    )
}

pub fn basic_shaders() -> GraphicsShaders {
    GraphicsShaders::new(
        include_bytes!(concat!(env!("OUT_DIR"), "/basic.vert")).to_vec(),
        "main".to_string(),
        include_bytes!(concat!(env!("OUT_DIR"), "/basic.frag")).to_vec(),
        "main".to_string(),
    )
}
