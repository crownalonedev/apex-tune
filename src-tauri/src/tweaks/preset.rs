//! Tweak overview, the Optimization Score, and the "Optimize for Gaming" batch.

use serde::Serialize;

use super::engine::ApplyOutcome;
use super::{ChangePreview, Engine, Restart, Risk, TweakError, TweakMeta, TweakState};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TweakInfo {
    #[serde(flatten)]
    pub meta: TweakMeta,
    pub state: TweakState,
    /// Apex Tune has a backup for it, so it can be reverted.
    pub revertible: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Score {
    /// 0–100, or `None` when there's nothing to score yet.
    pub value: Option<u8>,
    pub counted: usize,
    pub applied: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub tweaks: Vec<TweakInfo>,
    pub score: Score,
}

/// Preset tweaks count fully; other Moderate tweaks count half; Advanced
/// tweaks don't count, so skipping them never lowers the score.
fn weight(meta: &TweakMeta) -> f64 {
    match (meta.recommended, meta.risk) {
        (true, _) => 1.0,
        (false, Risk::Moderate) => 0.5,
        _ => 0.0,
    }
}

pub fn score<'a>(items: impl IntoIterator<Item = (&'a TweakMeta, &'a TweakState)>) -> Score {
    let (mut total, mut got, mut counted, mut applied) = (0.0, 0.0, 0, 0);
    for (meta, state) in items {
        let w = weight(meta);
        let credit = match state {
            TweakState::Applied => 1.0,
            TweakState::Partial => 0.5,
            TweakState::NotApplied => 0.0,
            TweakState::NotApplicable(_) | TweakState::Unknown(_) => continue,
        };
        if w == 0.0 {
            continue;
        }
        total += w;
        got += w * credit;
        counted += 1;
        if *state == TweakState::Applied {
            applied += 1;
        }
    }
    Score {
        value: (total > 0.0).then(|| (100.0 * got / total).round() as u8),
        counted,
        applied,
    }
}

