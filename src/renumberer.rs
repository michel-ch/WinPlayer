use crate::data::scanner::is_audio_path;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub fn renumber_folder(folder: &Path, threshold: f32) -> std::io::Result<u32> {
    let mut audio_files: Vec<PathBuf> = std::fs::read_dir(folder)?
        .filter_map(|r| r.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_audio_path(p))
        .collect();

    if audio_files.is_empty() { return Ok(0); }
    audio_files.sort();

    let prefixed = audio_files.iter()
        .filter(|p| has_track_prefix(p))
        .count();
    let ratio = prefixed as f32 / audio_files.len() as f32;
    if ratio < threshold { return Ok(0); }

    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let mut temp_paths: Vec<(PathBuf, PathBuf, u32)> = Vec::with_capacity(audio_files.len());
    for (i, src) in audio_files.iter().enumerate() {
        let stem_clean = strip_existing_prefix(src);
        let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("").to_string();
        let tmp_name = format!(".tmp_renumber_{}_{}_{}.{}", nanos, i, sanitize(&stem_clean), ext);
        let tmp = folder.join(tmp_name);
        std::fs::rename(src, &tmp)?;
        temp_paths.push((tmp, PathBuf::from(stem_clean), (i + 1) as u32));
    }

    let mut renamed = 0u32;
    for (tmp, stem, idx) in temp_paths {
        let ext = tmp.extension().and_then(|e| e.to_str()).unwrap_or("").to_string();
        let stem_str = stem.to_string_lossy();
        let final_name = if ext.is_empty() {
            format!("{:02} - {}", idx, stem_str)
        } else {
            format!("{:02} - {}.{}", idx, stem_str, ext)
        };
        let final_path = folder.join(final_name);
        std::fs::rename(&tmp, &final_path)?;
        renamed += 1;
    }
    Ok(renamed)
}

fn has_track_prefix(path: &Path) -> bool {
    let stem = path.file_stem().map(|s| s.to_string_lossy()).unwrap_or_default();
    let mut chars = stem.chars();
    let mut digits = 0;
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() { digits += 1; }
        else {
            return digits > 0 && (c == ' ' || c == '-' || c == '_' || c == '.');
        }
    }
    false
}

fn strip_existing_prefix(path: &Path) -> String {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
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
        let names: Vec<String> = std::fs::read_dir(dir.path()).unwrap()
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
}
