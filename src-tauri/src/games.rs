//! Installed games (known titles, Steam, Epic), per-game tweaks, and the
//! Minecraft Java JVM helper.
//!
//! Per-game tweaks only write documented registry settings that Windows
//! applies at launch. Apex Tune never opens, reads or modifies a running
//! game process.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::sys::{Hive, RegData, SysCtx, SysResult};
use crate::tweaks::atom::Atom;

const IFEO: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options";
const LAYERS: &str = r"Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers";
const FSO_FLAG: &str = "DISABLEDXMAXIMIZEDWINDOWEDMODE";
/// IFEO CpuPriorityClass 3 = High.
const HIGH_PRIORITY: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GameSource {
    Known,
    Steam,
    Epic,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub name: String,
    /// The executable the game actually runs as, e.g. `VALORANT-Win64-Shipping.exe`.
    pub exe: String,
    /// Full path when known; needed for the fullscreen-optimizations setting.
    pub path: Option<String>,
    pub source: GameSource,
    /// Shown next to the game, e.g. that `javaw.exe` covers every Java app.
    pub note: Option<String>,
    pub high_priority: bool,
    pub fso_disabled: bool,
}

// ---- Detection ------------------------------------------------------------------

/// Quoted tokens from Valve's KeyValues text format, in order.
fn vdf_tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '"' {
            continue;
        }
        let mut token = String::new();
        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    if let Some(next) = chars.next() {
                        token.push(next);
                    }
                }
                '"' => break,
                _ => token.push(c),
            }
        }
        out.push(token);
    }
    out
}

/// Values for `key` in a flat key/value walk of a VDF/ACF file.
fn vdf_values(text: &str, key: &str) -> Vec<String> {
    let tokens = vdf_tokens(text);
    tokens
        .windows(2)
        .filter(|w| w[0].eq_ignore_ascii_case(key))
        .map(|w| w[1].clone())
        .collect()
}

const NOT_GAME_EXES: &[&str] = &[
    "unins",
    "crash",
    "redist",
    "setup",
    "vcredist",
    "vc_redist",
    "dxsetup",
    "dotnet",
    "easyanticheat",
    "beservice",
    "battleye",
    "report",
    "helper",
    "updater",
    "cefprocess",
    "webhelper",
];

/// The biggest plausible executable in a game folder (two levels deep is
/// enough for almost every Steam game; Unreal games are handled by name).
pub fn main_exe(dir: &Path) -> Option<PathBuf> {
    fn walk(dir: &Path, depth: u32, best: &mut Option<(u64, PathBuf)>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() && depth > 0 {
                walk(&path, depth - 1, best);
            } else if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
            {
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_ascii_lowercase();
                if NOT_GAME_EXES.iter().any(|n| name.contains(n)) {
                    continue;
                }
                // Unreal games ship a small launcher; the real game is *-Shipping.exe.
                let size = meta.len()
                    + if name.contains("-shipping") {
                        1 << 40
                    } else {
                        0
                    };
                if best.as_ref().is_none_or(|(b, _)| size > *b) {
                    *best = Some((size, path));
                }
            }
        }
    }
    let mut best = None;
    walk(dir, 3, &mut best);
    best.map(|(_, p)| p)
}

fn steam_games(ctx: &SysCtx) -> SysResult<Vec<Game>> {
    let Some(RegData::Sz(steam)) =
        ctx.registry
            .get(Hive::CurrentUser, r"Software\Valve\Steam", "SteamPath")?
    else {
        return Ok(Vec::new());
    };
    let steam = PathBuf::from(steam.replace('/', "\\"));
    let mut libraries = vec![steam.clone()];
    if let Ok(text) = std::fs::read_to_string(steam.join(r"steamapps\libraryfolders.vdf")) {
        for p in vdf_values(&text, "path") {
            let p = PathBuf::from(p.replace("\\\\", "\\"));
            if !libraries.contains(&p) {
                libraries.push(p);
            }
        }
    }
    let mut games = Vec::new();
    for lib in libraries {
        let apps = lib.join("steamapps");
        let Ok(entries) = std::fs::read_dir(&apps) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(entry.path()) else {
                continue;
            };
            let (Some(title), Some(dir)) = (
                vdf_values(&text, "name").into_iter().next(),
                vdf_values(&text, "installdir").into_iter().next(),
            ) else {
                continue;
            };
            // Steam's own runtimes and redistributables aren't games.
            if title.starts_with("Steamworks")
                || title.contains("Redistributable")
                || title.starts_with("Proton")
            {
                continue;
            }
            if let Some(exe) = main_exe(&apps.join("common").join(&dir)) {
                games.push(game(title, &exe, GameSource::Steam, None));
            }
        }
    }
    Ok(games)
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct EpicManifest {
    display_name: String,
    install_location: String,
    #[serde(default)]
    launch_executable: String,
}

