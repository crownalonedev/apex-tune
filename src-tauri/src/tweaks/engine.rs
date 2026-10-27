//! Applies and reverts tweaks as journaled transactions.
//!
//! Every write goes through [`Txn`]: guard check, capture the current state,
//! save it to the journal, then write. If anything fails, everything the
//! transaction wrote is restored in reverse order before the error is
//! returned, so a tweak is never left half-applied.

use std::sync::Arc;

use serde::Serialize;

use crate::backup::journal::{Action, EntryRef, Journal, Record, Status};
use crate::backup::restore_point;
use crate::guard;
use crate::log::ACTIVITY_TARGET;
use crate::sys::{RestorePoint, SysCtx};
use crate::tweaks::atom::{Atom, Original};
use crate::tweaks::{Applicability, Tweak, TweakError, TweakState};

pub struct Txn<'a> {
    ctx: &'a SysCtx,
    journal: &'a mut Journal,
    entry: EntryRef,
    /// For a revert: the records of the apply being undone.
    applied: Option<Vec<Record>>,
}

impl<'a> Txn<'a> {
    pub(crate) fn new(
        ctx: &'a SysCtx,
        journal: &'a mut Journal,
        entry: EntryRef,
        applied: Option<Vec<Record>>,
    ) -> Self {
        Self {
            ctx,
            journal,
            entry,
            applied,
        }
    }

    pub fn ctx(&self) -> &SysCtx {
        self.ctx
    }

    /// Writes an atom's target state, after journaling what was there.
    pub fn write(&mut self, atom: Atom) -> Result<(), TweakError> {
        guard::check_atom(&atom)?;
        self.save_before(&atom)?;
        atom.write(self.ctx)?;
        Ok(())
    }

    /// Puts a captured original back, after journaling the current state,
    /// and checks that it stuck.
    pub fn restore(&mut self, atom: &Atom, original: &Original) -> Result<(), TweakError> {
        guard::check_atom(atom)?;
        self.save_before(atom)?;
        atom.restore(self.ctx, original)?;
        if !atom.matches_original(self.ctx, original)? {
            return Err(TweakError::Verify(format!(
                "{} didn't keep its original value",
                atom.label()
            )));
        }
        Ok(())
    }

    /// Restores every record of the apply being reverted, newest first.
    pub fn restore_applied(&mut self) -> Result<(), TweakError> {
        let records = self
            .applied
            .clone()
            .ok_or_else(|| TweakError::NoBackup("This transaction isn't a revert.".into()))?;
        for record in records.iter().rev() {
            self.restore(&record.atom, &record.before)?;
        }
        Ok(())
    }

    fn save_before(&mut self, atom: &Atom) -> Result<(), TweakError> {
        let before = atom.capture(self.ctx)?;
        self.journal
            .record(
                self.entry,
                Record {
                    atom: atom.clone(),
                    before,
                },
            )
            .map_err(|e| TweakError::Journal(e.to_string()))
    }
}

