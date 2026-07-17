//! Windows-only: name the processes holding open handles to the DB files.
//!
//! Three startup "database disk image is malformed" incidents (2026-07-13/15/16)
//! were transient — the file was provably intact and a later open worked — so
//! some external process holds or rewrites the index at the exact moment of
//! open, and the retry warnings alone have never been able to say WHO. The
//! Restart Manager API enumerates processes with open handles to a file, so
//! each failed open attempt logs the current holders of
//! `flyonthewall.db{,-wal,-shm}` next to the retry warning.
//!
//! Diagnostics only: every failure path here degrades to a log line — probing
//! must never block or fail the retry loop itself.

use std::path::Path;

/// Warn-log which processes hold open handles to the SQLite index files right
/// now. Best-effort by design; a no-op off Windows (no equivalent API wired).
#[cfg(windows)]
pub(crate) fn log_db_lock_owners(data_dir: &Path, db_file: &str) {
    let files: Vec<std::path::PathBuf> = [
        db_file.to_string(),
        format!("{db_file}-wal"),
        format!("{db_file}-shm"),
    ]
    .into_iter()
    .map(|name| data_dir.join(name))
    .filter(|path| path.exists())
    .collect();
    if files.is_empty() {
        // The open failed AND no index files are on disk — itself a lead
        // (something deleted them, or the data dir is inaccessible).
        tracing::warn!("no DB files present to probe for lock owners");
        return;
    }
    match holders(&files) {
        Ok(owners) if owners.is_empty() => tracing::warn!(
            "no process holds the DB files right now (interference already \
             cleared, or the interferer closes between attempts)"
        ),
        Ok(owners) => tracing::warn!(
            holders = %owners.join("; "),
            "processes holding the DB files"
        ),
        Err(code) => tracing::warn!(code, "Restart Manager probe for DB lock owners failed"),
    }
}

#[cfg(not(windows))]
pub(crate) fn log_db_lock_owners(_data_dir: &Path, _db_file: &str) {}

/// Processes with open handles to `files`, as `"name (pid N)"` strings, via a
/// throwaway Restart Manager session. `Err` carries the Win32 error code.
#[cfg(windows)]
fn holders(files: &[std::path::PathBuf]) -> Result<Vec<String>, u32> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::RestartManager::{
        RmEndSession, RmStartSession, CCH_RM_SESSION_KEY,
    };

    // NUL-terminated wide absolute paths, kept alive for the whole session.
    let wide: Vec<Vec<u16>> = files
        .iter()
        .map(|p| {
            p.as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect()
        })
        .collect();
    let names: Vec<PCWSTR> = wide.iter().map(|w| PCWSTR(w.as_ptr())).collect();

    let mut session = 0u32;
    let mut key = [0u16; CCH_RM_SESSION_KEY as usize + 1];
    // SAFETY: the key buffer outlives the call and has the documented minimum
    // capacity (CCH_RM_SESSION_KEY + NUL).
    let rc = unsafe { RmStartSession(&mut session, None, PWSTR(key.as_mut_ptr())) };
    if rc != ERROR_SUCCESS {
        return Err(rc.0);
    }
    let result = holders_in_session(session, &names);
    // SAFETY: `session` came from a successful RmStartSession; always closed,
    // even when registration or the list call failed. Nothing to do about a
    // close failure — the session leaks until process exit at worst.
    let _ = unsafe { RmEndSession(session) };
    result
}

#[cfg(windows)]
fn holders_in_session(session: u32, names: &[windows::core::PCWSTR]) -> Result<Vec<String>, u32> {
    use windows::Win32::Foundation::{ERROR_MORE_DATA, ERROR_SUCCESS};
    use windows::Win32::System::RestartManager::{RmGetList, RmRegisterResources, RM_PROCESS_INFO};

    // SAFETY: `names` points at NUL-terminated wide strings the caller keeps
    // alive across the session.
    let rc = unsafe { RmRegisterResources(session, Some(names), None, None) };
    if rc != ERROR_SUCCESS {
        return Err(rc.0);
    }

    // Size-then-fetch loop: a process can open the files between the sizing
    // call and the fetch, which surfaces as ERROR_MORE_DATA again.
    let mut procs: Vec<RM_PROCESS_INFO> = Vec::new();
    let mut reasons = 0u32;
    loop {
        let mut needed = 0u32;
        let mut count = procs.len() as u32;
        // SAFETY: `procs` has exactly `count` (zero-initialized) elements;
        // passing no buffer is allowed when `count` is 0.
        let rc = unsafe {
            RmGetList(
                session,
                &mut needed,
                &mut count,
                if procs.is_empty() {
                    None
                } else {
                    Some(procs.as_mut_ptr())
                },
                &mut reasons,
            )
        };
        if rc == ERROR_SUCCESS {
            procs.truncate(count as usize);
            return Ok(procs.iter().map(describe).collect());
        }
        if rc != ERROR_MORE_DATA {
            return Err(rc.0);
        }
        // Headroom so a process appearing mid-probe doesn't force another lap.
        // SAFETY: RM_PROCESS_INFO is a plain-old-data Win32 struct; all-zero
        // is a valid (empty) value.
        procs = vec![unsafe { std::mem::zeroed() }; needed as usize + 4];
    }
}

/// `"App Name (pid 1234)"` from the friendly name Restart Manager reports,
/// or just the pid when the name is empty.
#[cfg(windows)]
fn describe(info: &windows::Win32::System::RestartManager::RM_PROCESS_INFO) -> String {
    let len = info
        .strAppName
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(info.strAppName.len());
    let name = String::from_utf16_lossy(&info.strAppName[..len]);
    let pid = info.Process.dwProcessId;
    if name.is_empty() {
        format!("pid {pid}")
    } else {
        format!("{name} (pid {pid})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The probe is fire-and-forget on every platform: pointing it at a
    /// directory with no DB files must log and return, never panic or error.
    #[test]
    fn log_db_lock_owners_is_a_safe_no_op_without_files() {
        let dir = tempfile::tempdir().unwrap();
        log_db_lock_owners(dir.path(), "flyonthewall.db");
    }

    /// No contention: a DB file nobody has open reports zero holders.
    #[cfg(windows)]
    #[test]
    fn holders_empty_for_unheld_file() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("flyonthewall.db");
        std::fs::write(&db, b"not really a db").unwrap();
        let owners = holders(&[db]).expect("probe succeeds");
        assert!(owners.is_empty(), "unexpected holders: {owners:?}");
    }

    /// Contention: while this test process holds the file open, the probe
    /// must name it — the exact evidence the startup incidents are missing.
    #[cfg(windows)]
    #[test]
    fn holders_reports_the_process_holding_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("flyonthewall.db");
        std::fs::write(&db, b"not really a db").unwrap();
        let _held = std::fs::File::open(&db).unwrap();
        let owners = holders(&[db]).expect("probe succeeds");
        let pid = std::process::id();
        assert!(
            owners.iter().any(|o| o.contains(&format!("pid {pid}"))),
            "own pid {pid} not in holders: {owners:?}"
        );
    }
}
