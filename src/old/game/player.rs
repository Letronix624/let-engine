use crate::{
    data::{self, *},
    delta_time, Game, Object,
};

pub fn start(game: &mut Game) {
    //Runs one time before the first Frame.

    game.newobject(
        "player1".to_string(),
        Object {
            position: [0.0, 0.0],
            size: [0.9, 0.9],
            rotation: 0.0,
            color: [0.1, 0.0, 0.0, 1.0],
            texture: Some("rusty".into()),
            data: Data::square(),
            parent: None,
        },
    );
}
pub fn main(game: &mut Game) {
    //Runs every single frame once.

    let mut player = game.getobject("player1".to_string());
    player.position = [
        player.position[0] + delta_time() as f32 * game.input.get_xy().0 * 0.6,
        player.position[1] + delta_time() as f32 * game.input.get_xy().1 * 0.6,
    ];

    player.rotation +=
        delta_time() as f32 * (game.input.rmb as i32 - game.input.lmb as i32) as f32 * 5.0;
    // player.size = player.size.map(|x| {
    //     x + delta_time() as f32
    //         * (game.input.e as i32 - game.input.q as i32) as f32
    //         * player.size[0]
    //         * 2.0
    // });
    player.size = [
        player.size[0]
            + delta_time() as f32
                * (game.input.e as i32 - game.input.q as i32) as f32
                * player.size[0],
        player.size[1]
            + delta_time() as f32
                * (game.input.e as i32 - game.input.q as i32) as f32
                * player.size[1],
    ];
    if game.input.r {
        player.position = [0.0, 0.0];
        player.rotation = 0.0;
    }
    // if player.data.len() <= 9 && game.input.vsd == -1.0 {
    //     game.input.vsd = 0.0;
    // }
    // player.data = data::make_circle(
    //     ((player.data.len() / 3) as isize + game.input.vsd as isize) as usize,
    // );
    game.input.vsd = 0.0;

    game.setobject("player1".to_string(), player);
}

pub fn late_main(game: &mut Game) {
    //Runs every time after the redraw events are done.
}

pub fn tick(game: &mut Game) {
    //Runs 62.4 times per second.
}
