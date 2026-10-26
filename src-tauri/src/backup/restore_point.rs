//! System Restore points.
//!
//! Windows silently skips a new restore point if one was made in the last
//! 24 hours. To get one reliably, Apex Tune sets
//! `SystemRestorePointCreationFrequency` to 0 for the duration of the call
//! and puts the previous value back right after. That change is journaled
//! like any other, so a crash in between is rolled back on the next start.

use crate::backup::journal::{Action, Journal, Status};
use crate::sys::{Hive, RestorePoint, SysCtx};
use crate::tweaks::TweakError;
use crate::tweaks::atom::Atom;
use crate::tweaks::engine::{Txn, rollback};

pub const FREQUENCY_PATH: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\SystemRestore";
pub const FREQUENCY_NAME: &str = "SystemRestorePointCreationFrequency";
pub const DESCRIPTION: &str = "Apex Tune: before changes";

const BOOKKEEPING_ID: &str = "system.restore_point_frequency";

pub fn create_with_throttle_lifted(
    ctx: &SysCtx,
    journal: &mut Journal,
) -> Result<RestorePoint, TweakError> {
    let entry = journal
        .begin(
            BOOKKEEPING_ID,
            "Restore point frequency",
            Action::System,
            None,
        )
        .map_err(|e| TweakError::Journal(e.to_string()))?;

    let lifted = Txn::new(ctx, journal, entry, None).write(Atom::reg_dword(
        Hive::LocalMachine,
        FREQUENCY_PATH,
        FREQUENCY_NAME,
        0,
    ));
    // Creating a point is still worth trying if the throttle couldn't be lifted.
    if let Err(e) = &lifted {
        tracing::warn!("Couldn't lift the restore point throttle: {e}");
    }
    let created = ctx.restore_points.create(DESCRIPTION);

    let status = match rollback(ctx, journal, entry) {
        Ok(()) => Status::Done,
        Err(failures) => {
            tracing::error!(
                "Couldn't put {FREQUENCY_NAME} back: {}",
                failures.join("; ")
            );
            Status::RollbackFailed
        }
    };
    if let Err(e) = journal.finish(entry, status, None) {
        tracing::error!("Couldn't update the backup record: {e}");
    }
    Ok(created?)
}
