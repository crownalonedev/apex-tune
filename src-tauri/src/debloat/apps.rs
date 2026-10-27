//! Which preinstalled apps can be removed, and which must stay.

use serde::Serialize;

use crate::sys::{AppPackage, SysCtx, SysResult};
use crate::tweaks::atom::Atom;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AppGroup {
    /// Safe to remove; nothing else depends on it.
    Removable,
    /// Safe to remove, but some people use it.
    Optional,
    /// Needed by games or Windows; never removed.
    Keep,
    /// Not reviewed; shown but not removable.
    Other,
}

struct Known {
    name: &'static str,
    group: AppGroup,
    note: &'static str,
}

const fn app(name: &'static str, group: AppGroup, note: &'static str) -> Known {
    Known { name, group, note }
}

use AppGroup::{Keep, Optional, Removable};

const KNOWN: &[Known] = &[
    app("Clipchamp.Clipchamp", Removable, "Video editor."),
    app(
        "Microsoft.BingNews",
        Removable,
        "News app; also feeds Widgets.",
    ),
    app("Microsoft.BingWeather", Removable, "Weather app."),
    app(
        "Microsoft.MicrosoftSolitaireCollection",
        Removable,
        "Card games with ads.",
    ),
    app("MSTeams", Removable, "Consumer Teams chat."),
    app(
        "MicrosoftTeams",
        Removable,
        "Consumer Teams chat (older version).",
    ),
    app("Microsoft.GetHelp", Removable, "Get Help support app."),
    app("Microsoft.Getstarted", Removable, "Tips app."),
    app(
        "Microsoft.MicrosoftOfficeHub",
        Removable,
        "Microsoft 365 promotion app.",
    ),
    app(
        "Microsoft.PowerAutomateDesktop",
        Removable,
        "Automation tool most people never open.",
    ),
    app("Microsoft.Todos", Removable, "Microsoft To Do."),
    app("Microsoft.WindowsFeedbackHub", Removable, "Feedback Hub."),
    app("Microsoft.WindowsMaps", Removable, "Maps."),
    app("Microsoft.ZuneVideo", Removable, "Movies & TV."),
    app("Microsoft.WindowsAlarms", Removable, "Clock and alarms."),
    app(
        "Microsoft.OutlookForWindows",
        Removable,
        "New Outlook (mail).",
    ),
    app("Microsoft.Copilot", Removable, "Copilot app."),
    app(
        "Microsoft.549981C3F5F10",
        Removable,
        "Cortana (older builds).",
    ),
    app("Microsoft.People", Removable, "People (older builds)."),
    app("Microsoft.Windows.DevHome", Removable, "Dev Home."),
    app(
        "Microsoft.MixedReality.Portal",
        Removable,
        "Mixed Reality Portal (older builds).",
    ),
    app("Microsoft.SkypeApp", Removable, "Skype."),
    app(
        "Microsoft.Microsoft3DViewer",
        Removable,
        "3D Viewer (older builds).",
    ),
    app(
        "Microsoft.ZuneMusic",
        Optional,
        "Media Player. Remove only if you use another player.",
    ),
    app(
        "Microsoft.YourPhone",
        Optional,
        "Phone Link. Keep it if you use your phone with this PC.",
    ),
    app("Microsoft.MicrosoftStickyNotes", Optional, "Sticky Notes."),
    app(
        "Microsoft.WindowsSoundRecorder",
        Optional,
        "Sound Recorder.",
    ),
    app(
        "MicrosoftCorporationII.QuickAssist",
        Optional,
        "Quick Assist remote help. Removing it also removes a common scam route.",
    ),
    app(
        "Microsoft.WindowsStore",
        Keep,
        "Microsoft Store: installs and updates apps and games.",
    ),
    app("Microsoft.StorePurchaseApp", Keep, "Store purchases."),
    app(
        "Microsoft.DesktopAppInstaller",
        Keep,
        "App Installer and winget.",
    ),
    app(
        "Microsoft.GamingServices",
        Keep,
        "Gaming Services: needed by Game Pass and Minecraft.",
    ),
    app(
        "Microsoft.GamingApp",
        Keep,
        "Xbox app: Game Pass and the Minecraft Launcher.",
    ),
    app(
        "Microsoft.XboxIdentityProvider",
        Keep,
        "Xbox sign-in, used by Minecraft Bedrock.",
    ),
    app(
        "Microsoft.Xbox.TCUI",
        Keep,
        "Xbox sign-in screens used by games.",
    ),
    app("Microsoft.XboxGamingOverlay", Keep, "Game Bar."),
    app(
        "Microsoft.XboxSpeechToTextOverlay",
        Keep,
        "Game Bar accessibility.",
    ),
    app("Microsoft.SecHealthUI", Keep, "Windows Security."),
    app(
        "NVIDIACorp.NVIDIAControlPanel",
        Keep,
        "NVIDIA Control Panel.",
    ),
    app("Microsoft.MinecraftUWP", Keep, "Minecraft Bedrock."),
    app("Microsoft.4297127D64EC6", Keep, "Minecraft Launcher."),
    app("Microsoft.WindowsCalculator", Keep, "Calculator."),
    app("Microsoft.WindowsNotepad", Keep, "Notepad."),
    app("Microsoft.Windows.Photos", Keep, "Photos."),
    app("Microsoft.ScreenSketch", Keep, "Snipping Tool."),
    app("Microsoft.Paint", Keep, "Paint."),
    app("Microsoft.WindowsTerminal", Keep, "Terminal."),
];

