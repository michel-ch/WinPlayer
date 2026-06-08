use crate::data::scanner::is_audio_path;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub fn renumber_folder(folder: &Path, threshold: f32) -> std::io::Result<u32> {
    let mut audio_files: Vec<PathBuf> = std::fs::read_dir(folder)?
        .filter_map(|r| r.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_audio_path(p))
        .collect();

    if audio_files.is_empty() {
        return Ok(0);
    }
    audio_files.sort();

    let prefixed = audio_files.iter().filter(|p| has_track_prefix(p)).count();
    let ratio = prefixed as f32 / audio_files.len() as f32;
    if ratio < threshold {
        return Ok(0);
    }

    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();

    // Pass 1: move each source to a unique temp name. Track every successful
    // move so we can roll back if a later rename errors.
    let mut temp_paths: Vec<(PathBuf, PathBuf, u32)> = Vec::with_capacity(audio_files.len());
    let mut rollback: Vec<(PathBuf, PathBuf)> = Vec::with_capacity(audio_files.len());
    for (i, src) in audio_files.iter().enumerate() {
        let stem_clean = strip_existing_prefix(src);
        let ext = src
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();
        let tmp_name = format!(
            ".tmp_renumber_{}_{}_{}_{}.{}",
            pid,
            nanos,
            i,
            sanitize(&stem_clean),
            ext
        );
        let tmp = folder.join(tmp_name);
        if let Err(e) = std::fs::rename(src, &tmp) {
            // Roll back every prior move before bailing out.
            undo(&rollback);
            return Err(e);
        }
        rollback.push((tmp.clone(), src.clone()));
        temp_paths.push((tmp, PathBuf::from(stem_clean), (i + 1) as u32));
    }

    // Pass 2: rename each temp to its final numbered name. If the final
    // name already exists (collision with an unrelated pre-existing file),
    // skip that file (leaving it under .tmp_renumber_* is preferable to
    // clobbering data). If the rename itself errors, roll back everything.
    let mut renamed = 0u32;
    for (tmp, stem, idx) in temp_paths {
        let ext = tmp
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();
        let stem_str = stem.to_string_lossy();
        let final_name = if ext.is_empty() {
            format!("{:02} - {}", idx, stem_str)
        } else {
            format!("{:02} - {}.{}", idx, stem_str, ext)
        };
        let final_path = folder.join(&final_name);
        if final_path.exists() {
            // Restore the temp back to the original name so we don't leave
            // a `.tmp_renumber_*` orphan; the user's file is now numbered
            // the same way it started.
            log::warn!("renumber: skipping {} — target already exists", final_name);
            if let Some(entry) = rollback.iter().find(|(t, _)| *t == tmp) {
                let _ = std::fs::rename(&entry.0, &entry.1);
            }
            continue;
        }
        if let Err(e) = std::fs::rename(&tmp, &final_path) {
            log::error!("renumber: final rename failed for {}: {}", final_name, e);
            undo(&rollback);
            return Err(e);
        }
        // Update the rollback record so we'd unwind from final_path now.
        if let Some(entry) = rollback.iter_mut().find(|(t, _)| *t == tmp) {
            entry.0 = final_path;
        }
        renamed += 1;
    }
    Ok(renamed)
}

fn undo(rollback: &[(PathBuf, PathBuf)]) {
    // Best-effort rollback in reverse order — restore each successfully
    // moved file to its original name. Errors here can't be propagated;
    // they're logged so the user has a recovery breadcrumb.
    for (current, original) in rollback.iter().rev() {
        if let Err(e) = std::fs::rename(current, original) {
            log::error!(
                "renumber rollback failed: could not restore {} -> {}: {}",
                current.display(),
                original.display(),
                e,
            );
        }
    }
}

fn has_track_prefix(path: &Path) -> bool {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    let mut digits = 0;
    for c in stem.chars() {
        if c.is_ascii_digit() {
            digits += 1;
        } else {
            return digits > 0 && (c == ' ' || c == '-' || c == '_' || c == '.');
        }
    }
    false
}

fn strip_existing_prefix(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let trimmed = stem.trim_start_matches(|c: char| c.is_ascii_digit());
    trimmed.trim_start_matches([' ', '-', '_', '.']).to_string()
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .take(40)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    fn touch(dir: &Path, name: &str) {
        File::create(dir.join(name)).unwrap();
    }

    #[test]
    fn renames_when_above_threshold() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "01 - alpha.mp3");
        touch(dir.path(), "02 - beta.mp3");
        touch(dir.path(), "03 - gamma.mp3");
        let n = renumber_folder(dir.path(), 0.5).unwrap();
        assert_eq!(n, 3);
        let names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|r| r.ok())
            .map(|e| e.file_name().into_string().unwrap())
            .collect();
        assert!(names.iter().any(|n| n.starts_with("01 - alpha")));
    }

    #[test]
    fn skips_when_below_threshold() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "alpha.mp3");
        touch(dir.path(), "beta.mp3");
        touch(dir.path(), "01 - gamma.mp3");
        let n = renumber_folder(dir.path(), 0.5).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn two_pass_avoids_collisions() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "01 - foo.mp3");
        touch(dir.path(), "02 - bar.mp3");
        let n = renumber_folder(dir.path(), 0.5).unwrap();
        assert_eq!(n, 2);
    }

    #[test]
    fn empty_folder_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(renumber_folder(dir.path(), 0.5).unwrap(), 0);
    }

    #[test]
    fn target_collision_restores_source_without_temp_orphan() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "99 - alpha.mp3");
        std::fs::create_dir(dir.path().join("01 - alpha.mp3")).unwrap();

        let n = renumber_folder(dir.path(), 0.5).unwrap();

        assert_eq!(n, 0);
        assert!(dir.path().join("99 - alpha.mp3").exists());
        let names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|r| r.ok())
            .map(|e| e.file_name().into_string().unwrap())
            .collect();
        assert!(!names.iter().any(|n| n.starts_with(".tmp_renumber_")));
    }

    #[test]
    fn rollback_restores_moved_files() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("01 - alpha.mp3");
        let tmp = dir.path().join(".tmp_renumber_alpha.mp3");
        touch(dir.path(), "01 - alpha.mp3");
        std::fs::rename(&original, &tmp).unwrap();

        undo(&[(tmp.clone(), original.clone())]);

        assert!(original.exists());
        assert!(!tmp.exists());
    }
}
