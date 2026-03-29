use crate::config::Config;
use crate::scanner::{build_backlink_index, build_stem_index, VaultNote};
use chrono::{Local, NaiveDate};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug)]
pub struct Issue {
    pub severity: Severity,
    pub category: Category,
    pub note: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Category {
    BrokenLink,
    Orphan,
    Stale,
    MissingHub,
    MissingFrontmatter,
    Duplicate,
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Category::BrokenLink => write!(f, "broken-link"),
            Category::Orphan => write!(f, "orphan"),
            Category::Stale => write!(f, "stale"),
            Category::MissingHub => write!(f, "missing-hub"),
            Category::MissingFrontmatter => write!(f, "frontmatter"),
            Category::Duplicate => write!(f, "duplicate"),
        }
    }
}

pub fn check_broken_links(notes: &[VaultNote]) -> Vec<Issue> {
    let stem_index = build_stem_index(notes);
    let mut issues = Vec::new();

    for note in notes {
        for link in &note.wikilinks {
            let link_stem = link.rsplit('/').next().unwrap_or(link);

            // Check if the link resolves to any note
            let found = stem_index.contains_key(link)
                || stem_index.contains_key(link_stem);

            if !found {
                issues.push(Issue {
                    severity: Severity::Error,
                    category: Category::BrokenLink,
                    note: note.rel_path.clone(),
                    message: format!("broken wikilink [[{}]]", link),
                });
            }
        }
    }

    issues
}

pub fn check_orphans(notes: &[VaultNote]) -> Vec<Issue> {
    let backlinks = build_backlink_index(notes);
    let mut issues = Vec::new();

    // Index notes (like dashboard, tasks.md, projects.md) are expected to have no inbound links
    let index_names: HashSet<&str> = [
        "dashboard", "tasks", "outputs", "projects", "logs", "math",
        "systems", "dev", "business", "agents", "assets", "context",
        "memory", "templates", "meta", "README",
    ]
    .iter()
    .copied()
    .collect();

    for note in notes {
        if index_names.contains(note.stem.as_str()) {
            continue;
        }

        // Skip notes that are in the root (like Welcome.md)
        if !note.rel_path.contains('/') && note.stem == "Welcome" {
            continue;
        }

        let has_backlinks = backlinks
            .get(&note.stem)
            .is_some_and(|links| !links.is_empty());

        if !has_backlinks {
            issues.push(Issue {
                severity: Severity::Info,
                category: Category::Orphan,
                note: note.rel_path.clone(),
                message: "no inbound wikilinks from other notes".to_string(),
            });
        }
    }

    issues
}

pub fn check_stale(notes: &[VaultNote], stale_days: i64) -> Vec<Issue> {
    let today = Local::now().date_naive();
    let mut issues = Vec::new();

    for note in notes {
        let status = note.frontmatter.status.as_deref().unwrap_or("");
        if status != "active" && status != "in_progress" {
            continue;
        }

        let date_str = note
            .frontmatter
            .updated
            .as_deref()
            .or(note.frontmatter.created.as_deref());

        if let Some(date_str) = date_str {
            if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                let days_old = (today - date).num_days();
                if days_old > stale_days {
                    issues.push(Issue {
                        severity: Severity::Warning,
                        category: Category::Stale,
                        note: note.rel_path.clone(),
                        message: format!(
                            "status is '{}' but last updated {} days ago",
                            status, days_old
                        ),
                    });
                }
            }
        }
    }

    issues
}

pub fn check_missing_hubs(config: &Config) -> Vec<Issue> {
    let projects_path = std::fs::read_dir(config.vault_path().join(config.projects_dir()));
    let mut issues = Vec::new();

    // Get all project directories under /Projects (the code directory, not obsidian)
    let vault_path = config.vault_path();
    let code_projects_dir = vault_path.parent().unwrap_or(Path::new("/"));

    let mut hub_names: HashSet<String> = HashSet::new();
    if let Ok(entries) = projects_path {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".md") {
                let stem = name.strip_suffix(".md").unwrap_or(&name);
                hub_names.insert(stem.to_string());
            } else if entry.path().is_dir() {
                hub_names.insert(name);
            }
        }
    }

    // Check each directory in the code projects dir
    if let Ok(entries) = std::fs::read_dir(code_projects_dir) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip obsidian_docs itself and hidden dirs
            if name == "obsidian_docs" || name.starts_with('.') || name == "documents" || name == "math" {
                continue;
            }

            if !hub_names.contains(&name) {
                issues.push(Issue {
                    severity: Severity::Warning,
                    category: Category::MissingHub,
                    note: name.clone(),
                    message: format!(
                        "project directory '{}' exists but no project hub in {}",
                        name,
                        config.projects_dir()
                    ),
                });
            }
        }
    }

    issues
}

