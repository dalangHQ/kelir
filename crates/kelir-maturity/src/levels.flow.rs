// see: cans/01-architecture.md#Codebase-invariants
use crate::Level;
use std::fmt;
use std::path::{Path, PathBuf};

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Level::L1 => write!(f, "L1"),
            Level::L2 => write!(f, "L2"),
            Level::L3 => write!(f, "L3"),
        }
    }
}

impl Level {
    pub fn from_name(name: &str) -> Option<Level> {
        match name {
            "L1" => Some(Level::L1),
            "L2" => Some(Level::L2),
            "L3" => Some(Level::L3),
            _ => None,
        }
    }
}

pub fn level_of(path: &Path) -> Level {
    let name = match path.file_name() {
        Some(n) => n.to_string_lossy().into_owned(),
        None => String::new(),
    };
    if name.ends_with(".flow.rs") {
        Level::L2
    } else if name.ends_with(".run.rs") {
        Level::L3
    } else {
        Level::L1
    }
}

pub fn crate_of(path: &Path) -> PathBuf {
    let mut dir = match path.parent() {
        Some(d) => d.to_path_buf(),
        None => return PathBuf::new(),
    };
    loop {
        if dir.join("Cargo.toml").is_file() {
            return dir;
        }
        match dir.parent() {
            Some(p) => dir = p.to_path_buf(),
            None => return PathBuf::new(),
        }
    }
}
