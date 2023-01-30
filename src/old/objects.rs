use crate::{data::Data, GAME};
#[derive(Clone, Debug)]
pub struct Object {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub rotation: f32,
    pub color: [f32; 4],
    pub texture: Option<String>,
    pub data: Data,
    pub parent: Option<String>,
}
impl Object {
    pub fn empty() -> Self {
        Self {
            position: [0.0, 0.0],
            size: [0.0, 0.0],
            rotation: 0.0,
            color: [0.0, 0.0, 0.0, 0.0],
            texture: None,
            data: Data::empty(),
            parent: None,
        }
    }
    pub fn position(&self) -> [f32; 2] {
        if let Some(parent) = &self.parent {
            let pos: Vec<f32> = self
                .position
                .iter()
                .zip(
                    &GAME
                        .lock()
                        .unwrap()
                        .getobject(parent.to_string())
                        .position(),
                )
                .map(|(a, b)| a + b)
                .collect();
            [pos[0], pos[1]]
        } else {
            self.position
        }
    }
}

pub struct TextObject {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub rotation: f32,
    pub color: [f32; 4],
    pub font: String,
    pub text: String,
    pub scale: f32,
    pub parent: Option<String>,
}
impl TextObject {
    pub fn empty() -> Self {
        Self {
            position: [0.0, 0.0],
            size: [0.0, 0.0],
            rotation: 0.0,
            color: [0.0, 0.0, 0.0, 0.0],
            font: "Bani-Regular".into(),
            text: "".into(),
            scale: 24.0,
            parent: None,
        }
    }
    pub fn position(&self) -> [f32; 2] {
        if let Some(parent) = &self.parent {
            let pos: Vec<f32> = self
                .position
                .iter()
                .zip(
                    &GAME
                        .lock()
                        .unwrap()
                        .getobject(parent.to_string())
                        .position(),
                )
                .map(|(a, b)| a + b)
                .collect();
            [pos[0], pos[1]]
        } else {
            self.position
        }
    }
}
