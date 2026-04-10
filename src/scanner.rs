use anyhow::{Context, Result};
use regex::Regex;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Deserialize, Default)]
pub struct Frontmatter {
    #[serde(rename = "type")]
    pub note_type: Option<String>,
    pub status: Option<String>,
    pub created: Option<String>,
    pub updated: Option<String>,
    pub project: Option<String>,
}

#[derive(Debug)]
pub struct VaultNote {
    pub path: PathBuf,
    pub rel_path: String,
    pub stem: String,
    pub frontmatter: Frontmatter,
    pub body: String,
    pub wikilinks: Vec<String>,
}

pub fn scan_vault(vault_path: &Path, ignore_dirs: &[String]) -> Result<Vec<VaultNote>> {
    let wikilink_re = Regex::new(r"\[\[([^\]|]+)(?:\|[^\]]+)?\]\]").unwrap();
    let mut notes = Vec::new();

    for entry in WalkDir::new(vault_path).into_iter().filter_entry(|e| {
        if e.file_type().is_dir() {
            let name = e.file_name().to_string_lossy();
            !ignore_dirs.iter().any(|d| d == name.as_ref())
        } else {
            true
        }
    }) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let rel_path = path
            .strip_prefix(vault_path)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let stem = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let (frontmatter, body) = parse_frontmatter(&content);

        let wikilinks: Vec<String> = wikilink_re
            .captures_iter(&content)
            .map(|cap| cap[1].to_string())
            .collect();

        notes.push(VaultNote {
            path: path.to_path_buf(),
            rel_path,
            stem,
            frontmatter,
            body,
            wikilinks,
        });
    }

    Ok(notes)
}

fn parse_frontmatter(content: &str) -> (Frontmatter, String) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (Frontmatter::default(), content.to_string());
    }

    let after_open = &trimmed[3..];
    if let Some(close_pos) = after_open.find("\n---") {
        let yaml_str = &after_open[..close_pos];
        let body_start = close_pos + 4;
        let body = after_open[body_start..]
            .trim_start_matches('\n')
            .to_string();

        match serde_yaml::from_str::<Frontmatter>(yaml_str) {
            Ok(fm) => (fm, body),
            Err(_) => (Frontmatter::default(), content.to_string()),
        }
    } else {
        (Frontmatter::default(), content.to_string())
    }
}

/// Build a map of note stem -> relative paths for link resolution
pub fn build_stem_index(notes: &[VaultNote]) -> HashMap<String, Vec<String>> {
    let mut index: HashMap<String, Vec<String>> = HashMap::new();
    for note in notes {
        index
            .entry(note.stem.clone())
            .or_default()
            .push(note.rel_path.clone());

        // Also index by relative path without .md
        let without_ext = note.rel_path.strip_suffix(".md").unwrap_or(&note.rel_path);
        index
            .entry(without_ext.to_string())
            .or_default()
            .push(note.rel_path.clone());
    }
    index
}

/// Build a set of all inbound links per note stem
pub fn build_backlink_index(notes: &[VaultNote]) -> HashMap<String, HashSet<String>> {
    let mut backlinks: HashMap<String, HashSet<String>> = HashMap::new();
    for note in notes {
        for link in &note.wikilinks {
            // Extract the stem from the link (could be path/stem or just stem)
            let link_stem = link.rsplit('/').next().unwrap_or(link);
            backlinks
                .entry(link_stem.to_string())
                .or_default()
                .insert(note.stem.clone());
        }
    }
    backlinks
}