fn epic_games() -> Vec<Game> {
    let Some(data) = std::env::var_os("ProgramData") else {
        return Vec::new();
    };
    let dir = PathBuf::from(data).join(r"Epic\EpicGamesLauncher\Data\Manifests");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "item"))
        .filter_map(|e| serde_json::from_slice::<EpicManifest>(&std::fs::read(e.path()).ok()?).ok())
        .filter_map(|m| {
            let root = PathBuf::from(&m.install_location);
            // Fortnite's launch executable is a launcher; the game runs as the -Shipping client.
            let exe = if m.display_name.eq_ignore_ascii_case("Fortnite") {
                root.join(r"FortniteGame\Binaries\Win64\FortniteClient-Win64-Shipping.exe")
            } else if !m.launch_executable.is_empty() {
                root.join(m.launch_executable.replace('/', "\\"))
            } else {
                main_exe(&root)?
            };
            Some(game(m.display_name, &exe, GameSource::Epic, None))
        })
        .collect()
}

fn known_games() -> Vec<Game> {
    let mut out = Vec::new();
    let valorant = PathBuf::from(
        r"C:\Riot Games\VALORANT\live\ShooterGame\Binaries\Win64\VALORANT-Win64-Shipping.exe",
    );
    out.push(Game {
        name: "VALORANT".into(),
        exe: "VALORANT-Win64-Shipping.exe".into(),
        path: valorant.exists().then(|| valorant.display().to_string()),
        source: GameSource::Known,
        note: Some("Uses NVIDIA Reflex: turn on Reflex + Boost in its video settings.".into()),
        high_priority: false,
        fso_disabled: false,
    });
    out.push(Game {
        name: "Minecraft: Java Edition".into(),
        exe: "javaw.exe".into(),
        path: None,
        source: GameSource::Known,
        note: Some(
            "javaw.exe is shared by every Java app, so CPU priority applies to all of them.".into(),
        ),
        high_priority: false,
        fso_disabled: false,
    });
    out.push(Game {
        name: "Minecraft (Bedrock)".into(),
        exe: "Minecraft.Windows.exe".into(),
        path: None,
        source: GameSource::Known,
        note: None,
        high_priority: false,
        fso_disabled: false,
    });
    out
}

fn game(name: String, exe: &Path, source: GameSource, note: Option<String>) -> Game {
    Game {
        name,
        exe: exe
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        path: Some(exe.display().to_string()),
        source,
        note,
        high_priority: false,
        fso_disabled: false,
    }
}

/// Every game found, with each one's per-game tweak state filled in.
pub fn list(ctx: &SysCtx) -> SysResult<Vec<Game>> {
    let mut games = known_games();
    for g in epic_games().into_iter().chain(steam_games(ctx)?) {
        if !games.iter().any(|k| k.exe.eq_ignore_ascii_case(&g.exe)) {
            games.push(g);
        }
    }
    for g in &mut games {
        g.high_priority = priority_atom(&g.exe).is_applied(ctx)?;
        g.fso_disabled = match &g.path {
            Some(p) => fso_flag_set(ctx, p)?,
            None => false,
        };
    }
    Ok(games)
}

// ---- Per-game tweaks --------------------------------------------------------------

pub fn priority_atom(exe: &str) -> Atom {
    Atom::reg_dword(
        Hive::LocalMachine,
        &format!(r"{IFEO}\{exe}\PerfOptions"),
        "CpuPriorityClass",
        HIGH_PRIORITY,
    )
}

pub fn priority_action(exe: &str) -> String {
    format!("game.priority.{}", exe.to_ascii_lowercase())
}

