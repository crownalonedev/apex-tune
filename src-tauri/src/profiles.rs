//! Tweak profiles: named sets of tweaks you can save, apply, export and import.

use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::tweaks::{Engine, TweakState};

pub const FORMAT: &str = "apex-tune-profile";
const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// Always `apex-tune-profile`, so a random JSON file isn't mistaken for one.
    pub format: String,
    pub version: u32,
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub tweaks: Vec<String>,
    #[serde(default)]
    pub built_in: bool,
}

impl Profile {
    fn new(id: &str, name: &str, description: &str, tweaks: Vec<String>, built_in: bool) -> Self {
        Self {
            format: FORMAT.into(),
            version: VERSION,
            id: id.into(),
            name: name.into(),
            description: description.into(),
            created_at: Utc::now(),
            tweaks,
            built_in,
        }
    }
}

const COMPETITIVE_EXTRAS: &[&str] = &[
    "game_bar_controller_off",
    "mmcss_responsiveness",
    "network_throttling_off",
    "wu_drivers_off",
    "delivery_optimization_local",
    "transparency_off",
    "animations_off",
    "best_performance_visuals",
    "background_apps_off",
];

pub fn built_ins(engine: &Engine) -> Vec<Profile> {
    let ids = |pred: &dyn Fn(&crate::tweaks::TweakMeta) -> bool| -> Vec<String> {
        engine
            .catalog()
            .iter()
            .filter(|t| pred(t.meta()))
            .map(|t| t.id().to_owned())
            .collect()
    };
    let preset = ids(&|m| m.recommended);
    let mut competitive = preset.clone();
    competitive.extend(COMPETITIVE_EXTRAS.iter().map(|s| (*s).to_owned()));
    let quiet = ids(&|m| {
        m.category == crate::tweaks::Category::Privacy && m.risk == crate::tweaks::Risk::Safe
    });
    vec![
        Profile::new(
            "builtin.gaming",
            "Gaming (Safe preset)",
            "The Optimize for Gaming preset.",
            preset,
            true,
        ),
        Profile::new(
            "builtin.competitive",
            "Competitive",
            "The Safe preset plus Moderate tweaks for lower latency and a lighter desktop.",
            competitive,
            true,
        ),
        Profile::new(
            "builtin.quiet",
            "Clean and quiet",
            "Every Safe suggestion, ad and telemetry setting.",
            quiet,
            true,
        ),
    ]
}

fn dir() -> std::path::PathBuf {
    crate::paths::profiles_dir()
}

fn file_name(id: &str) -> String {
    let safe: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{safe}.json")
}

pub fn saved() -> Vec<Profile> {
    let Ok(entries) = std::fs::read_dir(dir()) else {
        return Vec::new();
    };
    let mut out: Vec<Profile> = entries
        .flatten()
        .filter_map(|e| serde_json::from_slice::<Profile>(&std::fs::read(e.path()).ok()?).ok())
        .filter(|p| p.format == FORMAT)
        .collect();
    out.sort_by_key(|p| std::cmp::Reverse(p.created_at));
    out
}

/// Saves the tweaks currently on as a new profile.
pub fn save_current(engine: &Engine, name: &str) -> std::io::Result<Profile> {
    let tweaks = engine
        .catalog()
        .iter()
        .filter(|t| matches!(t.detect(engine.ctx()), Ok(TweakState::Applied)))
        .map(|t| t.id().to_owned())
        .collect();
    let id = format!("user.{}", uuid::Uuid::new_v4().simple());
    let profile = Profile::new(&id, name.trim(), "Saved from this PC.", tweaks, false);
    write(&profile, &dir().join(file_name(&id)))?;
    Ok(profile)
}

pub fn write(profile: &Profile, path: &Path) -> std::io::Result<()> {
    std::fs::write(path, serde_json::to_vec_pretty(profile)?)
}

