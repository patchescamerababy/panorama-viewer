// rust/src/i18n.rs
//
// Lightweight runtime i18n:
// - Strings live in either:
//   A) assets/i18n/<lang>.json
//   B) assets/i18n.json (single file, format: { "<lang>": { "key": "value" } })
// - Load order: selected lang -> fallback zh-Hans
// - Lookup: tr(\"key\") / tr_with(\"key\", [(\"name\", \"...\")]) with {name} placeholders
//
// Language selection:
// - CLI: --lang <code> (e.g. en, zh-Hant, ja, ko, fr, ru, ar)
// - Env: PANORAMA_LANG
// - Default: zh-Hans

use once_cell::sync::OnceCell;
use serde::Deserialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::RwLock,
};

#[derive(Debug, Clone)]
pub struct I18n {
    pub lang: String,
    fallback_lang: String,
    map: HashMap<String, String>,
    fallback_map: HashMap<String, String>,
}

static I18N: OnceCell<RwLock<I18n>> = OnceCell::new();

fn load_json_map(path: &Path) -> Option<HashMap<String, String>> {
    let text = std::fs::read_to_string(path).ok()?;
    let map: HashMap<String, String> = serde_json::from_str(&text).ok()?;
    Some(map)
}

fn load_multi_lang_json(path: &Path, lang: &str) -> Option<HashMap<String, String>> {
    let text = std::fs::read_to_string(path).ok()?;
    let all: HashMap<String, HashMap<String, String>> = serde_json::from_str(&text).ok()?;
    all.get(lang).cloned()
}

/// Find assets/i18n/<lang>.json by searching:
/// 1) <exe_dir>/assets/i18n/<lang>.json
/// 2) ./assets/i18n/<lang>.json  (dev working dir)
fn find_lang_file(lang: &str) -> Option<PathBuf> {
    let file = format!(\"{}.json\", lang);

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join(\"assets\").join(\"i18n\").join(&file);
            if p.exists() {
                return Some(p);
            }
        }
    }

    let p = PathBuf::from(\"assets\").join(\"i18n\").join(&file);
    if p.exists() {
        return Some(p);
    }

    None
}

/// Find assets/i18n.json (single file) by searching:
/// 1) <exe_dir>/assets/i18n.json
/// 2) ./assets/i18n.json
fn find_multi_lang_file() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join(\"assets\").join(\"i18n.json\");
            if p.exists() {
                return Some(p);
            }
        }
    }

    let p = PathBuf::from(\"assets\").join(\"i18n.json\");
    if p.exists() {
        return Some(p);
    }

    None
}

fn load_lang(lang: &str) -> HashMap<String, String> {
    // First try per-lang file
    if let Some(p) = find_lang_file(lang) {
        if let Some(m) = load_json_map(&p) {
            return m;
        }
    }

    // Then try single multi-lang file
    if let Some(p) = find_multi_lang_file() {
        if let Some(m) = load_multi_lang_json(&p, lang) {
            return m;
        }
    }

    HashMap::new()
}

/// Initialize global i18n. Safe to call multiple times; later calls overwrite current lang maps.
pub fn init(lang: impl Into<String>) {
    let lang = lang.into();
    let fallback_lang = \"zh-Hans\".to_string();

    let map = load_lang(&lang);
    let fallback_map = if lang == fallback_lang {
        map.clone()
    } else {
        load_lang(&fallback_lang)
    };

    let i = I18n {
        lang,
        fallback_lang,
        map,
        fallback_map,
    };

    if let Some(lock) = I18N.get() {
        if let Ok(mut w) = lock.write() {
            *w = i;
        }
    } else {
        let _ = I18N.set(RwLock::new(i));
    }
}

fn get_locked() -> Option<std::sync::RwLockReadGuard<'static, I18n>> {
    I18N.get().and_then(|l| l.read().ok())
}

/// Get localized text by key. If key missing, returns key itself.
pub fn tr(key: &str) -> String {
    let Some(i) = get_locked() else {
        return key.to_string();
    };

    if let Some(v) = i.map.get(key) {
        return v.clone();
    }
    if let Some(v) = i.fallback_map.get(key) {
        return v.clone();
    }
    key.to_string()
}

/// Get localized text and substitute `{name}` placeholders.
/// Any placeholder not provided is kept as-is.
pub fn tr_with(key: &str, args: &[(&str, String)]) -> String {
    let mut s = tr(key);
    for (k, v) in args {
        let placeholder = format!(\"{{{}}}\", k);
        s = s.replace(&placeholder, v);
    }
    s
}

/// Choose language from CLI/env.
pub fn resolve_lang_from_args() -> String {
    // CLI: --lang <code>
    let mut it = std::env::args();
    while let Some(a) = it.next() {
        if a == \"--lang\" {
            if let Some(v) = it.next() {
                return v;
            }
        }
    }

    // Env: PANORAMA_LANG
    if let Ok(v) = std::env::var(\"PANORAMA_LANG\") {
        if !v.trim().is_empty() {
            return v;
        }
    }

    \"zh-Hans\".to_string()
}
