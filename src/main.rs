#![feature(plugin)]
#![feature(catch_expr)]
#![plugin(rocket_codegen)]

extern crate ws;
extern crate rocket;
extern crate rmpv;
extern crate rand;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate maplit;

#[macro_use] mod utils;
mod incremental_value;
mod network;
mod entities;
mod game_map;
mod game;

use std::thread;
use std::path::{Path, PathBuf};
use ws::listen;
use rocket::response::{NamedFile};

#[get("/")]
fn index() -> Option<NamedFile> {
    NamedFile::open(Path::new("public/index.html")).ok()
}

#[get("/<file..>")]
fn files(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("public/").join(file)).ok()
}

fn main() {
    // Start the game
    let game = game::Game::new();

    // Start the websocket server
    {
        let game = game.clone();
        thread::spawn(move || {
            listen(
                "0.0.0.0:8080",
                |out| network::Client::new(game.clone(), out)
            ).unwrap();
        });
    }

    // Start the rocket server
    rocket::ignite()
        .mount("/", routes![index, files])
        .launch();
}