fn fso_flag_set(ctx: &SysCtx, path: &str) -> SysResult<bool> {
    Ok(match ctx.registry.get(Hive::CurrentUser, LAYERS, path)? {
        Some(RegData::Sz(flags)) => flags
            .split_whitespace()
            .any(|f| f.eq_ignore_ascii_case(FSO_FLAG)),
        _ => false,
    })
}

/// Adds the fullscreen-optimizations flag to a compatibility-layers string,
/// keeping any flags (DPI, run as admin…) already there.
pub fn merge_layers(current: Option<&str>) -> String {
    let mut flags: Vec<&str> = current
        .unwrap_or("")
        .split_whitespace()
        .filter(|f| *f != "~")
        .collect();
    if !flags.iter().any(|f| f.eq_ignore_ascii_case(FSO_FLAG)) {
        flags.push(FSO_FLAG);
    }
    format!("~ {}", flags.join(" "))
}

pub fn fso_atom(ctx: &SysCtx, path: &str) -> SysResult<Atom> {
    let current = match ctx.registry.get(Hive::CurrentUser, LAYERS, path)? {
        Some(RegData::Sz(s)) => Some(s),
        _ => None,
    };
    Ok(Atom::reg_sz(
        Hive::CurrentUser,
        LAYERS,
        path,
        &merge_layers(current.as_deref()),
    ))
}

pub fn fso_action(path: &str) -> String {
    format!("game.fso.{}", path.to_ascii_lowercase())
}

// ---- Minecraft Java JVM helper -----------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Collector {
    G1,
    Zgc,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JvmAdvice {
    pub memory_gb: u32,
    pub arguments: String,
    pub notes: Vec<String>,
}

/// Heap size for Minecraft: enough for the pack, never more than half the PC's RAM.
pub fn recommended_heap_gb(total_ram_bytes: u64, modded: bool) -> u32 {
    let total_gb = (total_ram_bytes as f64 / (1u64 << 30) as f64).round() as u32;
    let want = match (total_gb, modded) {
        (0..=8, false) => 3,
        (0..=8, true) => 4,
        (9..=16, false) => 4,
        (9..=16, true) => 6,
        (17..=32, false) => 4,
        (17..=32, true) => 8,
        (_, false) => 6,
        (_, true) => 10,
    };
    want.min((total_gb / 2).max(2))
}

