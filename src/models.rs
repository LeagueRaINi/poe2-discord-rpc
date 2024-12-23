use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use regex::Captures;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub enum CharacterClass {
    Mercenary,
    Monk,
    Ranger,
    Sorceress,
    Warrior,
    Witch,
}

impl FromStr for CharacterClass {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "mercenary" => Ok(Self::Mercenary),
            "monk" => Ok(Self::Monk),
            "ranger" => Ok(Self::Ranger),
            "sorceress" => Ok(Self::Sorceress),
            "warrior" => Ok(Self::Warrior),
            "witch" => Ok(Self::Witch),
            _ => Err(()),
        }
    }
}

impl fmt::Display for CharacterClass {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl CharacterClass {
    pub fn get_ascendencies(&self) -> Option<[ClassAscendency; 2]> {
        match self {
            Self::Mercenary => {
                Some([ClassAscendency::Witchhunter, ClassAscendency::GemlingLegionnaire])
            },
            Self::Monk => Some([ClassAscendency::AcolyteOfChayula, ClassAscendency::Invoker]),
            Self::Ranger => Some([ClassAscendency::Deadeye, ClassAscendency::Pathfinder]),
            Self::Sorceress => Some([ClassAscendency::Chronomancer, ClassAscendency::Stormweaver]),
            Self::Warrior => Some([ClassAscendency::Titan, ClassAscendency::Warbringer]),
            Self::Witch => Some([ClassAscendency::BloodMage, ClassAscendency::Infernalist]),
        }
    }

    pub fn get_discord_image_name(&self) -> &'static str {
        match self {
            Self::Mercenary => "mercenary",
            Self::Monk => "monk",
            Self::Ranger => "ranger",
            Self::Sorceress => "sorceress",
            Self::Warrior => "warrior",
            Self::Witch => "witch",
        }
    }
}

#[derive(Debug, Clone)]
pub enum ClassAscendency {
    Witchhunter,
    GemlingLegionnaire,
    AcolyteOfChayula,
    Invoker,
    Deadeye,
    Pathfinder,
    Chronomancer,
    Stormweaver,
    Titan,
    Warbringer,
    BloodMage,
    Infernalist,
}

impl FromStr for ClassAscendency {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "witchhunter" => Ok(Self::Witchhunter),
            "gemling legionnaire" => Ok(Self::GemlingLegionnaire),
            "acolyte of chayula" => Ok(Self::AcolyteOfChayula),
            "invoker" => Ok(Self::Invoker),
            "deadeye" => Ok(Self::Deadeye),
            "pathfinder" => Ok(Self::Pathfinder),
            "chronomancer" => Ok(Self::Chronomancer),
            "stormweaver" => Ok(Self::Stormweaver),
            "titan" => Ok(Self::Titan),
            "warbringer" => Ok(Self::Warbringer),
            "blood mage" => Ok(Self::BloodMage),
            "infernalist" => Ok(Self::Infernalist),
            _ => Err(()),
        }
    }
}

impl Display for ClassAscendency {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Witchhunter => write!(f, "Witchhunter"),
            Self::GemlingLegionnaire => write!(f, "Gemling Legionnaire"),
            Self::AcolyteOfChayula => write!(f, "Acolyte of Chayula"),
            Self::Invoker => write!(f, "Invoker"),
            Self::Deadeye => write!(f, "Deadeye"),
            Self::Pathfinder => write!(f, "Pathfinder"),
            Self::Chronomancer => write!(f, "Chronomancer"),
            Self::Stormweaver => write!(f, "Stormweaver"),
            Self::Titan => write!(f, "Titan"),
            Self::Warbringer => write!(f, "Warbringer"),
            Self::BloodMage => write!(f, "Blood Mage"),
            Self::Infernalist => write!(f, "Infernalist"),
        }
    }
}

impl ClassAscendency {
    pub fn get_class(&self) -> CharacterClass {
        match self {
            Self::Witchhunter | Self::GemlingLegionnaire => CharacterClass::Mercenary,
            Self::AcolyteOfChayula | Self::Invoker => CharacterClass::Monk,
            Self::Deadeye | Self::Pathfinder => CharacterClass::Ranger,
            Self::Chronomancer | Self::Stormweaver => CharacterClass::Sorceress,
            Self::Titan | Self::Warbringer => CharacterClass::Warrior,
            Self::BloodMage | Self::Infernalist => CharacterClass::Witch,
        }
    }

    pub fn get_discord_image_name(&self) -> &'static str {
        match self {
            Self::Witchhunter => "mercenary_witchhunter",
            Self::GemlingLegionnaire => "mercenary_gemling_legionnaire",
            Self::AcolyteOfChayula => "monk_acolyte_of_chayula",
            Self::Invoker => "monk_invoker",
            Self::Deadeye => "ranger_deadeye",
            Self::Pathfinder => "ranger_pathfinder",
            Self::Chronomancer => "sorceress_chronomancer",
            Self::Stormweaver => "sorceress_stormweaver",
            Self::Titan => "warrior_titan",
            Self::Warbringer => "warrior_warbringer",
            Self::BloodMage => "witch_blood_mage",
            Self::Infernalist => "witch_infernalist",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Translations {
    pub areas: HashMap<String, String>,
}

impl Translations {
    pub fn get_area_display_name(&self, area: &str) -> Option<String> {
        let (name, is_cruel) = area.strip_prefix("C_").map_or((area, false), |s| (s, true));
        self.areas.get(name).map(|area_name| match is_cruel {
            true => format!("Cruel {area_name}"),
            false => area_name.to_owned(),
        })
    }
}

#[derive(Debug)]
pub struct ClassInfo {
    pub class: CharacterClass,
    pub ascendency: Option<ClassAscendency>,
    pub username: String,
    pub level: u16,
}

impl ClassInfo {
    pub fn parse_from_capture(caps: &Captures, user_blacklist: &[String]) -> Option<Self> {
        let username = caps.get(1).map_or("", |m| m.as_str());
        let class = caps.get(2).map_or("", |m| m.as_str());
        let level = caps.get(3).map_or(0, |m| m.as_str().parse::<u16>().unwrap());

        if user_blacklist.contains(&username.to_owned()) {
            return None;
        }

        let ascd_class = ClassAscendency::from_str(class).ok();
        let main_class = ascd_class
            .clone()
            .map_or_else(|| CharacterClass::from_str(class).unwrap(), |ascd| ascd.get_class());

        Some(Self {
            class: main_class,
            ascendency: ascd_class,
            username: username.to_owned(),
            level,
        })
    }
}

#[derive(Debug)]
pub struct MapChangeInfo {
    pub level: u16,
    pub name: String,
    pub seed: u64,
    pub ts: i64,
}

impl MapChangeInfo {
    pub fn parse_from_captures(caps: &Captures, translations: &Translations) -> Self {
        let level = caps.get(1).map_or(0, |m| m.as_str().parse::<u16>().unwrap());
        let name = caps.get(2).map_or("", |m| m.as_str());
        let seed = caps.get(3).map_or(0, |m| m.as_str().parse::<u64>().unwrap());

        let name = translations.get_area_display_name(name).unwrap_or(name.to_owned());
        let ts = chrono::Utc::now().timestamp();

        Self { level, name: name.to_owned(), seed, ts }
    }
}