pub fn delete(id: &str) -> std::io::Result<()> {
    if id.starts_with("builtin.") {
        return Err(std::io::Error::other("Built-in profiles can't be deleted."));
    }
    std::fs::remove_file(dir().join(file_name(id)))
}

/// Reads a profile file, keeps the tweaks this version knows, and stores it.
/// Returns the profile and any tweak ids that were dropped.
pub fn import(engine: &Engine, path: &Path) -> Result<(Profile, Vec<String>), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Couldn't read the file: {e}"))?;
    let mut profile: Profile = serde_json::from_slice(&bytes)
        .map_err(|_| "That file isn't an Apex Tune profile.".to_owned())?;
    if profile.format != FORMAT {
        return Err("That file isn't an Apex Tune profile.".into());
    }
    let known: Vec<&str> = engine.catalog().iter().map(|t| t.id()).collect();
    let (kept, dropped): (Vec<String>, Vec<String>) = profile
        .tweaks
        .into_iter()
        .partition(|t| known.contains(&t.as_str()));
    profile.tweaks = kept;
    profile.built_in = false;
    profile.id = format!("user.{}", uuid::Uuid::new_v4().simple());
    write(&profile, &dir().join(file_name(&profile.id))).map_err(|e| e.to_string())?;
    Ok((profile, dropped))
}

pub fn find(engine: &Engine, id: &str) -> Option<Profile> {
    built_ins(engine)
        .into_iter()
        .chain(saved())
        .find(|p| p.id == id)
}

/// Tweaks Apex Tune applied that aren't in the profile, for "switch fully to this profile".
pub fn to_turn_off(engine: &Engine, profile: &Profile) -> Vec<String> {
    engine
        .catalog()
        .iter()
        .filter(|t| !profile.tweaks.iter().any(|p| p == t.id()))
        .filter(|t| engine.journal().latest_applied(t.id()).is_some())
        .map(|t| t.id().to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::journal::Journal;
    use crate::sys::fake::FakeSystem;

    #[test]
    fn built_ins_only_name_real_tweaks_and_competitive_extends_the_preset() {
        let sys = FakeSystem::new();
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(
            sys.ctx.clone(),
            Journal::open(dir.path()).unwrap(),
            crate::tweaks::catalog(),
        );
        let profiles = built_ins(&engine);
        for p in &profiles {
            for id in &p.tweaks {
                assert!(
                    engine.tweak(id).is_some(),
                    "{} names unknown tweak {id}",
                    p.name
                );
            }
        }
        assert!(profiles[1].tweaks.len() > profiles[0].tweaks.len());
        assert!(
            profiles[0]
                .tweaks
                .iter()
                .all(|id| profiles[1].tweaks.contains(id))
        );
    }

    #[test]
    fn import_rejects_other_files_and_drops_unknown_tweaks() {
        let sys = FakeSystem::new();
        crate::paths::use_test_data_dir();
        let data = tempfile::tempdir().unwrap();
        let jdir = tempfile::tempdir().unwrap();
        let engine = Engine::new(
            sys.ctx.clone(),
            Journal::open(jdir.path()).unwrap(),
            crate::tweaks::catalog(),
        );

        let other = data.path().join("other.json");
        std::fs::write(&other, r#"{"hello": 1}"#).unwrap();
        assert!(import(&engine, &other).is_err());

        let mut p = Profile::new(
            "x",
            "Mine",
            "",
            vec!["game_mode".into(), "from_the_future".into()],
            true,
        );
        let file = data.path().join("mine.json");
        write(&p, &file).unwrap();
        let (imported, dropped) = import(&engine, &file).unwrap();
        assert_eq!(imported.tweaks, vec!["game_mode"]);
        assert_eq!(dropped, vec!["from_the_future"]);
        assert!(!imported.built_in);
        p.id = imported.id.clone();
        assert!(saved().iter().any(|s| s.id == imported.id));
        delete(&imported.id).unwrap();
        assert!(delete("builtin.gaming").is_err());
    }
}
