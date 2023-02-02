pub mod data;
use data::*;

/// Main game object that holds position, size, rotation, color, texture and data.
/// To make your objects appear take an empty object, add your traits and send an receiver
/// of it to the main game object.
#[derive(Clone, Debug)]
pub struct Object {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub rotation: f32,
    pub color: [f32; 4],
    pub graphic: Option<VisualObject>,
}
//game objects have position, size, rotation, color texture and data.
//text objects have position, size, rotation, color, text and font.
impl Object {
    pub fn new() -> Self {
        Self {
            position: [0.0, 0.0],
            size: [1.0, 1.0],
            rotation: 0.0,
            color: [0.0, 0.0, 0.0, 1.0],
            graphic: None,
        }
    }
    pub fn new_square() -> Self {
        Self {
            position: [0.0, 0.0],
            size: [0.5, 0.5],
            rotation: 0.0,
            color: [0.0, 0.0, 0.0, 1.0],
            graphic: Some(VisualObject::new(Display::Data)),
        }
    }
    // pub fn position(&self) -> [f32; 2] {
    //     if let Some(parent) = &self.parent {
    //         let pos: Vec<f32> = self
    //             .position
    //             .iter()
    //             .zip(
    //                 &GAME
    //                     .lock()
    //                     .unwrap()
    //                     .getobject(parent.to_string())
    //                     .position(),
    //             )
    //             .map(|(a, b)| a + b)
    //             .collect();
    //         [pos[0], pos[1]]
    //     } else {
    //         self.position
    //     }
    // }
}

#[derive(Debug, Clone)]
pub struct VisualObject {
    pub texture: Option<String>,
    pub data: Data,
    pub text: Option<String>,
    pub font: Option<String>,
    pub display: Display,
}
impl VisualObject {
    pub fn empty() -> Self {
        Self {
            texture: None,
            data: Data::empty(),
            text: None,
            font: None,
            display: Display::Data,
        }
    }
    pub fn new(display: Display) -> Self {
        Self {
            display: display,
            ..Self::empty()
        }
    }
    pub fn new_square() -> Self {
        Self {
            data: Data::square(),
            ..Self::empty()
        }
    }
    pub fn new_text(text: &str, font: String) -> Self {
        Self {
            text: Some(text.to_string()),
            font: Some(font),
            ..Self::empty()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Display {
    Data,
    Labeled,
}

// pub struct TextObject {
//     pub position: [f32; 2],
//     pub size: [f32; 2],
//     pub rotation: f32,
//     pub color: [f32; 4],
//     pub font: String,
//     pub text: String,
//     pub scale: f32,
//     pub parent: Option<String>,
// }
// impl TextObject {
//     pub fn empty() -> Self {
//         Self {
//             position: [0.0, 0.0],
//             size: [0.0, 0.0],
//             rotation: 0.0,
//             color: [0.0, 0.0, 0.0, 0.0],
//             font: "Bani-Regular".into(),
//             text: "".into(),
//             scale: 24.0,
//             parent: None,
//         }
//     }
//     pub fn position(&self) -> [f32; 2] {
//         if let Some(parent) = &self.parent {
//             let pos: Vec<f32> = self
//                 .position
//                 .iter()
//                 .zip(
//                     &GAME
//                         .lock()
//                         .unwrap()
//                         .getobject(parent.to_string())
//                         .position(),
//                 )
//                 .map(|(a, b)| a + b)
//                 .collect();
//             [pos[0], pos[1]]
//         } else {
//             self.position
//         }
//     }
// }