/// Restores an entry's records newest-first. Keeps going past failures and
/// returns them, so as much as possible is put back.
pub(crate) fn rollback(
    ctx: &SysCtx,
    journal: &Journal,
    entry: EntryRef,
) -> Result<(), Vec<String>> {
    let Some(e) = journal.entry(entry) else {
        return Ok(());
    };
    let failures: Vec<String> = e
        .records
        .iter()
        .rev()
        .filter_map(|r| {
            let result = guard::check_atom(&r.atom)
                .map_err(|e| e.to_string())
                .and_then(|()| r.atom.restore(ctx, &r.before).map_err(|e| e.to_string()));
            result.err().map(|err| format!("{}: {err}", r.atom.label()))
        })
        .collect();
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ApplyOutcome {
    Applied,
    AlreadyApplied,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevertReport {
    pub reverted: Vec<String>,
    pub failed: Vec<RevertFailure>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevertFailure {
    pub tweak: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SafetyStatus {
    /// System Protection on the system drive; `None` if it can't be read.
    pub protection_enabled: Option<bool>,
    pub restore_point: Option<RestorePoint>,
    pub restore_point_skipped: bool,
    /// Applied changes that can be reverted.
    pub active_changes: usize,
    pub unreadable_backups: Vec<String>,
}

pub struct Engine {
    ctx: SysCtx,
    journal: Journal,
    catalog: Vec<Arc<dyn Tweak>>,
}

impl Engine {
    pub fn new(ctx: SysCtx, journal: Journal, catalog: Vec<Arc<dyn Tweak>>) -> Self {
        Self {
            ctx,
            journal,
            catalog,
        }
    }

    pub fn ctx(&self) -> &SysCtx {
        &self.ctx
    }

    pub fn journal(&self) -> &Journal {
        &self.journal
    }

    pub fn catalog(&self) -> &[Arc<dyn Tweak>] {
        &self.catalog
    }

    pub fn tweak(&self, id: &str) -> Option<Arc<dyn Tweak>> {
        self.catalog.iter().find(|t| t.id() == id).cloned()
    }

    pub fn apply(&mut self, tweak: &dyn Tweak) -> Result<ApplyOutcome, TweakError> {
        let name = tweak.name();
        if let Applicability::NotApplicable(reason) = tweak.applicable(&self.ctx)? {
            return Err(TweakError::NotApplicable(reason));
        }
        // Refuse before anything is written, not halfway through.
        for atom in tweak.atoms(&self.ctx)? {
            guard::check_atom(&atom)?;
        }
        if tweak.detect(&self.ctx)? == TweakState::Applied {
            return Ok(ApplyOutcome::AlreadyApplied);
        }
        self.ensure_restore_point()?;

        let entry = self
            .journal
            .begin(tweak.id(), name, Action::Apply, None)
            .map_err(|e| TweakError::Journal(e.to_string()))?;

        let mut result = tweak.apply(&mut Txn::new(&self.ctx, &mut self.journal, entry, None));
        if result.is_ok() {
            result = match tweak.detect(&self.ctx) {
                Ok(TweakState::Applied) => Ok(()),
                Ok(state) => Err(TweakError::Verify(format!(
                    "Windows didn't keep the change (now {state:?}). Another program or a policy may be resetting it."
                ))),
                Err(e) => Err(e.into()),
            };
        }

        match result {
            Ok(()) => {
                self.finish(entry, Status::Done, None);
                tracing::info!(target: ACTIVITY_TARGET, tweak = tweak.id(), "Applied {name}");
                self.after_change(tweak);
                Ok(ApplyOutcome::Applied)
            }
            Err(cause) => Err(self.undo_failed(entry, name, cause)),
        }
    }

    /// Applies one-off atoms that aren't a catalog tweak (e.g. switching power
    /// plan), with the same restore point, journal and rollback guarantees.
    /// Revert Everything undoes these too.
    pub fn apply_action(
        &mut self,
        id: &str,
        name: &str,
        atoms: Vec<Atom>,
    ) -> Result<(), TweakError> {
        for atom in &atoms {
            guard::check_atom(atom)?;
        }
        let mut pending = Vec::new();
        for atom in atoms {
            if !atom.is_applied(&self.ctx)? {
                pending.push(atom);
            }
        }
        if pending.is_empty() {
            return Ok(());
        }
        self.ensure_restore_point()?;
        let entry = self
            .journal
            .begin(id, name, Action::Apply, None)
            .map_err(|e| TweakError::Journal(e.to_string()))?;

        let mut txn = Txn::new(&self.ctx, &mut self.journal, entry, None);
        let mut result = pending.iter().try_for_each(|atom| txn.write(atom.clone()));
        if result.is_ok() {
            for atom in &pending {
                if !atom.is_applied(&self.ctx)? {
                    result = Err(TweakError::Verify(format!(
                        "Windows didn't keep {}",
                        atom.label()
                    )));
                    break;
                }
            }
        }
        match result {
            Ok(()) => {
                self.finish(entry, Status::Done, None);
                tracing::info!(target: ACTIVITY_TARGET, tweak = id, "{name}");
                Ok(())
            }
            Err(cause) => Err(self.undo_failed(entry, name, cause)),
        }
    }

    /// Reverts every active entry recorded under an action id, newest first.
    pub fn revert_action(&mut self, id: &str) -> Result<(), TweakError> {
        let entries: Vec<EntryRef> = self
            .journal
            .active_applies()
            .filter(|(_, e)| e.tweak_id == id)
            .map(|(r, _)| r)
            .collect();
        for entry in entries {
            self.revert_entry(entry)?;
        }
        Ok(())
    }

    pub fn revert(&mut self, tweak: &dyn Tweak) -> Result<(), TweakError> {
        let applied = self.journal.latest_applied(tweak.id()).ok_or_else(|| {
            TweakError::NoBackup(format!(
                "Apex Tune has no backup for {} because it didn't make this change.",
                tweak.name()
            ))
        })?;
        self.revert_with(applied, Some(tweak))
    }

    /// Undoes one apply entry. Uses the tweak's own revert when it's still
    /// in the catalog, otherwise restores the journaled records directly.
    pub fn revert_entry(&mut self, applied: EntryRef) -> Result<(), TweakError> {
        let tweak_id = self
            .journal
            .entry(applied)
            .map(|e| e.tweak_id.clone())
            .ok_or_else(|| TweakError::NoBackup("That backup no longer exists.".into()))?;
        let tweak = self.tweak(&tweak_id);
        self.revert_with(applied, tweak.as_deref())
    }

    fn revert_with(
        &mut self,
        applied: EntryRef,
        tweak: Option<&dyn Tweak>,
    ) -> Result<(), TweakError> {
        let source = self
            .journal
            .entry(applied)
            .cloned()
            .ok_or_else(|| TweakError::NoBackup("That backup no longer exists.".into()))?;
        let name = source.tweak_name.clone();

        let entry = self
            .journal
            .begin(&source.tweak_id, &name, Action::Revert, Some(applied))
            .map_err(|e| TweakError::Journal(e.to_string()))?;

        let mut txn = Txn::new(&self.ctx, &mut self.journal, entry, Some(source.records));
        let result = match tweak {
            Some(t) => t.revert(&mut txn),
            None => txn.restore_applied(),
        };

        match result {
            Ok(()) => {
                self.finish(entry, Status::Done, None);
                if let Err(e) = self.journal.mark_reverted(applied, entry) {
                    tracing::error!("Reverted {name} but couldn't update its backup record: {e}");
                }
                tracing::info!(target: ACTIVITY_TARGET, tweak = source.tweak_id.as_str(), "Reverted {name}");
                if let Some(t) = tweak {
                    self.after_change(t);
                }
                Ok(())
            }
            Err(cause) => Err(self.undo_failed(entry, &name, cause)),
        }
    }

    /// Reverts every active change, newest first. Keeps going past failures.
    pub fn revert_everything(&mut self) -> RevertReport {
        let targets: Vec<(EntryRef, String)> = self
            .journal
            .active_applies()
            .map(|(r, e)| (r, e.tweak_name.clone()))
            .collect();
        let mut report = RevertReport::default();
        for (applied, name) in targets {
            match self.revert_entry(applied) {
                Ok(()) => report.reverted.push(name),
                Err(e) => report.failed.push(RevertFailure {
                    tweak: name,
                    message: e.to_string(),
                }),
            }
        }
        tracing::info!(
            target: ACTIVITY_TARGET,
            "Revert everything: {} reverted, {} failed",
            report.reverted.len(),
            report.failed.len()
        );
        report
    }

    /// Rolls back entries a crash left in progress. Call once at startup.
    pub fn recover(&mut self) -> Vec<String> {
        let mut messages = Vec::new();
        for entry in self.journal.incomplete() {
            let name = self
                .journal
                .entry(entry)
                .map(|e| e.tweak_name.clone())
                .unwrap_or_default();
            match rollback(&self.ctx, &self.journal, entry) {
                Ok(()) => {
                    self.finish(
                        entry,
                        Status::RolledBack,
                        Some("Apex Tune closed before this finished.".into()),
                    );
                    let msg = format!("{name} was interrupted last time and has been undone");
                    tracing::warn!(target: ACTIVITY_TARGET, "{msg}");
                    messages.push(msg);
                }
                Err(failures) => {
                    let detail = failures.join("; ");
                    self.finish(entry, Status::RollbackFailed, Some(detail.clone()));
                    let msg = format!(
                        "{name} was interrupted last time and couldn't be fully undone: {detail}"
                    );
                    tracing::error!(target: ACTIVITY_TARGET, "{msg}");
                    messages.push(msg);
                }
            }
        }
        messages
    }

    pub fn safety_status(&self) -> SafetyStatus {
        let session = self.journal.current_session();
        SafetyStatus {
            protection_enabled: self.ctx.restore_points.protection_enabled().unwrap_or(None),
            restore_point: session.and_then(|s| s.restore_point.clone()),
            restore_point_skipped: session.is_some_and(|s| s.restore_point_skipped),
            active_changes: self.journal.active_applies().count(),
            unreadable_backups: self
                .journal
                .unreadable
                .iter()
                .map(|p| p.display().to_string())
                .collect(),
        }
    }

    /// Makes sure this session has a restore point (or the user explicitly
    /// declined one) before its first change.
    pub fn ensure_restore_point(&mut self) -> Result<(), TweakError> {
        if let Some(s) = self.journal.current_session()
            && (s.restore_point.is_some() || s.restore_point_skipped)
        {
            return Ok(());
        }
        if self.ctx.restore_points.protection_enabled()? == Some(false) {
            return Err(TweakError::RestorePointRequired(
                "System Protection is off for the system drive. Turn it on, or continue without a restore point."
                    .into(),
            ));
        }
        let point = restore_point::create_with_throttle_lifted(&self.ctx, &mut self.journal)
            .map_err(|e| {
                TweakError::RestorePointRequired(format!("Windows couldn't create one. {e}"))
            })?;
        self.journal
            .set_restore_point(point.clone())
            .map_err(|e| TweakError::Journal(e.to_string()))?;
        tracing::info!(target: ACTIVITY_TARGET, "Created restore point \"{}\"", point.description);
        Ok(())
    }

    pub fn enable_protection(&mut self) -> Result<(), TweakError> {
        self.ctx.restore_points.enable_protection()?;
        tracing::info!(target: ACTIVITY_TARGET, "Turned on System Protection for the system drive");
        Ok(())
    }

    pub fn skip_restore_point(&mut self) -> Result<(), TweakError> {
        self.journal
            .set_restore_point_skipped()
            .map_err(|e| TweakError::Journal(e.to_string()))?;
        tracing::warn!(target: ACTIVITY_TARGET, "Continuing this session without a restore point");
        Ok(())
    }

    fn after_change(&self, tweak: &dyn Tweak) {
        if let Err(e) = tweak.after_change(&self.ctx) {
            tracing::warn!(
                "{} is saved but needs a sign-out to take effect: {e}",
                tweak.name()
            );
        }
    }

    /// Rolls back a failed transaction and builds the error to report.
    fn undo_failed(&mut self, entry: EntryRef, name: &str, cause: TweakError) -> TweakError {
        match rollback(&self.ctx, &self.journal, entry) {
            Ok(()) => {
                self.finish(entry, Status::RolledBack, Some(cause.to_string()));
                let err = TweakError::RolledBack {
                    tweak: name.to_owned(),
                    cause: Box::new(cause),
                };
                tracing::error!(target: ACTIVITY_TARGET, "{err}");
                err
            }
            Err(failures) => {
                self.finish(entry, Status::RollbackFailed, Some(failures.join("; ")));
                let err = TweakError::RollbackFailed {
                    tweak: name.to_owned(),
                    cause: Box::new(cause),
                    failures,
                };
                tracing::error!(target: ACTIVITY_TARGET, "{err}");
                err
            }
        }
    }

    fn finish(&mut self, entry: EntryRef, status: Status, error: Option<String>) {
        if let Err(e) = self.journal.finish(entry, status, error) {
            // The change itself is settled; only its record is stale. Recovery
            // would replay a rollback, which restores the same values.
            tracing::error!("Couldn't update the backup record: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::fake::FakeSystem;
    use crate::sys::{Hive, RegData, StartType, SysErrorKind};
    use crate::tweaks::StaticTweak;
    use crate::tweaks::test_support::*;

    fn engine(sys: &FakeSystem, dir: &std::path::Path) -> Engine {
        Engine::new(sys.ctx.clone(), Journal::open(dir).unwrap(), vec![])
    }

    fn val(sys: &FakeSystem, name: &str) -> Option<RegData> {
        sys.registry.value(Hive::CurrentUser, PATH, name)
    }

    #[test]
    fn apply_then_revert_restores_exact_originals() {
        let sys = FakeSystem::new();
        let dir = tempfile::tempdir().unwrap();
        sys.registry
            .seed(Hive::CurrentUser, PATH, "A", RegData::Dword(5));
        let mut engine = engine(&sys, dir.path());
        let tweak = two_values();

        assert_eq!(engine.apply(&tweak).unwrap(), ApplyOutcome::Applied);
        assert_eq!(val(&sys, "A"), Some(RegData::Dword(1)));
        assert_eq!(engine.safety_status().active_changes, 1);
        assert_eq!(sys.restore_points.count(), 1);

        engine.revert(&tweak).unwrap();
        assert_eq!(val(&sys, "A"), Some(RegData::Dword(5)));
        assert_eq!(val(&sys, "B"), None);
        assert_eq!(engine.safety_status().active_changes, 0);
    }

    #[test]
    fn already_applied_writes_nothing() {
        let sys = FakeSystem::new();
        let dir = tempfile::tempdir().unwrap();
        sys.registry
            .seed(Hive::CurrentUser, PATH, "A", RegData::Dword(1));
        sys.registry
            .seed(Hive::CurrentUser, PATH, "B", RegData::Dword(1));
        let mut engine = engine(&sys, dir.path());
        assert_eq!(
            engine.apply(&two_values()).unwrap(),
            ApplyOutcome::AlreadyApplied
        );
        assert_eq!(sys.registry.write_count(), 0);
        assert_eq!(sys.restore_points.count(), 0);
    }

    #[test]
    fn failure_midway_rolls_back_earlier_writes() {
        let sys = FakeSystem::new();
        let dir = tempfile::tempdir().unwrap();
        sys.registry
            .seed(Hive::CurrentUser, PATH, "A", RegData::Dword(5));
        sys.registry.fail_writes_to(r"apextest\b");
        let mut engine = engine(&sys, dir.path());

        let err = engine.apply(&two_values()).unwrap_err();
        assert_eq!(err.kind(), "rolledBack");
        assert_eq!(
            val(&sys, "A"),
            Some(RegData::Dword(5)),
            "A must be put back"
        );
        assert_eq!(engine.safety_status().active_changes, 0);
    }

    #[test]
    fn change_that_does_not_stick_is_rolled_back() {
        let sys = FakeSystem::new();
        let dir = tempfile::tempdir().unwrap();
        sys.registry.ignore_writes_to(r"apextest\b");
        let mut engine = engine(&sys, dir.path());
        let err = engine.apply(&two_values()).unwrap_err();
        match err {
            TweakError::RolledBack { cause, .. } => assert_eq!(cause.kind(), "verify"),
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(val(&sys, "A"), None);
    }

    #[test]
    fn rollback_failure_is_reported_with_the_stuck_setting() {
        let sys = FakeSystem::new();
        let dir = tempfile::tempdir().unwrap();
        let mut engine = engine(&sys, dir.path());
        engine.skip_restore_point().unwrap();
        let entry = engine
            .journal
            .begin("stuck", "Stuck", Action::Apply, None)
            .unwrap();
        Txn::new(&engine.ctx, &mut engine.journal, entry, None)
            .write(Atom::reg_dword(Hive::CurrentUser, PATH, "A", 1))
            .unwrap();
        // The write landed, but now Windows denies putting A back.
        sys.registry.fail_writes_to(r"apextest\a");
        let err = engine.undo_failed(entry, "Stuck", TweakError::Verify("x".into()));
        match err {
            TweakError::RollbackFailed { failures, .. } => {
                assert!(failures[0].contains(r"ApexTest\A"))
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn guard_refusal_happens_before_any_write() {
        let sys = FakeSystem::new();
        let dir = tempfile::tempdir().unwrap();
        sys.services.install("vgc", StartType::Manual, true);
        let tweak = StaticTweak {
            refresh: crate::tweaks::Refresh::Nothing,
            meta: meta("evil"),
            atoms: || {
                vec![
                    Atom::reg_dword(Hive::CurrentUser, PATH, "A", 1),
                    Atom::service("vgc", StartType::Disabled, true),
                ]
            },
        };
        let mut engine = engine(&sys, dir.path());
        let err = engine.apply(&tweak).unwrap_err();
        assert_eq!(err.kind(), "refused");
        assert_eq!(sys.registry.write_count(), 0);
        assert!(sys.services.state("vgc").unwrap().running);
    }

    #[test]
    fn revert_without_backup_explains_why() {
        let sys = FakeSystem::new();
        let dir = tempfile::tempdir().unwrap();
        let err = engine(&sys, dir.path()).revert(&two_values()).unwrap_err();
        assert_eq!(err.kind(), "noBackup");
    }

    #[test]
    fn failed_revert_puts_the_applied_state_back() {
        let sys = FakeSystem::new();
        let dir = tempfile::tempdir().unwrap();
        sys.registry
            .seed(Hive::CurrentUser, PATH, "A", RegData::Dword(5));
        sys.registry
            .seed(Hive::CurrentUser, PATH, "B", RegData::Dword(5));
        let mut engine = engine(&sys, dir.path());
        let tweak = two_values();
        engine.apply(&tweak).unwrap();

        // Restoring B (first, since reverts go newest-first) works; A is denied.
        sys.registry.fail_writes_to(r"apextest\a");
        let err = engine.revert(&tweak).unwrap_err();
        sys.registry.clear_failures();
        assert!(matches!(err.kind(), "rolledBack" | "rollbackFailed"));
        assert_eq!(
            val(&sys, "B"),
            Some(RegData::Dword(1)),
            "B must be re-applied"
        );
        assert_eq!(
            engine.safety_status().active_changes,
            1,
            "still revertable later"
        );
        engine.revert(&tweak).unwrap();
        assert_eq!(val(&sys, "A"), Some(RegData::Dword(5)));
    }

    #[test]
    fn revert_everything_goes_newest_first_and_continues_past_failures() {
        let sys = FakeSystem::new();
        let dir = tempfile::tempdir().unwrap();
        let mut engine = engine(&sys, dir.path());
        let first = StaticTweak {
            refresh: crate::tweaks::Refresh::Nothing,
            meta: meta("first"),
            atoms: || vec![Atom::reg_dword(Hive::CurrentUser, PATH, "X", 1)],
        };
        let second = StaticTweak {
            refresh: crate::tweaks::Refresh::Nothing,
            meta: meta("second"),
            atoms: || vec![Atom::reg_dword(Hive::CurrentUser, PATH, "X", 2)],
        };
        let third = StaticTweak {
            refresh: crate::tweaks::Refresh::Nothing,
            meta: meta("third"),
            atoms: || vec![Atom::reg_dword(Hive::LocalMachine, PATH, "Y", 1)],
        };
        engine.apply(&first).unwrap();
        engine.apply(&second).unwrap();
        engine.apply(&third).unwrap();

        sys.registry.fail_writes_to(r"hklm\software\apextest\y");
        let report = engine.revert_everything();
        assert_eq!(report.failed.len(), 1);
        assert_eq!(report.reverted.len(), 2);
        // second (X=2 → 1) then first (X=1 → absent).
        assert_eq!(val(&sys, "X"), None);
    }

    #[test]
    fn interrupted_apply_is_rolled_back_on_next_start() {
        let sys = FakeSystem::new();
        let dir = tempfile::tempdir().unwrap();
        sys.registry
            .seed(Hive::CurrentUser, PATH, "A", RegData::Dword(5));
        {
            let mut engine = engine(&sys, dir.path());
            engine.skip_restore_point().unwrap();
            let entry = engine
                .journal
                .begin("two_values", "Two", Action::Apply, None)
                .unwrap();
            let mut txn = Txn::new(&engine.ctx, &mut engine.journal, entry, None);
            txn.write(Atom::reg_dword(Hive::CurrentUser, PATH, "A", 1))
                .unwrap();
            // Process "dies" here.
        }
        let mut engine = engine(&sys, dir.path());
        let messages = engine.recover();
        assert_eq!(messages.len(), 1);
        assert_eq!(val(&sys, "A"), Some(RegData::Dword(5)));
        assert!(engine.journal.incomplete().is_empty());
    }

    #[test]
    fn protection_off_blocks_until_enabled_or_skipped() {
        let sys = FakeSystem::new();
        *sys.restore_points.enabled.lock().unwrap() = Some(false);
        let dir = tempfile::tempdir().unwrap();
        let mut engine = engine(&sys, dir.path());

        let err = engine.apply(&two_values()).unwrap_err();
        assert_eq!(err.kind(), "restorePointRequired");
        assert_eq!(sys.registry.write_count(), 0);

        engine.enable_protection().unwrap();
        engine.apply(&two_values()).unwrap();
        assert_eq!(sys.restore_points.count(), 1);
    }

    #[test]
    fn skipping_the_restore_point_is_recorded_once_per_session() {
        let sys = FakeSystem::new();
        *sys.restore_points.fail_create.lock().unwrap() = Some(crate::sys::SysError::new(
            SysErrorKind::Other,
            "create restore point",
            "C:",
            "boom",
        ));
        let dir = tempfile::tempdir().unwrap();
        let mut engine = engine(&sys, dir.path());
        assert_eq!(
            engine.apply(&two_values()).unwrap_err().kind(),
            "restorePointRequired"
        );
        engine.skip_restore_point().unwrap();
        engine.apply(&two_values()).unwrap();
        assert!(engine.safety_status().restore_point_skipped);
    }

    #[test]
    fn restore_point_throttle_is_lifted_and_put_back() {
        let sys = FakeSystem::new();
        let dir = tempfile::tempdir().unwrap();
        let (path, name) = (restore_point::FREQUENCY_PATH, restore_point::FREQUENCY_NAME);
        sys.registry
            .seed(Hive::LocalMachine, path, name, RegData::Dword(1440));
        let mut engine = engine(&sys, dir.path());
        engine.ensure_restore_point().unwrap();
        assert_eq!(sys.restore_points.count(), 1);
        assert_eq!(
            sys.registry.value(Hive::LocalMachine, path, name),
            Some(RegData::Dword(1440))
        );
        assert!(engine.journal.incomplete().is_empty());
    }
}