pub fn overview(engine: &Engine) -> Overview {
    let tweaks: Vec<TweakInfo> = engine
        .catalog()
        .iter()
        .map(|t| TweakInfo {
            meta: t.meta().clone(),
            state: t
                .detect(engine.ctx())
                .unwrap_or_else(|e| TweakState::Unknown(e.to_string())),
            revertible: engine.journal().latest_applied(t.id()).is_some(),
        })
        .collect();
    let score = score(tweaks.iter().map(|t| (&t.meta, &t.state)));
    Overview { tweaks, score }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetItem {
    #[serde(flatten)]
    pub meta: TweakMeta,
    pub state: TweakState,
    pub changes: Vec<ChangePreview>,
}

/// Recommended tweaks that aren't fully applied, with their exact changes.
pub fn preview(engine: &Engine) -> Result<Vec<PresetItem>, TweakError> {
    let mut items = Vec::new();
    for tweak in engine.catalog().iter().filter(|t| t.meta().recommended) {
        let state = tweak.detect(engine.ctx())?;
        if matches!(state, TweakState::Applied | TweakState::NotApplicable(_)) {
            continue;
        }
        items.push(PresetItem {
            meta: tweak.meta().clone(),
            state,
            changes: tweak.plan(engine.ctx())?,
        });
    }
    Ok(items)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BatchStatus {
    Applied,
    AlreadyApplied,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchResult {
    pub id: String,
    pub name: String,
    pub status: BatchStatus,
    pub error_kind: Option<&'static str>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchReport {
    pub results: Vec<BatchResult>,
    /// The strongest restart any applied tweak needs.
    pub restart: Restart,
}

/// Applies tweaks one at a time, reporting each result as it lands. A
/// failure doesn't stop the rest (it was rolled back), except a missing
/// restore point, which stops the batch before anything changes.
pub fn apply_batch(
    engine: &mut Engine,
    ids: &[String],
    mut progress: impl FnMut(&BatchResult),
) -> Result<BatchReport, TweakError> {
    engine.ensure_restore_point()?;
    let mut results = Vec::new();
    let mut restart = Restart::None;

    for id in ids {
        let Some(tweak) = engine.tweak(id) else {
            let result = BatchResult {
                id: id.clone(),
                name: id.clone(),
                status: BatchStatus::Failed,
                error_kind: Some("notApplicable"),
                error: Some("Unknown tweak.".into()),
            };
            progress(&result);
            results.push(result);
            continue;
        };
        let result = match engine.apply(&*tweak) {
            Ok(outcome) => {
                if outcome == ApplyOutcome::Applied {
                    restart = restart.max(tweak.requires_restart());
                }
                BatchResult {
                    id: id.clone(),
                    name: tweak.name().into(),
                    status: match outcome {
                        ApplyOutcome::Applied => BatchStatus::Applied,
                        ApplyOutcome::AlreadyApplied => BatchStatus::AlreadyApplied,
                    },
                    error_kind: None,
                    error: None,
                }
            }
            Err(e) => BatchResult {
                id: id.clone(),
                name: tweak.name().into(),
                status: BatchStatus::Failed,
                error_kind: Some(e.kind()),
                error: Some(e.to_string()),
            },
        };
        progress(&result);
        results.push(result);
    }
    Ok(BatchReport { results, restart })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::journal::Journal;
    use crate::sys::fake::FakeSystem;
    use crate::sys::{Hive, RegData};
    use crate::tweaks::test_support::*;
    use crate::tweaks::{Atom, Category, StaticTweak};
    use std::sync::Arc;

    fn m(recommended: bool, risk: Risk) -> TweakMeta {
        TweakMeta {
            id: "x",
            name: "x",
            description: "",
            category: Category::Gaming,
            risk,
            restart: Restart::None,
            recommended,
        }
    }

    #[test]
    fn score_weights_and_exclusions() {
        let safe = m(true, Risk::Safe);
        let moderate = m(false, Risk::Moderate);
        let advanced = m(false, Risk::Advanced);
        let na = TweakState::NotApplicable("Home".into());
        let items = [
            (&safe, &TweakState::Applied),
            (&safe, &TweakState::NotApplied),
            (&moderate, &TweakState::Applied),
            (&advanced, &TweakState::NotApplied),
            (&safe, &na),
        ];
        // (1 + 0 + 0.5) / (1 + 1 + 0.5) = 60%
        assert_eq!(
            score(items),
            Score {
                value: Some(60),
                counted: 3,
                applied: 2
            }
        );
        assert_eq!(score([(&safe, &TweakState::Partial)]).value, Some(50));
        assert_eq!(score([(&advanced, &TweakState::NotApplied)]).value, None);
    }

    fn engine_with(sys: &FakeSystem, dir: &std::path::Path) -> Engine {
        let failing = StaticTweak {
            refresh: crate::tweaks::Refresh::Nothing,
            meta: meta("failing"),
            atoms: || {
                vec![Atom::reg_dword(
                    Hive::LocalMachine,
                    r"Software\Denied",
                    "X",
                    1,
                )]
            },
        };
        let mut restart = two_values();
        restart.meta.restart = Restart::SignOut;
        Engine::new(
            sys.ctx.clone(),
            Journal::open(dir).unwrap(),
            vec![Arc::new(restart), Arc::new(failing)],
        )
    }

    #[test]
    fn preview_lists_unapplied_recommended_tweaks_with_changes() {
        let sys = FakeSystem::new();
        let dir = tempfile::tempdir().unwrap();
        sys.registry
            .seed(Hive::CurrentUser, PATH, "A", RegData::Dword(1));
        let engine = engine_with(&sys, dir.path());
        let items = preview(&engine).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].state, TweakState::Partial);
        assert_eq!(items[0].changes.len(), 1, "only B still needs changing");
    }

    #[test]
    fn batch_continues_past_failures_and_reports_restart() {
        let sys = FakeSystem::new();
        let dir = tempfile::tempdir().unwrap();
        sys.registry.fail_writes_to(r"software\denied");
        let mut engine = engine_with(&sys, dir.path());
        let mut seen = Vec::new();
        let ids = [
            "failing".to_string(),
            "two_values".to_string(),
            "nope".to_string(),
        ];
        let report = apply_batch(&mut engine, &ids, |r| seen.push(r.id.clone())).unwrap();

        assert_eq!(seen, ids);
        let statuses: Vec<_> = report.results.iter().map(|r| r.status).collect();
        assert_eq!(
            statuses,
            [
                BatchStatus::Failed,
                BatchStatus::Applied,
                BatchStatus::Failed
            ]
        );
        assert_eq!(report.results[0].error_kind, Some("rolledBack"));
        assert_eq!(report.restart, Restart::SignOut);
        assert_eq!(overview(&engine).score.value, Some(50));
    }

    #[test]
    fn batch_stops_before_any_change_without_a_restore_point() {
        let sys = FakeSystem::new();
        *sys.restore_points.enabled.lock().unwrap() = Some(false);
        let dir = tempfile::tempdir().unwrap();
        let mut engine = engine_with(&sys, dir.path());
        let err = apply_batch(&mut engine, &["two_values".into()], |_| {}).unwrap_err();
        assert_eq!(err.kind(), "restorePointRequired");
        assert_eq!(sys.registry.write_count(), 0);
    }
}
