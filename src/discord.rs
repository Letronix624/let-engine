use discord_presence::client::Client;
use discord_presence::Event;
use std::thread;

pub struct DiscordPresence {
    client_id: u64,
}
impl DiscordPresence {
    pub fn start(&self) {
        let client_id = self.client_id;
        thread::spawn(move || {
            let mut client = Client::new(client_id);

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
}

/*
Activity {
            state: Some("Test".into()),
            ..Default::default()
        }
*/
