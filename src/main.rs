use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::{fs, thread};

use clap::Parser;
use discord_rich_presence::activity::{Activity, Assets, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use lazy_static::lazy_static;
use models::{ClassInfo, MapChangeInfo, Translations};
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

lazy_static! {
    static ref RGX_GENERATING_AREA: Regex = Regex::new(r#"] Generating level (\d+) area "([^"]+)" with seed (\d+)"#).unwrap();
    static ref RGX_JOINED_AREA: Regex = Regex::new(r#": (\w+) has joined the area."#).unwrap();
    static ref RGX_LEVEL_UP: Regex = Regex::new(r#": (\w+) \((\w+)\) is now level (\d+)"#).unwrap();
}

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

    let mut log_bufr = BufReader::new(log_file);

    let mut activity = Activity::new();
    let mut last_area: Option<MapChangeInfo> = None;
    let mut last_class: Option<ClassInfo> = None;
    let mut user_blacklist: Vec<String> = Vec::new();

    log::info!("Starting main loop");
    loop {
        if !is_poe_running(&mut sys) {
            thread::sleep(std::time::Duration::from_secs(5));
            continue;
        }

        rpc.connect()?;
        log::trace!("Connected to discord rpc");

        let mut log_str = String::new();
        log_bufr.read_to_string(&mut log_str)?;

        RGX_JOINED_AREA.captures_iter(&log_str).for_each(|caps| {
            if let Some(username) = caps.get(1) {
                user_blacklist.push(username.as_str().to_owned());
            }
        });
        log::trace!("Initial user blacklist: {user_blacklist:#?}");

        if let Some(last_class_info) = RGX_LEVEL_UP
            .captures_iter(&log_str)
            .filter_map(|caps| ClassInfo::parse_from_capture(&caps, &user_blacklist))
            .last()
        {
            last_class = Some(last_class_info);
        }
        log::trace!("Initial class info: {last_class:#?}");

        log_bufr.seek(SeekFrom::End(0))?;

        while is_poe_running(&mut sys) {
            let mut log_line = String::new();

            if log_bufr.read_line(&mut log_line)? == 0 {
                if last_class.is_some() || last_area.is_some() {
                    log::info!(
                        "Updating activity {{ class: {last_class:#?}, instance: {last_area:#?} }}"
                    );

                    if let Some(mut class_info) = last_class.take() {
                        activity = activity.details(class_info.username);

                        let mut assets = Assets::default();
                        if let Some(ascd) = class_info.ascendency.take() {
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

                        activity = activity.assets(assets);
                    }

                    if let Some(instance_info) = last_area.take() {
                        activity = activity
                            .state(format!("{} ({})", &instance_info.name, instance_info.level))
                            .timestamps(Timestamps::default().start(instance_info.ts));
                    }

                    rpc.set_activity(activity.clone())?;
                }
                thread::sleep(std::time::Duration::from_millis(500));
                continue;
            }

            if let Some(class_info) = RGX_LEVEL_UP
                .captures(&log_line)
                .and_then(|caps| ClassInfo::parse_from_capture(&caps, &user_blacklist))
            {
                last_class = Some(class_info);
            } else if let Some(area_info) = RGX_GENERATING_AREA
                .captures(&log_line)
                .map(|caps| MapChangeInfo::parse_from_captures(&caps, &translations))
            {
                last_area = Some(area_info);
            } else if let Some(caps) = RGX_JOINED_AREA.captures(&log_line) {
                let username = caps[1].to_string();
                if !user_blacklist.contains(&username) {
                    user_blacklist.push(username);
                }
            }
        }

        rpc.clear_activity()?;
        log::trace!("Cleared activity");

        rpc.close()?;
        log::trace!("Disconnected from discord rpc");
    }
}
