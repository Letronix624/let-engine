use shaderc::{CompileOptions, Compiler};
use std::{ffi::OsString, fs, path::PathBuf};

fn main() {
    println!("cargo::rerun-if-changed=src/resources/shaders/");

    let compiler = Compiler::new().unwrap();
    let mut options = CompileOptions::new().unwrap();
    options.add_macro_definition("EP", Some("main"));

    // Vec of vertex shaders
    let mut vertex_shaders: Vec<(OsString, String)> = vec![];
    // Vec of fragment shaders
    let mut fragment_shaders: Vec<(OsString, String)> = vec![];

    // Go through every shader in the shaders folder
    for file in fs::read_dir("src/resources/shaders").unwrap() {
        let file_name = file.as_ref().unwrap().file_name();
        if let Some(ending) = file_name.to_str().unwrap().rsplit_once('.') {
            match ending.1 {
                "vert" => {
                    vertex_shaders.push((
                        file_name,
                        fs::read_to_string(file.unwrap().path())
                            .expect("Vertex shader should be a text file"),
                    ));
                }
                "frag" => {
                    fragment_shaders.push((
                        file_name,
                        fs::read_to_string(file.unwrap().path())
                            .expect("Fragment shader should be a text file"),
                    ));
                }
                _ => (),
            }
        }
    }

    let out_dir: PathBuf = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    // Compile each shader and output to the out_dir
    for vertex_shader in vertex_shaders {
        let file_name = vertex_shader.0.to_str().unwrap();
        let shader = compiler
            .compile_into_spirv(
                &vertex_shader.1,
                shaderc::ShaderKind::Vertex,
                file_name,
                "main",
                Some(&options),
            )
            .unwrap();
        let binary = shader.as_binary_u8();
        fs::write(out_dir.join(file_name), binary).unwrap();
    }

    for fragment_shader in fragment_shaders {
        let file_name = fragment_shader.0.to_str().unwrap();
        let shader = compiler
            .compile_into_spirv(
                &fragment_shader.1,
                shaderc::ShaderKind::Fragment,
                file_name,
                "main",
                Some(&options),
            )
            .unwrap();
        let binary = shader.as_binary_u8();
        fs::write(out_dir.join(file_name), binary).unwrap();
    }
}
