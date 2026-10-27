//! NVIDIA driver profiles: the global "Apex Tune Low Latency" profile,
//! per-game profiles, frame caps, and full driver-settings backups.
//!
//! Setting IDs and values come from NVIDIA's `NvApiDriverSettings.h`.

use serde::Serialize;

use crate::sys::{NvProfile, SysCtx, SysResult};
use crate::tweaks::atom::Atom;
use crate::tweaks::{Applicability, Category, Restart, Risk, Tweak, TweakMeta};

pub const PRERENDER_LIMIT: u32 = 0x007B_A09E;
pub const POWER_MANAGEMENT: u32 = 0x1057_EB71;
pub const TEXTURE_QUALITY: u32 = 0x00CE_2691;
pub const OGL_THREADED: u32 = 0x20C1_221E;
pub const SHADER_CACHE_SIZE: u32 = 0x00AC_8497;
pub const FRAME_RATE_LIMIT: u32 = 0x1083_5002;

const LOW_LATENCY_ON: u32 = 1;
const PREFER_MAX_PERFORMANCE: u32 = 1;
const TEXTURE_HIGH_PERFORMANCE: u32 = 0x14;
const THREADED_ON: u32 = 1;
/// In MB.
const SHADER_CACHE_10_GB: u32 = 10 * 1024;

#[derive(Debug, Clone, Copy)]
struct Spec {
    id: u32,
    label: &'static str,
    value: u32,
    value_label: &'static str,
}

const LOW_LATENCY: Spec = Spec {
    id: PRERENDER_LIMIT,
    label: "Low Latency Mode",
    value: LOW_LATENCY_ON,
    value_label: "On (1 pre-rendered frame)",
};
const POWER: Spec = Spec {
    id: POWER_MANAGEMENT,
    label: "Power management mode",
    value: PREFER_MAX_PERFORMANCE,
    value_label: "Prefer maximum performance",
};
const TEXTURE: Spec = Spec {
    id: TEXTURE_QUALITY,
    label: "Texture filtering quality",
    value: TEXTURE_HIGH_PERFORMANCE,
    value_label: "High performance",
};
const THREADED: Spec = Spec {
    id: OGL_THREADED,
    label: "Threaded optimization",
    value: THREADED_ON,
    value_label: "On",
};
const SHADER_CACHE: Spec = Spec {
    id: SHADER_CACHE_SIZE,
    label: "Shader cache size",
    value: SHADER_CACHE_10_GB,
    value_label: "10 GB",
};

fn atom(profile: &NvProfile, spec: Spec) -> Atom {
    Atom::NvSetting {
        profile: profile.clone(),
        setting: spec.id,
        label: spec.label.into(),
        value: spec.value,
        value_label: spec.value_label.into(),
    }
}

/// "Apex Tune Low Latency" on the global profile, which every game inherits.
pub struct GlobalProfile {
    meta: TweakMeta,
}

impl GlobalProfile {
    pub fn new() -> Self {
        Self {
            meta: TweakMeta {
                id: "nvidia_global",
                name: "NVIDIA low-latency defaults",
                description: "Sets the driver's global profile: Low Latency Mode on, Prefer maximum performance (no \
                              GPU downclocking mid-match), high-performance texture filtering, threaded optimization \
                              and a 10 GB shader cache. Games with NVIDIA Reflex ignore Low Latency Mode; use Reflex \
                              in their settings instead.",
                category: Category::Nvidia,
                risk: Risk::Safe,
                restart: Restart::None,
                recommended: true,
            },
        }
    }
}

impl Default for GlobalProfile {
    fn default() -> Self {
        Self::new()
    }
}

impl Tweak for GlobalProfile {
    fn meta(&self) -> &TweakMeta {
        &self.meta
    }

    fn applicable(&self, ctx: &SysCtx) -> SysResult<Applicability> {
        Ok(match ctx.nvidia.info()? {
            Some(_) => Applicability::Applicable,
            None => Applicability::NotApplicable("No NVIDIA GPU found.".into()),
        })
    }

