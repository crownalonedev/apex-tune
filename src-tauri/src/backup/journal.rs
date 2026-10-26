//! Write-ahead journal of every change, one JSON file per session in
//! `%ProgramData%\ApexTune\backups\`.
//!
//! Before an atom is written, its current state is appended to the entry and
//! the file is saved (temp file + rename). If Apex Tune dies mid-change, the
//! entry is left `inProgress` and is rolled back on the next start.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::sys::RestorePoint;
use crate::tweaks::atom::{Atom, Original};

const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Apply,
    Revert,
    /// Apex Tune's own bookkeeping, e.g. lifting the restore point throttle.
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    InProgress,
    Done,
    RolledBack,
    RollbackFailed,
}

/// The state of one target just before Apex Tune wrote to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub atom: Atom,
    pub before: Original,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: Uuid,
    pub tweak_id: String,
    pub tweak_name: String,
    pub action: Action,
    pub status: Status,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub records: Vec<Record>,
    pub error: Option<String>,
    /// For a revert: the apply entry it undid.
    pub reverts: Option<EntryRef>,
    /// For an apply: the revert entry that undid it.
    pub reverted_by: Option<EntryRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub format_version: u32,
    pub id: Uuid,
    pub started_at: DateTime<Utc>,
    pub restore_point: Option<RestorePoint>,
    /// The user chose to continue without a restore point.
    pub restore_point_skipped: bool,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryRef {
    pub session: Uuid,
    pub entry: Uuid,
}

pub struct Journal {
    dir: PathBuf,
    /// Oldest first. The last one is the current session once it exists.
    sessions: Vec<(PathBuf, Session)>,
    current: Option<Uuid>,
    /// Files that couldn't be read, kept for the user to inspect.
    pub unreadable: Vec<PathBuf>,
}

impl Journal {
    /// Loads every session file in `dir`. Unreadable files are listed, not deleted.
    pub fn open(dir: &Path) -> std::io::Result<Self> {
        fs::create_dir_all(dir)?;
        let mut sessions = Vec::new();
        let mut unreadable = Vec::new();
        for item in fs::read_dir(dir)? {
            let path = item?.path();
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            match fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|bytes| {
                    serde_json::from_slice::<Session>(&bytes).map_err(|e| e.to_string())
                }) {
                Ok(session) => sessions.push((path, session)),
                Err(e) => {
                    tracing::warn!("Skipping unreadable backup file {}: {e}", path.display());
                    unreadable.push(path);
                }
            }
        }
        sessions.sort_by_key(|(_, s)| s.started_at);
        Ok(Self {
            dir: dir.to_owned(),
            sessions,
            current: None,
            unreadable,
        })
    }

    pub fn sessions(&self) -> impl DoubleEndedIterator<Item = &Session> {
        self.sessions.iter().map(|(_, s)| s)
    }

    pub fn current_session(&self) -> Option<&Session> {
        let id = self.current?;
        self.sessions().find(|s| s.id == id)
    }

    /// The current session, created and saved on first use.
    pub fn ensure_session(&mut self) -> std::io::Result<Uuid> {
        if let Some(id) = self.current {
            return Ok(id);
        }
        let now = Utc::now();
        let id = Uuid::new_v4();
        let session = Session {
            format_version: FORMAT_VERSION,
            id,
            started_at: now,
            restore_point: None,
            restore_point_skipped: false,
            entries: Vec::new(),
        };
        let simple = id.simple().to_string();
        let file = self.dir.join(format!(
            "{}-{}.json",
            now.format("%Y%m%d-%H%M%S"),
            &simple[..8]
        ));
        save(&file, &session)?;
        self.sessions.push((file, session));
        self.current = Some(id);
        Ok(id)
    }

    pub fn set_restore_point(&mut self, point: RestorePoint) -> std::io::Result<()> {
        let id = self.ensure_session()?;
        self.update_session(id, |s| s.restore_point = Some(point))
    }

    pub fn set_restore_point_skipped(&mut self) -> std::io::Result<()> {
        let id = self.ensure_session()?;
        self.update_session(id, |s| s.restore_point_skipped = true)
    }

    pub fn begin(
        &mut self,
        tweak_id: &str,
        tweak_name: &str,
        action: Action,
        reverts: Option<EntryRef>,
    ) -> std::io::Result<EntryRef> {
        let session = self.ensure_session()?;
        let entry = Entry {
            id: Uuid::new_v4(),
            tweak_id: tweak_id.to_owned(),
            tweak_name: tweak_name.to_owned(),
            action,
            status: Status::InProgress,
            started_at: Utc::now(),
            finished_at: None,
            records: Vec::new(),
            error: None,
            reverts,
            reverted_by: None,
        };
        let r = EntryRef {
            session,
            entry: entry.id,
        };
        self.update_session(session, |s| s.entries.push(entry))?;
        Ok(r)
    }

    /// Appends a record and saves. Must succeed before the atom is written.
    pub fn record(&mut self, r: EntryRef, record: Record) -> std::io::Result<()> {
        self.update_entry(r, |e| e.records.push(record))
    }

    pub fn finish(
        &mut self,
        r: EntryRef,
        status: Status,
        error: Option<String>,
    ) -> std::io::Result<()> {
        self.update_entry(r, |e| {
            e.status = status;
            e.error = error;
            e.finished_at = Some(Utc::now());
        })
    }

    pub fn mark_reverted(&mut self, applied: EntryRef, by: EntryRef) -> std::io::Result<()> {
        self.update_entry(applied, |e| e.reverted_by = Some(by))
    }

    pub fn entry(&self, r: EntryRef) -> Option<&Entry> {
        self.sessions()
            .find(|s| s.id == r.session)
            .and_then(|s| s.entries.iter().find(|e| e.id == r.entry))
    }

    /// The newest completed, not-yet-reverted apply of this tweak.
    pub fn latest_applied(&self, tweak_id: &str) -> Option<EntryRef> {
        self.active_applies()
            .find(|(_, e)| e.tweak_id == tweak_id)
            .map(|(r, _)| r)
    }

    /// Completed, not-yet-reverted applies, newest first.
    pub fn active_applies(&self) -> impl Iterator<Item = (EntryRef, &Entry)> {
        self.sessions().rev().flat_map(|s| {
            s.entries
                .iter()
                .rev()
                .filter(|e| {
                    e.action == Action::Apply && e.status == Status::Done && e.reverted_by.is_none()
                })
                .map(move |e| {
                    (
                        EntryRef {
                            session: s.id,
                            entry: e.id,
                        },
                        e,
                    )
                })
        })
    }

    /// Entries left in progress by a crash, newest first.
    pub fn incomplete(&self) -> Vec<EntryRef> {
        self.sessions()
            .rev()
            .flat_map(|s| {
                s.entries
                    .iter()
                    .rev()
                    .filter(|e| e.status == Status::InProgress)
                    .map(move |e| EntryRef {
                        session: s.id,
                        entry: e.id,
                    })
            })
            .collect()
    }

    fn update_session(&mut self, id: Uuid, f: impl FnOnce(&mut Session)) -> std::io::Result<()> {
        let (path, session) = self
            .sessions
            .iter_mut()
            .find(|(_, s)| s.id == id)
            .ok_or_else(|| std::io::Error::other("backup session not found"))?;
        // Save a modified copy first so memory never runs ahead of disk.
        let mut updated = session.clone();
        f(&mut updated);
        save(path, &updated)?;
        *session = updated;
        Ok(())
    }

    fn update_entry(&mut self, r: EntryRef, f: impl FnOnce(&mut Entry)) -> std::io::Result<()> {
        let mut found = false;
        self.update_session(r.session, |s| {
            if let Some(e) = s.entries.iter_mut().find(|e| e.id == r.entry) {
                f(e);
                found = true;
            }
        })?;
        if found {
            Ok(())
        } else {
            Err(std::io::Error::other("backup entry not found"))
        }
    }
}

