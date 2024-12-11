use std::io::{BufReader, Read, Seek};
use std::path::PathBuf;
use std::str::FromStr;
use std::{fs, thread};

use clap::Parser;
use discord_rich_presence::activity::{Activity, Assets, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use models::{CharacterClass, ClassAscendency, ClassInfo, MapChangeInfo, Translations};
use regex::Regex;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

mod models;

const DEFAULT_TRANSLATIONS: &str = include_str!("../resources/translations_en.json");
const DEFAULT_DIRECTORIES: [&str; 2] = [
    "C:\\Program Files (x86)\\Grinding Gear Games\\Path of Exile 2",
    "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Path of Exile 2",
];

const PROCESS_NAMES: [&str; 4] =
    ["PathOfExile_x64Steam.exe", "PathOfExile_x64.exe", "PathOfExileSteam.exe", "PathOfExile.exe"];

#[derive(Parser, Debug)]
#[clap(about, author, version)]
struct Opt {
    /// Path to the game directory
    #[arg(short, long)]
    game_dir: Option<PathBuf>,

    /// Path to translations.json
    #[arg(short, long)]
    translations_file: Option<PathBuf>,
}

fn is_poe_running(sys: &mut System) -> bool {
    sys.refresh_processes(ProcessesToUpdate::All, true);
    sys.processes_by_name("PathOfExile".as_ref())
        .any(|p| p.name().to_str().is_some_and(|n| PROCESS_NAMES.contains(&n)))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {} {}] {}",
                humantime::format_rfc3339(std::time::SystemTime::now()),
                record.level(),
                record.target(),
                message
            ))
        })
        .chain(fern::Dispatch::new().level(log::LevelFilter::Info).chain(std::io::stdout()))
        .chain(
            fern::Dispatch::new()
                .level(log::LevelFilter::Trace)
                .chain(fern::log_file("poe2-drpc.log")?),
        )
        .apply()?;

    let Opt { game_dir, translations_file } = Opt::parse();
    log::trace!("Args: {{ game_dir: {game_dir:?}, translations_file: {translations_file:?} }}");

    let translations: Translations = serde_json::from_str(
        &translations_file
            .map(|f| fs::read_to_string(f).unwrap())
            .unwrap_or(DEFAULT_TRANSLATIONS.to_string()),
    )?;
    log::trace!("Translations: {translations:#?}");

    let game_dir = game_dir
        .or_else(|| {
            DEFAULT_DIRECTORIES
                .iter()
                .find(|&d| fs::metadata(d).is_ok())
                .map(|d| d.to_string())
                .map(PathBuf::from)
        })
        .ok_or("Game directory not found")?;
    log::trace!("Game directory: {game_dir:?}");

    let log_file = game_dir.join("logs/Client.txt");
    let log_file = fs::File::open(log_file)?;
    log::trace!("Opened log file");

    let mut rpc = DiscordIpcClient::new("550890770056347648")?;
    log::info!("Created discord ipc client");

    let mut sys = System::new_with_specifics(
        RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
    );
    log::info!("Created sysinfo");

    let level_up_rgx = Regex::new(r#": (\w+) \((\w+)\) is now level (\d+)"#)?;
    let instance_rgx = Regex::new(r#"] Generating level (\d+) area "([^"]+)" with seed (\d+)"#)?;

    let mut log_bufr = BufReader::new(log_file);

    let mut last_instance: Option<MapChangeInfo> = None;
    let mut last_class: Option<ClassInfo> = None;

    log::trace!("Starting main loop");
    loop {
        if !is_poe_running(&mut sys) {
            thread::sleep(std::time::Duration::from_secs(5));
            continue;
        }

        rpc.connect()?;
        log::trace!("Connected to discord rpc");

        // I'm lazy so lets seek back to start to set the activity after reconnecting
        log_bufr.seek(std::io::SeekFrom::Start(0))?;

        while is_poe_running(&mut sys) {
            let mut log = String::new();

            if log_bufr.read_to_string(&mut log)? == 0 {
                thread::sleep(std::time::Duration::from_millis(500));
                continue;
            }

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

                    log::trace!(
                        "Level up: {{ username: {username}, class: {main_class}, ascendency: {ascd_class:?}, level: {level} }}"
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

                    log::trace!("Instance change: {{ lvl: {level}, name: {name}, seed: {seed} }}");

                    last_instance = Some(MapChangeInfo { level, name: name.to_owned(), seed, ts });
                }
            }

            if last_class.is_some() || last_instance.is_some() {
                log::info!(
                    "Updating activity {{ class: {last_class:#?}, instance: {last_instance:#?} }}"
                );

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
                log::trace!("Set activity");
            }
        }

        rpc.clear_activity()?;
        log::trace!("Cleared activity");

        rpc.close()?;
        log::trace!("Disconnected from discord rpc");
    }
}
