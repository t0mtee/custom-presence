use dirs::config_dir;
use discord_presence::{models::ActivityType, Client, Event};
use serde::Deserialize;
use sysinfo::{Pid, System};

use std::cmp;
use std::fs::File;
use std::io::BufReader;
use std::io::ErrorKind::NotFound;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

#[derive(Deserialize)]
struct Game {
    process_name: String,
    app_id: u64,
    state: String,
    activity_type: ActivityType,
    large_text: String,
    large_image: String,
    small_text: String,
    small_image: String,
}

#[derive(Deserialize)]
struct Config {
    games: Vec<Game>,
    scan_time: u64,
    check_time: u64,
}

fn main() {
    let path: PathBuf = [config_dir().unwrap(), PathBuf::from("custom_presence.json")]
        .iter()
        .collect();
    let mut system = System::new_all();

    loop {
        println!("-------------------");

        let file = match File::open(&path) {
            Ok(file) => {
                println!("Config read.");
                file
            }
            Err(error) => {
                if error.kind() == NotFound {
                    println!("Make a config file at {}", path.to_string_lossy());
                }
                return;
            }
        };
        let reader = BufReader::new(file);
        let config: Config =
            serde_json::from_reader(reader).expect("Unable to read the config JSON");

        let mut drpc = Client::new(0);
        let mut pid: Option<Pid> = None;

        system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        'gamescan: for game in config.games {
            println!("Searching for processes that match {}", game.process_name);

            let name_length = if cfg!(target_os = "linux") {
                cmp::min(game.process_name.len(), 15)
            } else {
                game.process_name.len()
            };

            for process in system.processes_by_exact_name(game.process_name[..name_length].as_ref())
            {
                if let None = process.thread_kind() {
                    pid = Some(process.pid());

                    println!("Process {} matches {}", pid.unwrap(), game.process_name);

                    drpc = Client::new(game.app_id);

                    drpc.start();

                    println!("Starting RPC client.");

                    drpc.block_until_event(Event::Ready).unwrap();

                    assert!(Client::is_ready());

                    println!("Client is ready.");

                    // Set the activity
                    drpc.set_activity(|act| {
                        act.state(game.state.as_str())
                            .activity_type(game.activity_type)
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

        match pid {
            Some(p) => {
                while let Some(_) = system.process(p) {
                    sleep(Duration::new(config.check_time, 0));
                    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[p]), true);

                    println!("Process {} is still running.", p);
                    println!(
                        "Sleeping for {} seconds before the next process check.",
                        config.check_time
                    );
                }

                println!("Process ended!");
                drpc.shutdown().unwrap();
            }
            None => {
                println!("No process found.");
            }
        }

        println!(
            "Sleeping for {} seconds before the next scan.",
            config.scan_time
        );

        println!("-------------------");

        sleep(Duration::new(config.scan_time, 0));
    }
}
