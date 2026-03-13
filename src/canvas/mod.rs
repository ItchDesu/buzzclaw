use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

pub struct CanvasStore {
    pub dir: PathBuf,
}

pub struct Canvas {
    #[allow(dead_code)]
    pub name: String,
    pub content: String,
}

impl CanvasStore {
    pub fn open(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir)?;
        Ok(Self { dir: dir.to_path_buf() })
    }

    pub fn list(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(&self.dir)
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) == Some("md") {
                    p.file_stem().and_then(|n| n.to_str()).map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect();
        names.sort();
        names
    }

    pub fn get(&self, name: &str) -> Option<Canvas> {
        let content = fs::read_to_string(self.path(name)).ok()?;
        Some(Canvas { name: name.to_string(), content })
    }

    pub fn write(&self, name: &str, content: &str) -> Result<()> {
        fs::write(self.path(name), content)?;
        Ok(())
    }

    pub fn append(&self, name: &str, content: &str) -> Result<()> {
        let mut existing = fs::read_to_string(self.path(name)).unwrap_or_default();
        if !existing.is_empty() && !existing.ends_with('\n') {
            existing.push('\n');
        }
        existing.push_str(content);
        fs::write(self.path(name), existing)?;
        Ok(())
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
        self.dir.join(format!("{name}.md"))
    }
}