    fn apply(&self, txn: &mut crate::tweaks::Txn<'_>) -> Result<(), crate::tweaks::TweakError> {
        ensure_backup(txn.ctx(), txn.session())?;
        for atom in self.atoms(txn.ctx())? {
            txn.write(atom)?;
        }
        Ok(())
    }

    fn atoms(&self, _ctx: &SysCtx) -> SysResult<Vec<Atom>> {
        let g = NvProfile::Global;
        Ok([LOW_LATENCY, POWER, TEXTURE, THREADED, SHADER_CACHE]
            .into_iter()
            .map(|s| atom(&g, s))
            .collect())
    }
}

/// Games whose Reflex makes the driver's Low Latency Mode irrelevant.
const REFLEX_GAMES: &[&str] = &[
    "valorant-win64-shipping.exe",
    "fortniteclient-win64-shipping.exe",
    "cs2.exe",
    "r5apex.exe",
];
/// OpenGL games where threaded optimization matters.
const OPENGL_GAMES: &[&str] = &["javaw.exe"];

pub fn has_reflex(exe: &str) -> bool {
    REFLEX_GAMES.contains(&exe.to_ascii_lowercase().as_str())
}

/// A low-latency profile for one game, tailored to whether it has Reflex
/// and whether it's OpenGL.
pub fn game_atoms(exe: &str) -> Vec<Atom> {
    let p = NvProfile::App { exe: exe.into() };
    let lower = exe.to_ascii_lowercase();
    let mut atoms = vec![
        Atom::NvAppProfile { exe: exe.into() },
        atom(&p, POWER),
        atom(&p, TEXTURE),
    ];
    if !has_reflex(&lower) {
        atoms.push(atom(&p, LOW_LATENCY));
    }
    if OPENGL_GAMES.contains(&lower.as_str()) {
        atoms.push(atom(&p, THREADED));
    }
    atoms
}

pub fn game_action(exe: &str) -> String {
    format!("nvidia.game.{}", exe.to_ascii_lowercase())
}

pub fn fps_atoms(exe: &str, fps: u32) -> Vec<Atom> {
    let p = NvProfile::App { exe: exe.into() };
    vec![
        Atom::NvAppProfile { exe: exe.into() },
        Atom::NvSetting {
            profile: p,
            setting: FRAME_RATE_LIMIT,
            label: "Max frame rate".into(),
            value: fps.min(1023),
            value_label: format!("{} FPS", fps.min(1023)),
        },
    ]
}