/// Writes atomically: temp file, flush to disk, rename over the target.
fn save(path: &Path, session: &Session) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    {
        let mut file = fs::File::create(&tmp)?;
        serde_json::to_writer_pretty(&mut file, session)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::{Hive, RegData};

    fn record() -> Record {
        Record {
            atom: Atom::reg_dword(Hive::CurrentUser, r"Software\X", "v", 1),
            before: Original::Registry {
                value: Some(RegData::Dword(0)),
                created_keys: vec![],
            },
        }
    }

    #[test]
    fn survives_reopen_and_tracks_active_applies() {
        let dir = tempfile::tempdir().unwrap();
        let applied = {
            let mut j = Journal::open(dir.path()).unwrap();
            let r = j
                .begin("game_mode", "Game Mode", Action::Apply, None)
                .unwrap();
            j.record(r, record()).unwrap();
            j.finish(r, Status::Done, None).unwrap();
            r
        };

        let mut j = Journal::open(dir.path()).unwrap();
        assert_eq!(j.latest_applied("game_mode"), Some(applied));
        assert_eq!(j.entry(applied).unwrap().records, vec![record()]);

        let revert = j
            .begin("game_mode", "Game Mode", Action::Revert, Some(applied))
            .unwrap();
        j.finish(revert, Status::Done, None).unwrap();
        j.mark_reverted(applied, revert).unwrap();
        assert_eq!(j.latest_applied("game_mode"), None);
    }

    #[test]
    fn unfinished_entries_are_reported_after_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let r = {
            let mut j = Journal::open(dir.path()).unwrap();
            let r = j.begin("hags", "HAGS", Action::Apply, None).unwrap();
            j.record(r, record()).unwrap();
            r
        };
        let j = Journal::open(dir.path()).unwrap();
        assert_eq!(j.incomplete(), vec![r]);
    }

    #[test]
    fn corrupt_files_are_skipped_not_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("bad.json");
        fs::write(&bad, "{ not json").unwrap();
        let j = Journal::open(dir.path()).unwrap();
        assert_eq!(j.unreadable, vec![bad.clone()]);
        assert!(bad.exists());
    }

    #[test]
    fn no_file_until_first_use() {
        let dir = tempfile::tempdir().unwrap();
        let mut j = Journal::open(dir.path()).unwrap();
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
        j.ensure_session().unwrap();
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
