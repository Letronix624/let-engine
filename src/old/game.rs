mod player;

use std::collections::HashMap;

#[allow(unused_imports)]
use super::{
    client, delta_time, discord, fps, resources::*, sound, window, BACKGROUND, BACKGROUND_ID,
    SQUARE,
};
use crate::Object;

#[allow(unused_imports)]
use client::{get_ping, Client};

#[derive(Clone, Copy)]
pub struct InputState {
    pub w: bool,
    pub s: bool,
    pub a: bool,
    pub d: bool,
    pub q: bool,
    pub e: bool,
    pub r: bool,
    pub mouse: (f32, f32),
    pub lmb: bool,
    pub mmb: bool,
    pub rmb: bool,
    pub vsd: f32,
}
impl InputState {
    pub fn new() -> Self {
        Self {
            w: false,
            s: false,
            a: false,
            d: false,
            q: false,
            e: false,
            r: false,
            mouse: (0.0, 0.0),
            lmb: false,
            mmb: false,
            rmb: false,
            vsd: 0.0,
        }
    }
    pub fn get_xy(&self) -> (f32, f32) {
        let x = (self.d as i32 - self.a as i32) as f32;
        let y = (self.w as i32 - self.s as i32) as f32;
        let sx = x.abs() * 4.0 - x.abs() * y.abs() * 4.0 / 2.0;
        let sy = y.abs() * 4.0 - y.abs() * x.abs() * 4.0 / 2.0;

        (x * (sx.sqrt() / 2.0), -y * (sy.sqrt() / 2.0))
    }
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct Game {
    pub objects: HashMap<String, Object>,
    pub renderorder: Vec<String>, //variable that has the order of object render
    pub input: InputState,
    pub resources: Resources,
    // client: Client,
    // olddata: Object,
}

impl Game {
    pub fn init() -> Self {
        Self {
            objects: HashMap::new(),
            renderorder: vec![],
            input: InputState::new(),
            resources: Resources::load_all(), // client: Client::new(),
                                              // olddata: Object::empty(),
        }
    }
    pub fn getobject(&self, name: String) -> Object {
        return self.objects[&name].clone();
    }
    pub fn setobject(&mut self, name: String, object: Object) {
        self.objects.insert(name, object);
    }
    #[allow(unused)]
    fn deleteobject(&mut self, name: String) {
        self.objects.remove(&name);
        let index = self.renderorder.iter().position(|x| *x == name).unwrap();
        self.renderorder.remove(index);
    }
    fn newobject(&mut self, name: String, obj: Object) {
        self.objects.insert(name.clone(), obj);
        self.renderorder.push(name);
    }
    pub fn start(&mut self) {
        //Runs one time before the first Frame.
        player::start(self);
        // self.newobject(
        //     "background".to_string(),
        //     [0.1, 0.3, 0.9, 1.0],
        //     BACKGROUND.to_vec(),
        //     BACKGROUND_ID.to_vec(),
        //     [0.0, 0.0],
        //     [1.0, 1.0],
        //     0.0,
        // );

        //let _ = self.client.connect(); //Connects to the server (seflon.ddns.net) if its available

        println!("{:?}", self.renderorder);

        discord::start();

        //sound::memeloop();
    }
    pub fn main(&mut self) {
        //Runs every single frame once.
        player::main(self);

        self.input.vsd = 0.0;
    }

    pub fn late_main(&mut self) {
        //Runs every time after the redraw events are done.
        player::late_main(self);
    }

    pub fn tick(&mut self) {
        // Runs 62.4 times per second.
        player::tick(self);

        // println!("FPS:{} Ping:{}", fps(), get_ping());
        // if self.client.connected {
        //     //Client data sender
        //     let player = self.getobject("player1".to_string());
        //     if self.olddata.position != player.position || self.olddata.size != player.size {
        //         match self.client.sendobject(player) {
        //             _ => (),
        //         };
        //         self.olddata = self.getobject("player1".to_string());
        //     }
        //     {
        //         let objects = client::GAMEOBJECTS.lock().unwrap();
        //         for object in objects.iter() {
        //             if self.objects.contains_key(object.0) {
        //                 self.setobject(object.0.clone(), object.1.clone());
        //             } else {
        //                 self.newobject(
        //                     object.0.clone(),
        //                     object.1.clone().data,
        //                     object.1.position,
        //                     object.1.position,
        //                     0.0,
        //                 )
        //             }
        //         }
        //     }
        // }
    }
}
