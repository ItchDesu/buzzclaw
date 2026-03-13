use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

use crate::providers::ChatMessage;

pub struct SessionStore {
    pub dir: PathBuf,
}

impl SessionStore {
    pub fn open(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir)?;
        Ok(Self { dir: dir.to_path_buf() })
    }

    pub fn load(&self, name: &str) -> Vec<ChatMessage> {
        fs::read_to_string(self.path(name))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, name: &str, history: &[ChatMessage]) -> Result<()> {
        let json = serde_json::to_string_pretty(history)?;
        fs::write(self.path(name), json)?;
        Ok(())
    }

    pub fn list(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(&self.dir)
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) == Some("json") {
                    p.file_stem().and_then(|n| n.to_str()).map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect();
        names.sort();
        names
    }

    pub fn delete(&self, name: &str) -> Result<bool> {
        let p = self.path(name);
        if p.exists() {
            fs::remove_file(p)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.json"))
    }

    pub fn size(&self, name: &str) -> usize {
        self.load(name).len()
    }
}
