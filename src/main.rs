use std::io::{BufReader, Read};
use std::path::PathBuf;
use std::str::FromStr;
use std::{env, fs};

use clap::Parser;
use discord_rich_presence::activity::{Activity, Assets, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use models::{CharacterClass, ClassAscendency, Translations};
use regex::Regex;

mod models;

#[derive(Parser, Debug)]
#[clap(about, author, version)]
struct Opt {
    /// Path to the game directory
    #[arg(long)]
    game_dir: PathBuf,

    /// Path to translations.json
    #[arg(long)]
    translations: PathBuf,
}

#[derive(Debug)]
struct ClassInfo {
    class: CharacterClass,
    ascendency: Option<ClassAscendency>,
    username: String,
    level: u16,
}

#[derive(Debug)]
struct MapChangeInfo {
    level: u16,
    name: String,
    seed: u64,
    ts: i64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env::set_var("RUST_LOG", env::var("RUST_LOG").unwrap_or_else(|_| "info".into()));
    pretty_env_logger::init();

    let Opt { game_dir, translations } = Opt::parse();

    let translations = fs::read_to_string(translations)?;
    let translations: Translations = serde_json::from_str(&translations)?;
    log::debug!("{:#?}", translations);

    let log_file = game_dir.join("logs/Client.txt");
    let log_file = fs::File::open(log_file)?;

    let mut rpc = DiscordIpcClient::new("550890770056347648")?;
    rpc.connect()?;
    rpc.set_activity(Activity::new().details("Booting up..."))?;

    let mut log_bufr = BufReader::new(log_file);

    let level_up_rgx = Regex::new(r#": (\w+) \((\w+)\) is now level (\d+)"#)?;
    let instance_rgx = Regex::new(r#"] Generating level (\d+) area "([^"]+)" with seed (\d+)"#)?;

    let mut last_instance: Option<MapChangeInfo> = None;
    let mut last_class: Option<ClassInfo> = None;

    loop {
        let mut log = String::new();

        if log_bufr.read_to_string(&mut log)? == 0 {
            // hush little cpu don't you cry
            std::thread::sleep(std::time::Duration::from_millis(500));
            continue;
        }

        // this is written like it is because we have to assume the character from the past logs,
        // we dont have an api to call yet
        for line in log.lines() {
            if let Some(caps) = level_up_rgx.captures(line) {
                let username = caps.get(1).map_or("", |m| m.as_str());
                let class = caps.get(2).map_or("", |m| m.as_str());
                let level = caps.get(3).map_or(0, |m| m.as_str().parse::<u16>().unwrap());

                let ascd_class = ClassAscendency::from_str(class).ok();
                let main_class = ascd_class.clone().map_or_else(
                    || CharacterClass::from_str(class).unwrap(),
                    |ascd| ascd.get_class(),
                );

                log::info!(
                    "Leveled up: {{ username: {username}, class: {main_class}, ascendency: {ascd_class:?}, level: {level} }}"
                );

                last_class = Some(ClassInfo {
                    class: main_class,
                    ascendency: ascd_class,
                    username: username.to_owned(),
                    level,
                });
            } else if let Some(caps) = instance_rgx.captures(line) {
                let level = caps.get(1).map_or(0, |m| m.as_str().parse::<u16>().unwrap());
                let name = caps.get(2).map_or("", |m| m.as_str());
                let seed = caps.get(3).map_or(0, |m| m.as_str().parse::<u64>().unwrap());

                let name = translations.get_area_display_name(name).unwrap_or(name);
                let ts = chrono::Utc::now().timestamp();

                log::info!("Changed instance: {{ lvl: {level}, name: {name}, seed: {seed} }}");

                last_instance = Some(MapChangeInfo { level, name: name.to_owned(), seed, ts });
            }
        }

        if last_class.is_some() || last_instance.is_some() {
            let mut act = Activity::new();

            if let Some(class_info) = last_class.take() {
                act = act.details(class_info.username);

                let mut assets = Assets::default();

                if let Some(ascd) = class_info.ascendency {
                    assets = assets
                        .large_image(ascd.get_discord_image_name())
                        .large_text(format!("{ascd} ({})", class_info.level))
                        .small_image(class_info.class.get_discord_image_name())
                        .small_text(class_info.class);
                } else {
                    assets = assets
                        .large_image(class_info.class.get_discord_image_name())
                        .large_text(format!("{} ({})", class_info.class, class_info.level));
                }

                act = act.assets(assets);
            }

            if let Some(instance_info) = last_instance.take() {
                act = act
                    .state(format!("{} ({})", &instance_info.name, instance_info.level))
                    .timestamps(Timestamps::default().start(instance_info.ts));
            }

            rpc.set_activity(act)?;
            log::info!("Updated activity");
        }
    }
}