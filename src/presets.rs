use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::pinyin_util::to_pinyin_keys;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedPreset {
    pub id: String,
    pub name: String,
    pub category: String,
    pub file_path: String,
    /// "user" | "custom" | "builtin"
    pub source: String,
    pub pinyin_full: String,
    pub pinyin_initials: String,
}

fn scan_ffx_dir(root: &Path, source_tag: &str, seen_paths: &mut HashSet<String>, out: &mut Vec<IndexedPreset>) {
    if !root.exists() {
        return;
    }

    for entry in WalkDir::new(root)
        .follow_links(false)
        .max_depth(5)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let is_ffx = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|s| s.eq_ignore_ascii_case("ffx"))
            .unwrap_or(false);
        if !is_ffx {
            continue;
        }

        let full_path_str = path.to_string_lossy().replace('\\', "/");
        let norm_key = full_path_str.to_lowercase();
        if !seen_paths.insert(norm_key) {
            continue;
        }

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Preset")
            .to_string();

        let category = path
            .parent()
            .and_then(|p| p.strip_prefix(root).ok())
            .map(|rel| {
                let rel_s = rel.to_string_lossy().replace('\\', " / ");
                if rel_s.is_empty() {
                    if source_tag == "user" {
                        "User Presets".to_string()
                    } else {
                        "Presets".to_string()
                    }
                } else {
                    rel_s
                }
            })
            .unwrap_or_else(|| "Presets".to_string());

        let combined = format!("{} {}", name, category);
        let (py_full, py_init) = to_pinyin_keys(&combined);

        out.push(IndexedPreset {
            id: format!("ffx:{}", full_path_str),
            name,
            category,
            file_path: full_path_str,
            source: source_tag.to_string(),
            pinyin_full: py_full,
            pinyin_initials: py_init,
        });
    }
}

use std::time::UNIX_EPOCH;

/// Fast (<0.1ms) directory modification fingerprint across User Presets & custom preset folders
/// so newly saved `.ffx` presets are detected and synced automatically when opening FXSearch!
pub fn compute_presets_dir_stamp(custom_dirs: &[String]) -> u64 {
    let mut hash: u64 = 14695981039346656037;
    let mut mix = |p: &Path| {
        if let Ok(meta) = fs::metadata(p) {
            if let Ok(m) = meta.modified() {
                let secs = m.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                hash ^= secs;
                hash = hash.wrapping_mul(1099511628211);
            }
        }
    };

    for dir_str in custom_dirs {
        let t = dir_str.trim();
        if !t.is_empty() {
            mix(Path::new(t));
        }
    }

    if let Some(user_profile) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
        let docs_adobe = user_profile.join("Documents").join("Adobe");
        if let Ok(entries) = fs::read_dir(&docs_adobe) {
            for e in entries.filter_map(|x| x.ok()) {
                let up = e.path().join("User Presets");
                if up.exists() {
                    mix(&up);
                }
            }
        }
    }

    hash
}

/// Discover user Documents\Adobe\After Effects *\User Presets + newest installed AE Support Files\Presets + custom dirs
pub fn scan_all_presets(custom_dirs: &[String]) -> Vec<IndexedPreset> {
    let mut results = Vec::new();
    let mut seen_paths = HashSet::new();

    // 1. Custom user-configured directories (highest priority)
    for dir_str in custom_dirs {
        let trimmed = dir_str.trim();
        if !trimmed.is_empty() {
            scan_ffx_dir(Path::new(trimmed), "custom", &mut seen_paths, &mut results);
        }
    }

    // 2. User Presets inside %USERPROFILE%\Documents\Adobe\After Effects *\User Presets
    if let Some(user_profile) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
        let docs_adobe = user_profile.join("Documents").join("Adobe");
        if let Ok(entries) = fs::read_dir(&docs_adobe) {
            let mut ae_user_dirs: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.starts_with("After Effects"))
                        .unwrap_or(false)
                })
                .collect();
            // Sort descending so newer AE User Presets take precedence on identical names
            ae_user_dirs.sort_by(|a, b| b.cmp(a));
            for dir in ae_user_dirs {
                scan_ffx_dir(&dir.join("User Presets"), "user", &mut seen_paths, &mut results);
            }
        }
    }

    // 3. Built-in Presets from the newest installed Adobe After Effects version on C:\Program Files\Adobe
    let program_adobe = Path::new(r"C:\Program Files\Adobe");
    if let Ok(entries) = fs::read_dir(program_adobe) {
        let mut ae_install_dirs: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.starts_with("Adobe After Effects"))
                    .unwrap_or(false)
                    && p.join("Support Files").join("Presets").exists()
            })
            .collect();
        ae_install_dirs.sort_by(|a, b| b.cmp(a));
        if let Some(newest_ae) = ae_install_dirs.first() {
            let presets_dir = newest_ae.join("Support Files").join("Presets");
            scan_ffx_dir(&presets_dir, "builtin", &mut seen_paths, &mut results);
        }
    }

    results
}
