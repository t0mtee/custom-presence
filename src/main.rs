use discord_presence::{models::ActivityType, Client, Event};
use serde::Deserialize;
use sysinfo::System;

use std::cmp;
use std::env::current_exe;
use std::fs::File;
use std::io::BufReader;
use std::io::ErrorKind::NotFound;
use std::thread::sleep;
use std::time::Duration;

#[derive(Deserialize)]
struct Game {
    process_name: String,
    app_id: u64,
    state: String,
    large_text: String,
    large_image: String,
    small_text: String,
    small_image: String,
}

#[derive(Deserialize)]
struct Config {
    games: Vec<Game>,
    wait_time: u64,
}

fn main() {
    let path = current_exe()
        .expect("Can't find current executable")
        .parent()
        .and_then(|p| p.to_str().map(|s| s.to_owned()))
        .expect("Failed to convert path to string")
        + "/config.json";

    let mut drpc;

    loop {
        println!("-------------------");

        let file = match File::open(&path) {
            Ok(file) => {
                println!("Config read.");
                file
            }
            Err(error) => {
                if error.kind() == NotFound {
                    println!("Make a config file at {}", path);
                }
                return;
            }
        };

        let reader = BufReader::new(file);

        let config: Config =
            serde_json::from_reader(reader).expect("Unable to read the config JSON");

        let system = System::new_all();

        'gamescan: for game in config.games {
            println!("Searching for processes that match {}", game.process_name);

            let name_length = cmp::min(game.process_name.len(), 15);

            for process in system.processes_by_exact_name(game.process_name[..name_length].as_ref())
            {
                if let None = process.thread_kind() {
                    println!("Process {} matches {}", process.pid(), game.process_name);

                    drpc = Client::new(game.app_id);

                    drpc.start();

                    println!("Starting RPC client.");

                    drpc.block_until_event(Event::Ready).unwrap();

                    assert!(Client::is_ready());

                    println!("Client is ready.");

                    // Set the activity
                    drpc.set_activity(|act| {
                        act.state(game.state.as_str())
                            .activity_type(ActivityType::Playing)
                            .assets(|assets| {
                                assets
                                    .large_text(game.large_text.as_str())
                                    .large_image(game.large_image.as_str())
                                    .small_text(game.small_text.as_str())
                                    .small_image(game.small_image.as_str())
                            })
                    })
                    .unwrap();

                    println!("Activity set.");

                    break 'gamescan;
                }
            }
        }

        println!(
            "Sleeping for {} seconds before the next scan.",
            config.wait_time
        );

        println!("-------------------");

        sleep(Duration::new(config.wait_time, 0));
    }
}