fn known(name: &str) -> Option<&'static Known> {
    KNOWN.iter().find(|k| k.name.eq_ignore_ascii_case(name))
}

pub fn is_removable(name: &str) -> bool {
    known(name).is_some_and(|k| matches!(k.group, Removable | Optional))
}

pub fn action_id(name: &str) -> String {
    format!("appx.{name}")
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppRow {
    pub name: String,
    pub display_name: String,
    pub group: AppGroup,
    pub note: String,
    pub installed: bool,
    /// Apex Tune removed it and can bring it back.
    pub restorable: bool,
}

/// Installed apps plus ones Apex Tune removed, grouped for the Debloat page.
pub fn rows(ctx: &SysCtx, removed_by_apex: &[(String, String)]) -> SysResult<Vec<AppRow>> {
    let installed = ctx.apps.list()?;
    let mut rows: Vec<AppRow> = installed
        .iter()
        .filter(|p| !p.system || known(&p.name).is_some())
        .map(|p| row(p, true, false))
        .collect();
    for (name, display) in removed_by_apex {
        if !rows.iter().any(|r| r.name.eq_ignore_ascii_case(name)) {
            let pkg = AppPackage {
                name: name.clone(),
                family_name: String::new(),
                full_name: String::new(),
                display_name: display.clone(),
                system: false,
            };
            rows.push(row(&pkg, false, true));
        }
    }
    rows.sort_by(|a, b| (a.group as u8, &a.display_name).cmp(&(b.group as u8, &b.display_name)));
    Ok(rows)
}

fn row(p: &AppPackage, installed: bool, restorable: bool) -> AppRow {
    let k = known(&p.name);
    AppRow {
        name: p.name.clone(),
        display_name: if p.display_name.starts_with("ms-resource:") {
            p.name.clone()
        } else {
            p.display_name.clone()
        },
        group: k.map_or(AppGroup::Other, |k| k.group),
        note: k
            .map_or(
                "Not reviewed by Apex Tune, so it can't be removed here.",
                |k| k.note,
            )
            .into(),
        installed,
        restorable,
    }
}

/// The atom that removes one installed app.
pub fn removal(ctx: &SysCtx, name: &str) -> SysResult<Option<Atom>> {
    Ok(ctx
        .apps
        .list()?
        .into_iter()
        .find(|p| p.name.eq_ignore_ascii_case(name))
        .map(|p| Atom::AppRemoved {
            name: p.name,
            full_name: p.full_name,
            display_name: p.display_name,
        }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::journal::Journal;
    use crate::sys::fake::FakeSystem;
    use crate::tweaks::Engine;

    #[test]
    fn keep_list_and_unknown_apps_are_never_removable() {
        assert!(is_removable("Clipchamp.Clipchamp"));
        assert!(is_removable("microsoft.yourphone"));
        assert!(!is_removable("Microsoft.WindowsStore"));
        assert!(!is_removable("Microsoft.GamingServices"));
        assert!(!is_removable("Contoso.Unknown"));
    }

    #[test]
    fn removing_and_restoring_a_staged_app() {
        let sys = FakeSystem::new();
        sys.apps.install("Clipchamp.Clipchamp", true);
        sys.apps.install("Microsoft.WindowsStore", true);
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(sys.ctx.clone(), Journal::open(dir.path()).unwrap(), vec![]);

        let atom = removal(&sys.ctx, "Clipchamp.Clipchamp").unwrap().unwrap();
        engine
            .apply_action(
                &action_id("Clipchamp.Clipchamp"),
                "Removed Clipchamp",
                vec![atom],
            )
            .unwrap();
        assert!(!sys.apps.has("Clipchamp.Clipchamp"));

        engine
            .revert_action(&action_id("Clipchamp.Clipchamp"))
            .unwrap();
        assert!(sys.apps.has("Clipchamp.Clipchamp"));
    }

    #[test]
    fn guard_blocks_removing_the_store() {
        let sys = FakeSystem::new();
        sys.apps.install("Microsoft.WindowsStore", true);
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(sys.ctx.clone(), Journal::open(dir.path()).unwrap(), vec![]);
        let atom = removal(&sys.ctx, "Microsoft.WindowsStore")
            .unwrap()
            .unwrap();
        assert_eq!(
            engine
                .apply_action("appx.store", "x", vec![atom])
                .unwrap_err()
                .kind(),
            "refused"
        );
        assert!(sys.apps.has("Microsoft.WindowsStore"));
    }

    #[test]
    fn rows_group_and_include_restorable_removals() {
        let sys = FakeSystem::new();
        sys.apps.install("Microsoft.BingNews", true);
        sys.apps.install("Contoso.Thing", false);
        let rows = rows(
            &sys.ctx,
            &[("Clipchamp.Clipchamp".into(), "Clipchamp".into())],
        )
        .unwrap();
        assert_eq!(rows.len(), 3);
        assert!(
            rows.iter()
                .any(|r| r.name == "Clipchamp.Clipchamp" && r.restorable && !r.installed)
        );
        assert_eq!(
            rows.iter()
                .find(|r| r.name == "Contoso.Thing")
                .unwrap()
                .group,
            AppGroup::Other
        );
    }
}
