use discord_presence::client::Client;
use discord_presence::Event;
use std::thread;

use crate::consts::D_PRESENCE;

pub fn start() {
    thread::spawn(|| {
        let mut client = Client::new(D_PRESENCE);

        client.on_ready(|_| {
            println!("INFO: Started Discord rich presence.");
        });

        let handle = client.start();

        client.block_until_event(Event::Ready).unwrap();

        client
            .set_activity(
                |activity| {
                    activity
                        .state("Test".to_string())
                        .details("Testing my own game engine.".to_string())
                }, // .assets(
                   //     |assets|
                   //     assets.small_image("button_angel".to_string())
                   // )
            )
            .expect("Couldn't start discord RPC.");

        handle.join().unwrap();
    });
}
/*
Activity {
            state: Some("Test".into()),
            ..Default::default()
        }
*/
