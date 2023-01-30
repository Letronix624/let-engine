use rodio::{Decoder, OutputStream};
use std::{io::Cursor, thread};

#[allow(dead_code)]
pub fn memeloop() {
    thread::spawn(|| {
        let (_stream, soundhandle) = OutputStream::try_default().unwrap();
        let bsink = rodio::Sink::try_new(&soundhandle).unwrap();
        bsink.set_volume(0.4);

        let sound = include_bytes!("../assets/sounds/omaga.mp3");

        bsink.append(Decoder::new_mp3(Cursor::new(sound)).unwrap());
        bsink.sleep_until_end();
    });
    thread::spawn(|| {
        let (_stream, soundhandle) = OutputStream::try_default().unwrap();
        let bsink = rodio::Sink::try_new(&soundhandle).unwrap();
        bsink.set_volume(0.4);
        let sound = include_bytes!("../assets/sounds/boom.mp3");
        for _ in 0..3 {
            bsink.append(Decoder::new_mp3(Cursor::new(sound)).unwrap());
        }

        bsink.sleep_until_end();
    });
}
