use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct PluginConfig {
    pub add: bool,
    pub remove: bool,
    pub single_quotes: bool,
    pub regexp: Option<String>,
    pub rename: Option<Vec<RenameEntry>>,
    pub enable: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RenameEntry {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Add,
    Remove,
    Rebuild,
}

impl PluginConfig {
    pub fn mode(&self) -> Option<Mode> {
        match (self.add, self.remove) {
            (true, true) => Some(Mode::Rebuild),
            (true, false) => Some(Mode::Add),
            (false, true) => Some(Mode::Remove),
            _ => None,
        }
    }

    pub fn quote_char(&self) -> char {
        if self.single_quotes {
            '\''
        } else {
            '"'
        }
    }

    pub fn rename_map(&self) -> HashMap<String, String> {
        self.rename
            .as_ref()
            .map(|v| {
                v.iter()
                    .map(|e| (e.from.clone(), e.to.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn is_adf_enabled(&self) -> bool {
        self.enable
            .as_ref()
            .map(|v| {
                v.iter()
                    .any(|s| s == "angular-dashboard-framework")
            })
            .unwrap_or(false)
    }
}
