use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::pinyin_util::{lookup_bilingual, to_pinyin_keys, BUILTIN_AE_EFFECTS};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// UI language: "zh" (Simplified Chinese) or "en" (English)
    #[serde(default = "default_language")]
    pub language: String,
    /// Hotkey to open FXSearch in "Apply FX / Presets" mode (default: "Ctrl+Space")
    pub hotkey_fx: String,
    /// Hotkey to open FXSearch directly in "Used Effects Finder" mode (default: "Ctrl+Shift+Space")
    pub hotkey_used: String,
    /// Legacy compatibility flag
    #[serde(default = "default_true_bool")]
    pub only_in_ae: bool,
    /// Default search scope in Used Effects mode: "all" (All Project Comps) or "active" (Active Comp)
    pub default_used_scope: String,
    /// Additional user-configured folders to scan for .ffx animation presets
    #[serde(default)]
    pub custom_preset_dirs: Vec<String>,
    /// User short aliases: alias -> item key (matchName or preset filePath)
    #[serde(default)]
    pub aliases: HashMap<String, String>,
    /// Pinned favorite item keys
    #[serde(default)]
    pub favorites: Vec<String>,
}

fn default_language() -> String {
    "zh".to_string()
}

fn default_true_bool() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: "zh".to_string(),
            hotkey_fx: "Ctrl+Space".to_string(),
            hotkey_used: "Ctrl+Shift+Space".to_string(),
            only_in_ae: true,
            default_used_scope: "all".to_string(),
            custom_preset_dirs: Vec::new(),
            aliases: HashMap::new(),
            favorites: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageStats {
    pub counts: HashMap<String, u32>,
    pub last_used: HashMap<String, u64>,
}

