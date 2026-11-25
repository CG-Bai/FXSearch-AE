use serde::Deserialize;

use crate::config::{AppSettings, IndexedEffect, UsageStats};
use crate::pinyin_util::{lookup_bilingual, to_pinyin_keys};
use crate::presets::IndexedPreset;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleMode {
    ApplyFx,
    UsedEffects,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxTypeFilter {
    All,
    EffectsOnly,
    PresetsOnly,
}

impl FxTypeFilter {
    pub fn next(self) -> Self {
        match self {
            Self::All => Self::EffectsOnly,
            Self::EffectsOnly => Self::PresetsOnly,
            Self::PresetsOnly => Self::All,
        }
    }

    pub fn label(self, is_en: bool) -> &'static str {
        match (self, is_en) {
            (Self::All, false) => "类型: 全部",
            (Self::EffectsOnly, false) => "类型: 仅插件效果",
            (Self::PresetsOnly, false) => "类型: 仅 .ffx 预设",
            (Self::All, true) => "Type: All",
            (Self::EffectsOnly, true) => "Type: Effects",
            (Self::PresetsOnly, true) => "Type: Presets",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsedScope {
    AllProject,
    ActiveComp,
}

impl UsedScope {
    pub fn toggle(self) -> Self {
        match self {
            Self::AllProject => Self::ActiveComp,
            Self::ActiveComp => Self::AllProject,
        }
    }
}

#[derive(Debug, Clone)]
pub enum CatalogKind {
    Effect { match_name: String },
    Preset { file_path: String, source: String },
}

#[derive(Debug, Clone)]
pub struct CatalogItem {
    pub id: String,
    pub name: String,
    pub secondary_name: String,
    pub category: String,
    pub kind: CatalogKind,
    pub starred: bool,
    pub alias: String,
    name_lower: String,
    sec_lower: String,
    match_lower: String,
    cat_lower: String,
    py_full: String,
    initials: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawUsedEffect {
    #[serde(rename = "compId")]
    pub comp_id: i32,
    #[serde(rename = "compName")]
    pub comp_name: String,
    #[serde(rename = "isActiveComp")]
    pub is_active_comp: bool,
    #[serde(rename = "layerIndex")]
    pub layer_index: i32,
    #[serde(rename = "layerName")]
    pub layer_name: String,
    #[serde(rename = "layerLabel", default)]
    pub layer_label: i32,
    #[serde(rename = "effectIndex")]
    pub effect_index: i32,
    #[serde(rename = "effectName")]
    pub effect_name: String,
    #[serde(rename = "matchName")]
    pub match_name: String,
    #[serde(rename = "effectEnabled", default = "default_true")]
    pub effect_enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone)]
pub struct EnrichedUsedEffect {
    pub raw: RawUsedEffect,
    fx_lower: String,
    bi_lower: String,
    layer_lower: String,
    comp_lower: String,
    match_lower: String,
    py_full: String,
    py_init: String,
}

pub fn enrich_used_effects(raw_items: Vec<RawUsedEffect>) -> Vec<EnrichedUsedEffect> {
    raw_items
        .into_iter()
        .map(|raw| {
            let bi_extra = lookup_bilingual(&raw.match_name)
                .map(|b| format!("{} {}", b.zh_name, b.en_name))
                .unwrap_or_default();
            let combined = format!(
                "{} {} {} {}",
                raw.effect_name, bi_extra, raw.layer_name, raw.comp_name
            );
            let (py_full, py_init) = to_pinyin_keys(&combined);
            EnrichedUsedEffect {
                fx_lower: raw.effect_name.to_lowercase(),
                bi_lower: bi_extra.to_lowercase(),
                layer_lower: raw.layer_name.to_lowercase(),
                comp_lower: raw.comp_name.to_lowercase(),
                match_lower: raw.match_name.to_lowercase(),
                py_full,
                py_init,
                raw,
            }
        })
        .collect()
}

fn extract_word_initials(s: &str) -> String {
    s.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .filter_map(|w| w.chars().next())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

pub fn build_catalog(
    effects: &[IndexedEffect],
    presets: &[IndexedPreset],
    settings: &AppSettings,
) -> Vec<CatalogItem> {
    let mut rev_alias = std::collections::HashMap::new();
    for (alias, target_id) in &settings.aliases {
        rev_alias.insert(target_id.as_str(), alias.clone());
    }
    let fav_set: std::collections::HashSet<&str> =
        settings.favorites.iter().map(|s| s.as_str()).collect();

    let mut out = Vec::with_capacity(effects.len() + presets.len());

    for fx in effects {
        let initials = format!(
            "{} {} {}",
            extract_word_initials(&fx.display_name),
            extract_word_initials(&fx.secondary_name),
            fx.pinyin_initials
        );
        out.push(CatalogItem {
            id: fx.id.clone(),
            name: fx.display_name.clone(),
            secondary_name: fx.secondary_name.clone(),
            category: fx.category.clone(),
            kind: CatalogKind::Effect {
                match_name: fx.match_name.clone(),
            },
            starred: fav_set.contains(fx.id.as_str()),
            alias: rev_alias.get(fx.id.as_str()).cloned().unwrap_or_default(),
            name_lower: fx.display_name.to_lowercase(),
            sec_lower: fx.secondary_name.to_lowercase(),
            match_lower: fx.match_name.to_lowercase(),
            cat_lower: fx.category.to_lowercase(),
            py_full: fx.pinyin_full.clone(),
            initials,
        });
    }

    for pr in presets {
        let initials = format!("{} {}", extract_word_initials(&pr.name), pr.pinyin_initials);
        out.push(CatalogItem {
            id: pr.id.clone(),
            name: pr.name.clone(),
            secondary_name: String::new(),
            category: pr.category.clone(),
            kind: CatalogKind::Preset {
                file_path: pr.file_path.clone(),
                source: pr.source.clone(),
            },
            starred: fav_set.contains(pr.id.as_str()),
            alias: rev_alias.get(pr.id.as_str()).cloned().unwrap_or_default(),
            name_lower: pr.name.to_lowercase(),
            sec_lower: String::new(),
            match_lower: String::new(),
            cat_lower: pr.category.to_lowercase(),
            py_full: pr.pinyin_full.clone(),
            initials,
        });
    }

    out
}

fn fuzzy_subsequence_score(query: &str, target: &str) -> i32 {
    if query.is_empty() || target.len() < query.len() {
        return 0;
    }
    let q_chars: Vec<char> = query.chars().collect();
    let mut qi = 0usize;
    let mut gaps = 0i32;
    let mut last_match: Option<usize> = None;

    for (ti, tc) in target.chars().enumerate() {
        if qi < q_chars.len() && tc == q_chars[qi] {
            if let Some(prev) = last_match {
                if ti > prev + 1 {
                    gaps += (ti - prev - 1) as i32;
                }
            }
            last_match = Some(ti);
            qi += 1;
        }
    }

    if qi < q_chars.len() {
        0
    } else {
        (720 - gaps * 14).max(120)
    }
}

fn score_single_token_fx(token: &str, item: &CatalogItem) -> i32 {
    if !item.alias.is_empty() && item.alias == token {
        return 12000;
    }
    if !item.alias.is_empty() && item.alias.starts_with(token) {
        return 6500;
    }
    if item.name_lower == token || item.sec_lower == token {
        return 5200;
    }
    if item.name_lower.starts_with(token) || (!item.sec_lower.is_empty() && item.sec_lower.starts_with(token)) {
        return 3600;
    }
    if item.initials.split_whitespace().any(|ini| ini == token) {
        return 3500;
    }
    if item.initials.split_whitespace().any(|ini| ini.starts_with(token)) {
        return 2900;
    }
    if !item.py_full.is_empty() && item.py_full.starts_with(token) {
        return 2700;
    }
    if let Some(pos) = item.name_lower.find(token) {
        return 2200 - (pos as i32) * 8;
    }
    if let Some(pos) = item.sec_lower.find(token) {
        return 2050 - (pos as i32) * 8;
    }
    if let Some(pos) = item.py_full.find(token) {
        return 1800 - (pos as i32) * 5;
    }
    if let Some(pos) = item.match_lower.find(token) {
        return 1400 - (pos as i32) * 4;
    }
    if let Some(pos) = item.cat_lower.find(token) {
        return 950 - (pos as i32) * 3;
    }
    let sub1 = fuzzy_subsequence_score(token, &item.name_lower);
    if sub1 > 0 {
        return sub1;
    }
    fuzzy_subsequence_score(token, &item.sec_lower)
}

pub fn filter_catalog(
    catalog: &[CatalogItem],
    query: &str,
    type_filter: FxTypeFilter,
    usage: &UsageStats,
) -> Vec<usize> {
    let tokens: Vec<String> = query
        .trim()
        .to_lowercase()
        .split_whitespace()
        .map(String::from)
        .collect();

    let mut scored: Vec<(usize, i32, u64)> = Vec::new();

    for (idx, item) in catalog.iter().enumerate() {
        match (&item.kind, type_filter) {
            (CatalogKind::Preset { .. }, FxTypeFilter::EffectsOnly) => continue,
            (CatalogKind::Effect { .. }, FxTypeFilter::PresetsOnly) => continue,
            _ => {}
        }

        let mut score = if tokens.is_empty() {
            100
        } else {
            let mut sum = 0;
            let mut ok = true;
            for tk in &tokens {
                let s = score_single_token_fx(tk, item);
                if s <= 0 {
                    ok = false;
                    break;
                }
                sum += s;
            }
            if !ok {
                continue;
            }
            sum
        };

        if item.starred {
            score += 550;
        }
        let use_cnt = usage.counts.get(&item.id).copied().unwrap_or(0);
        if use_cnt > 0 {
            score += ((use_cnt as i32) * 42).min(420);
        }
        let lu = usage.last_used.get(&item.id).copied().unwrap_or(0);
        if lu > 0 && tokens.is_empty() {
            score += 200;
        }

        match &item.kind {
            CatalogKind::Effect { .. } => score += 35,
            CatalogKind::Preset { source, .. } if source == "user" || source == "custom" => {
                score += 65
            }
            _ => {}
        }

        scored.push((idx, score, lu));
    }

    scored.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| b.2.cmp(&a.2))
            .then_with(|| catalog[a.0].name.cmp(&catalog[b.0].name))
    });

    scored.into_iter().take(80).map(|x| x.0).collect()
}

pub fn filter_used_effects(
    items: &[EnrichedUsedEffect],
    query: &str,
    scope: UsedScope,
) -> Vec<usize> {
    let tokens: Vec<String> = query
        .trim()
        .to_lowercase()
        .split_whitespace()
        .map(String::from)
        .collect();

    let mut scored: Vec<(usize, i32)> = Vec::new();

    for (idx, u) in items.iter().enumerate() {
        if scope == UsedScope::ActiveComp && !u.raw.is_active_comp {
            continue;
        }

        let score = if tokens.is_empty() {
            if u.raw.is_active_comp {
                500
            } else {
                200
            }
        } else {
            let mut sum = 0;
            let mut ok = true;
            for tk in &tokens {
                let s = if u.fx_lower == *tk {
                    4500
                } else if u.fx_lower.starts_with(tk) {
                    3400
                } else if u.py_init.contains(tk) {
                    2800
                } else if u.fx_lower.contains(tk) {
                    2400
                } else if u.bi_lower.contains(tk) {
                    2200
                } else if u.py_full.contains(tk) {
                    1800
                } else if u.layer_lower.contains(tk) {
                    1500
                } else if u.comp_lower.contains(tk) {
                    1200
                } else if u.match_lower.contains(tk) {
                    900
                } else {
                    fuzzy_subsequence_score(tk, &format!("{} {}", u.fx_lower, u.bi_lower))
                };

                if s <= 0 {
                    ok = false;
                    break;
                }
                sum += s;
            }
            if !ok {
                continue;
            }
            if u.raw.is_active_comp {
                sum + 180
            } else {
                sum
            }
        };

        scored.push((idx, score));
    }

    scored.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| items[b.0].raw.is_active_comp.cmp(&items[a.0].raw.is_active_comp))
            .then_with(|| items[a.0].raw.comp_name.cmp(&items[b.0].raw.comp_name))
            .then_with(|| items[a.0].raw.layer_index.cmp(&items[b.0].raw.layer_index))
            .then_with(|| items[a.0].raw.effect_index.cmp(&items[b.0].raw.effect_index))
    });

    scored.into_iter().take(100).map(|x| x.0).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::load_initial_effects;

    #[test]
    fn test_bilingual_and_pinyin_search() {
        let (fx, _) = load_initial_effects();
        let settings = AppSettings::default();
        let usage = UsageStats::default();
        let cat = build_catalog(&fx, &[], &settings);

        // Search by Pinyin initials "gsmh" -> 高斯模糊
        let res_py = filter_catalog(&cat, "gsmh", FxTypeFilter::All, &usage);
        assert!(!res_py.is_empty());
        assert_eq!(cat[res_py[0]].name, "高斯模糊");

        // Search by English "gaussian" -> 高斯模糊
        let res_en = filter_catalog(&cat, "gaussian", FxTypeFilter::All, &usage);
        assert!(!res_en.is_empty());
        assert_eq!(cat[res_en[0]].name, "高斯模糊");
    }
}