pub fn fps_action(exe: &str) -> String {
    format!("nvidia.fps.{}", exe.to_ascii_lowercase())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameProfile {
    pub name: String,
    pub exe: String,
    pub reflex: bool,
    pub applied: bool,
    pub fps_cap: Option<u32>,
}

pub fn game_profiles(ctx: &SysCtx, games: &[crate::games::Game]) -> SysResult<Vec<GameProfile>> {
    let mut out = Vec::new();
    for g in games {
        let mut applied = true;
        for a in game_atoms(&g.exe) {
            applied &= a.is_applied(ctx)?;
        }
        let fps_cap = if ctx.nvidia.app_profile_exists(&g.exe)? {
            ctx.nvidia
                .get(&NvProfile::App { exe: g.exe.clone() }, FRAME_RATE_LIMIT)?
                .filter(|v| *v > 0)
        } else {
            None
        };
        out.push(GameProfile {
            name: g.name.clone(),
            exe: g.exe.clone(),
            reflex: has_reflex(&g.exe),
            applied,
            fps_cap,
        });
    }
    Ok(out)
}

/// Exports the whole driver settings database once per session, before the
/// first NVIDIA change, so everything can be put back in one step.
pub fn ensure_backup(ctx: &SysCtx, session: uuid::Uuid) -> SysResult<std::path::PathBuf> {
    let path = crate::paths::backups_dir().join(format!("nvidia-{}.nvdrs", session.simple()));
    if !path.exists() {
        ctx.nvidia.export(&path)?;
        tracing::info!(target: crate::log::ACTIVITY_TARGET, tweak = "nvidia", "Backed up all NVIDIA driver settings");
    }
    Ok(path)
}

/// The newest full backup, if any.
pub fn latest_backup() -> Option<std::path::PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(crate::paths::backups_dir())
        .ok()?
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".nvdrs"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    files.sort();
    files.pop().map(|(_, p)| p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::journal::Journal;
    use crate::sys::fake::FakeSystem;
    use crate::tweaks::{Engine, TweakState};

    fn engine(sys: &FakeSystem, dir: &std::path::Path) -> Engine {
        Engine::new(sys.ctx.clone(), Journal::open(dir).unwrap(), vec![])
    }

    #[test]
    fn not_applicable_without_nvidia() {
        let sys = FakeSystem::new();
        assert!(matches!(
            GlobalProfile::new().detect(&sys.ctx).unwrap(),
            TweakState::NotApplicable(_)
        ));
    }

    #[test]
    fn global_profile_round_trips_to_driver_defaults() {
        let sys = FakeSystem::new();
        *sys.nvidia.present.lock().unwrap() = true;
        sys.ctx
            .nvidia
            .set(&NvProfile::Global, POWER_MANAGEMENT, 5)
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let mut engine = engine(&sys, dir.path());
        let tweak = GlobalProfile::new();
        crate::paths::use_test_data_dir();
        engine.apply(&tweak).unwrap();
        assert_eq!(
            sys.nvidia.exports.lock().unwrap().len(),
            1,
            "full backup before the first change"
        );
        assert_eq!(
            sys.ctx
                .nvidia
                .get(&NvProfile::Global, PRERENDER_LIMIT)
                .unwrap(),
            Some(1)
        );
        engine.revert(&tweak).unwrap();
        assert_eq!(
            sys.ctx
                .nvidia
                .get(&NvProfile::Global, PRERENDER_LIMIT)
                .unwrap(),
            None,
            "back to driver default"
        );
        assert_eq!(
            sys.ctx
                .nvidia
                .get(&NvProfile::Global, POWER_MANAGEMENT)
                .unwrap(),
            Some(5),
            "user value restored"
        );
    }

    #[test]
    fn game_profiles_skip_low_latency_for_reflex_and_remove_created_profiles() {
        let sys = FakeSystem::new();
        *sys.nvidia.present.lock().unwrap() = true;
        let dir = tempfile::tempdir().unwrap();
        let mut engine = engine(&sys, dir.path());

        let valorant = game_atoms("VALORANT-Win64-Shipping.exe");
        assert!(!valorant.iter().any(|a| matches!(
            a,
            Atom::NvSetting {
                setting: PRERENDER_LIMIT,
                ..
            }
        )));
        let java = game_atoms("javaw.exe");
        assert!(java.iter().any(|a| matches!(
            a,
            Atom::NvSetting {
                setting: OGL_THREADED,
                ..
            }
        )));

        engine
            .apply_action(&game_action("javaw.exe"), "Minecraft profile", java)
            .unwrap();
        assert!(sys.ctx.nvidia.app_profile_exists("javaw.exe").unwrap());
        engine.revert_action(&game_action("javaw.exe")).unwrap();
        assert!(
            !sys.ctx.nvidia.app_profile_exists("javaw.exe").unwrap(),
            "profile Apex created is removed"
        );
    }

    #[test]
    fn anti_cheat_executables_never_get_profiles() {
        for a in game_atoms("EasyAntiCheat_EOS.exe") {
            assert!(crate::guard::check_atom(&a).is_err());
        }
    }

    #[test]
    fn backup_happens_once_per_session() {
        let sys = FakeSystem::new();
        *sys.nvidia.present.lock().unwrap() = true;
        crate::paths::use_test_data_dir();
        let session = uuid::Uuid::new_v4();
        ensure_backup(&sys.ctx, session).unwrap();
        ensure_backup(&sys.ctx, session).unwrap();
        assert_eq!(sys.nvidia.exports.lock().unwrap().len(), 1);
        assert!(latest_backup().is_some());
    }
}