pub fn check_frontmatter(notes: &[VaultNote]) -> Vec<Issue> {
    let mut issues = Vec::new();

    for note in notes {
        // Skip index files
        if note.stem == "tasks"
            || note.stem == "outputs"
            || note.stem == "projects"
            || note.stem == "Welcome"
        {
            continue;
        }

        let note_type = note.frontmatter.note_type.as_deref().unwrap_or("");

        // Notes in tasks/ should have type: task
        if note.rel_path.starts_with("tasks/") && note.stem != "tasks" {
            if note_type.is_empty() {
                issues.push(Issue {
                    severity: Severity::Warning,
                    category: Category::MissingFrontmatter,
                    note: note.rel_path.clone(),
                    message: "task note missing 'type' in frontmatter".to_string(),
                });
            }
            if note.frontmatter.status.is_none() {
                issues.push(Issue {
                    severity: Severity::Warning,
                    category: Category::MissingFrontmatter,
                    note: note.rel_path.clone(),
                    message: "task note missing 'status' in frontmatter".to_string(),
                });
            }
        }

        // Notes in outputs/ should have type: output
        if note.rel_path.starts_with("outputs/") && note.stem != "outputs" {
            if note_type.is_empty() {
                issues.push(Issue {
                    severity: Severity::Warning,
                    category: Category::MissingFrontmatter,
                    note: note.rel_path.clone(),
                    message: "output note missing 'type' in frontmatter".to_string(),
                });
            }
        }

        // Project hubs should have type: project
        if note.rel_path.starts_with("01-projects/") && note.stem != "projects" {
            if note_type.is_empty() {
                issues.push(Issue {
                    severity: Severity::Warning,
                    category: Category::MissingFrontmatter,
                    note: note.rel_path.clone(),
                    message: "project hub missing 'type' in frontmatter".to_string(),
                });
            }
        }
    }

    issues
}

pub fn check_duplicates(notes: &[VaultNote]) -> Vec<Issue> {
    let stem_index = build_stem_index(notes);
    let mut issues = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for (stem, paths) in &stem_index {
        // The stem index includes both stem and path-without-ext entries, so filter to unique paths
        if paths.len() > 1 && !seen.contains(stem) {
            // Check if they're actually different files (not just the same file indexed twice)
            let unique_paths: HashSet<&String> = paths.iter().collect();
            if unique_paths.len() > 1 {
                issues.push(Issue {
                    severity: Severity::Info,
                    category: Category::Duplicate,
                    note: stem.clone(),
                    message: format!(
                        "multiple notes share the stem '{}': {}",
                        stem,
                        paths.join(", ")
                    ),
                });
                seen.insert(stem.clone());
            }
        }
    }

    issues
}

pub fn run_all_checks(notes: &[VaultNote], config: &Config) -> Vec<Issue> {
    let mut issues = Vec::new();

    issues.extend(check_broken_links(notes));
    issues.extend(check_orphans(notes));
    issues.extend(check_stale(notes, config.stale_days()));
    issues.extend(check_missing_hubs(config));
    issues.extend(check_frontmatter(notes));
    issues.extend(check_duplicates(notes));

    // Sort: errors first, then warnings, then info
    issues.sort_by(|a, b| {
        let sev_order = |s: &Severity| match s {
            Severity::Error => 0,
            Severity::Warning => 1,
            Severity::Info => 2,
        };
        sev_order(&a.severity)
            .cmp(&sev_order(&b.severity))
            .then_with(|| a.category.to_string().cmp(&b.category.to_string()))
    });

    issues
}