pub fn jvm_advice(total_ram_bytes: u64, modded: bool, collector: Collector) -> JvmAdvice {
    let gb = recommended_heap_gb(total_ram_bytes, modded);
    let heap = format!("-Xms{gb}G -Xmx{gb}G");
    let (flags, mut notes) = match collector {
        Collector::G1 => (
            "-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=200 -XX:+UnlockExperimentalVMOptions \
             -XX:+DisableExplicitGC -XX:+AlwaysPreTouch -XX:G1NewSizePercent=30 -XX:G1MaxNewSizePercent=40 \
             -XX:G1HeapRegionSize=8M -XX:G1ReservePercent=20 -XX:G1HeapWastePercent=5 -XX:G1MixedGCCountTarget=4 \
             -XX:InitiatingHeapOccupancyPercent=15 -XX:G1MixedGCLiveThresholdPercent=90 \
             -XX:G1RSetUpdatingPauseTimePercent=5 -XX:SurvivorRatio=32 -XX:+PerfDisableSharedMem \
             -XX:MaxTenuringThreshold=1",
            vec!["G1 with Aikar's tuning: the dependable choice for 4–12 GB.".to_owned()],
        ),
        Collector::Zgc => (
            "-XX:+UseZGC -XX:+ZGenerational -XX:+AlwaysPreTouch -XX:+DisableExplicitGC",
            vec![
                "Generational ZGC keeps garbage-collection pauses under a millisecond, using roughly 10% more memory."
                    .to_owned(),
                "Needs Java 21 or newer (Minecraft 1.20.5+ ships with it).".to_owned(),
            ],
        ),
    };
    notes.push(format!(
        "{gb} GB is plenty for {}. Giving Minecraft more memory than it needs makes pauses longer, not shorter.",
        if modded { "most modpacks" } else { "vanilla and light mods" }
    ));
    notes.push(
        "Paste these into the launcher's Installations → Edit → More options → JVM arguments."
            .to_owned(),
    );
    JvmAdvice {
        memory_gb: gb,
        arguments: format!("{heap} {flags}")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::fake::FakeSystem;

    const GB: u64 = 1 << 30;

    #[test]
    fn parses_library_folders_and_manifests() {
        let lib = r#""libraryfolders" { "0" { "path" "C:\\Program Files (x86)\\Steam" "apps" { "730" "1" } } "1" { "path" "D:\\SteamLibrary" } }"#;
        assert_eq!(
            vdf_values(lib, "path"),
            vec![r"C:\Program Files (x86)\Steam", r"D:\SteamLibrary"]
        );
        let acf = r#""AppState" { "appid" "730" "name" "Counter-Strike 2" "installdir" "Counter-Strike Global Offensive" }"#;
        assert_eq!(vdf_values(acf, "name"), vec!["Counter-Strike 2"]);
        assert_eq!(
            vdf_values(acf, "installdir"),
            vec!["Counter-Strike Global Offensive"]
        );
    }

    #[test]
    fn picks_the_shipping_exe_over_a_bigger_launcher() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("Game/Binaries/Win64");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(dir.path().join("Launcher.exe"), vec![0u8; 5000]).unwrap();
        std::fs::write(dir.path().join("unins000.exe"), vec![0u8; 9000]).unwrap();
        std::fs::write(bin.join("Game-Win64-Shipping.exe"), vec![0u8; 100]).unwrap();
        assert_eq!(
            main_exe(dir.path()).unwrap().file_name().unwrap(),
            "Game-Win64-Shipping.exe"
        );
    }

    #[test]
    fn layers_merge_keeps_existing_flags() {
        assert_eq!(merge_layers(None), "~ DISABLEDXMAXIMIZEDWINDOWEDMODE");
        assert_eq!(
            merge_layers(Some("~ HIGHDPIAWARE")),
            "~ HIGHDPIAWARE DISABLEDXMAXIMIZEDWINDOWEDMODE"
        );
        assert_eq!(
            merge_layers(Some("~ DISABLEDXMAXIMIZEDWINDOWEDMODE")),
            "~ DISABLEDXMAXIMIZEDWINDOWEDMODE"
        );
    }

    #[test]
    fn priority_is_guarded_for_anti_cheat_launchers_only() {
        crate::guard::check_atom(&priority_atom("VALORANT-Win64-Shipping.exe")).unwrap();
        crate::guard::check_atom(&priority_atom("javaw.exe")).unwrap();
        assert!(crate::guard::check_atom(&priority_atom("vgc.exe")).is_err());
        assert!(
            crate::guard::check_atom(&priority_atom("FortniteClient-Win64-Shipping_EAC.exe"))
                .is_err()
        );
    }

    #[test]
    fn known_games_report_their_tweak_state() {
        let sys = FakeSystem::new();
        sys.registry.seed(
            Hive::LocalMachine,
            &format!(r"{IFEO}\javaw.exe\PerfOptions"),
            "CpuPriorityClass",
            RegData::Dword(3),
        );
        let games = list(&sys.ctx).unwrap();
        assert!(
            games
                .iter()
                .find(|g| g.exe == "javaw.exe")
                .unwrap()
                .high_priority
        );
        assert!(
            !games
                .iter()
                .find(|g| g.exe == "Minecraft.Windows.exe")
                .unwrap()
                .high_priority
        );
    }

    #[test]
    fn heap_sizes_scale_with_ram_and_never_exceed_half() {
        assert_eq!(recommended_heap_gb(8 * GB, false), 3);
        assert_eq!(recommended_heap_gb(16 * GB, true), 6);
        assert_eq!(recommended_heap_gb(32 * GB, true), 8);
        assert_eq!(recommended_heap_gb(64 * GB, true), 10);
        assert_eq!(recommended_heap_gb(6 * GB, true), 3, "half of 6 GB");
        let zgc = jvm_advice(32 * GB, false, Collector::Zgc);
        assert!(zgc.arguments.starts_with("-Xms4G -Xmx4G -XX:+UseZGC"));
        assert!(
            !jvm_advice(16 * GB, false, Collector::G1)
                .arguments
                .contains("  ")
        );
    }
}