impl UsageStats {
    pub fn record_use(&mut self, key: &str) {
        let count = self.counts.entry(key.to_string()).or_insert(0);
        *count = count.saturating_add(1);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.last_used.insert(key.to_string(), now);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawAeEffect {
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "matchName")]
    pub match_name: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectsCacheFile {
    #[serde(rename = "aeVersion", default)]
    pub ae_version: String,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: u64,
    #[serde(default)]
    pub effects: Vec<RawAeEffect>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedEffect {
    pub id: String,
    pub display_name: String,
    pub secondary_name: String,
    pub match_name: String,
    pub category: String,
    pub pinyin_full: String,
    pub pinyin_initials: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct BootstrapConfig {
    #[serde(default)]
    custom_data_dir: String,
}

fn get_default_bootstrap_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("FXSearch");
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn get_app_data_dir() -> PathBuf {
    let boot_dir = get_default_bootstrap_dir();
    let boot_file = boot_dir.join("storage-location.json");
    if let Ok(txt) = fs::read_to_string(&boot_file) {
        if let Ok(cfg) = serde_json::from_str::<BootstrapConfig>(&txt) {
            let trimmed = cfg.custom_data_dir.trim();
            if !trimmed.is_empty() {
                let custom_path = PathBuf::from(trimmed);
                if fs::create_dir_all(&custom_path).is_ok() {
                    return custom_path;
                }
            }
        }
    }
    boot_dir
}

/// Migrate existing config/cache files to a new user-selected directory and persist the custom location.
pub fn set_custom_app_data_dir(new_dir: &std::path::Path) -> bool {
    if fs::create_dir_all(new_dir).is_err() {
        return false;
    }
    let old_dir = get_app_data_dir();
    if old_dir != new_dir {
        for file_name in ["settings.json", "usage.json", "effects-cache.json"] {
            let src = old_dir.join(file_name);
            let dst = new_dir.join(file_name);
            if src.exists() && !dst.exists() {
                let _ = fs::copy(&src, &dst);
            }
        }
    }

    let boot_dir = get_default_bootstrap_dir();
    let boot_file = boot_dir.join("storage-location.json");
    let default_same = new_dir == boot_dir;
    let cfg = BootstrapConfig {
        custom_data_dir: if default_same {
            String::new()
        } else {
            new_dir.to_string_lossy().into_owned()
        },
    };
    if let Ok(json_str) = serde_json::to_string_pretty(&cfg) {
        return fs::write(boot_file, json_str).is_ok();
    }
    false
}

pub fn load_settings() -> AppSettings {
    let path = get_app_data_dir().join("settings.json");
    if let Ok(content) = fs::read_to_string(path) {
        if let Ok(parsed) = serde_json::from_str::<AppSettings>(&content) {
            return parsed;
        }
    }
    let default = AppSettings::default();
    save_settings(&default);
    default
}

pub fn save_settings(settings: &AppSettings) {
    let path = get_app_data_dir().join("settings.json");
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = fs::write(path, json);
    }
}

pub fn load_usage() -> UsageStats {
    let path = get_app_data_dir().join("usage.json");
    if let Ok(content) = fs::read_to_string(path) {
        if let Ok(parsed) = serde_json::from_str::<UsageStats>(&content) {
            return parsed;
        }
    }
    UsageStats::default()
}

pub fn save_usage(usage: &UsageStats) {
    let path = get_app_data_dir().join("usage.json");
    if let Ok(json) = serde_json::to_string_pretty(usage) {
        let _ = fs::write(path, json);
    }
}

pub fn enrich_effects(raw_list: &[RawAeEffect]) -> Vec<IndexedEffect> {
    let mut out = Vec::with_capacity(raw_list.len());
    let mut seen = std::collections::HashSet::new();

    for raw in raw_list {
        if raw.match_name.is_empty() || raw.display_name.is_empty() {
            continue;
        }
        if !seen.insert(raw.match_name.clone()) {
            continue;
        }

        let mut secondary = String::new();
        let mut combined_for_pinyin = raw.display_name.clone();

        if let Some(bi) = lookup_bilingual(&raw.match_name) {
            if raw.display_name != bi.zh_name && raw.display_name != bi.en_name {
                secondary = format!("{} / {}", bi.zh_name, bi.en_name);
            } else if raw.display_name == bi.zh_name {
                secondary = bi.en_name.to_string();
            } else {
                secondary = bi.zh_name.to_string();
            }
            combined_for_pinyin.push(' ');
            combined_for_pinyin.push_str(bi.zh_name);
        }

        let (py_full, py_init) = to_pinyin_keys(&combined_for_pinyin);
        out.push(IndexedEffect {
            id: format!("fx:{}", raw.match_name),
            display_name: raw.display_name.clone(),
            secondary_name: secondary,
            match_name: raw.match_name.clone(),
            category: raw.category.clone(),
            pinyin_full: py_full,
            pinyin_initials: py_init,
        });
    }

    out
}

pub fn load_initial_effects() -> (Vec<IndexedEffect>, bool) {
    let cache_path = get_app_data_dir().join("effects-cache.json");
    if let Ok(content) = fs::read_to_string(cache_path) {
        if let Ok(cache) = serde_json::from_str::<EffectsCacheFile>(&content) {
            if !cache.effects.is_empty() {
                // Return `false` for `synced_this_session` so `idle_hook` ALWAYS refreshes `app.effects`
                // automatically on every AE launch (picking up newly installed/uninstalled plugins automatically!),
                // while still giving 0ms instant warm startup from cache!
                return (enrich_effects(&cache.effects), false);
            }
        }
    }

    // Fallback to rich built-in AE catalog until AE syncs full `app.effects`
    let fallback_raw: Vec<RawAeEffect> = BUILTIN_AE_EFFECTS
        .iter()
        .map(|b| RawAeEffect {
            display_name: b.zh_name.to_string(),
            match_name: b.match_name.to_string(),
            category: b.category.to_string(),
            version: String::new(),
        })
        .collect();
    (enrich_effects(&fallback_raw), false)
}
