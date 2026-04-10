use crate::config::Config;
use crate::scanner::{VaultNote, build_backlink_index, build_stem_index};
use chrono::{Local, NaiveDate};
use regex::Regex;
use std::collections::HashSet;

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
    UnlinkedProject,
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
            Category::UnlinkedProject => write!(f, "unlinked-project"),
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
            let found = stem_index.contains_key(link) || stem_index.contains_key(link_stem);

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
        "dashboard",
        "tasks",
        "outputs",
        "projects",
        "logs",
        "math",
        "systems",
        "dev",
        "business",
        "agents",
        "assets",
        "context",
        "memory",
        "templates",
        "meta",
        "README",
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

        if let Some(date_str) = date_str
            && let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        {
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

    issues
}

pub fn check_missing_hubs(config: &Config) -> Vec<Issue> {
    let projects_path = std::fs::read_dir(config.vault_path().join(config.projects_dir()));
    let mut issues = Vec::new();

    // If code_projects_path is not configured, skip this check.
    // The expected code-projects root is environment-specific.
    let Some(code_projects_dir) = config.code_projects_path() else {
        return issues;
    };

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
    if let Ok(entries) = std::fs::read_dir(&code_projects_dir) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip obsidian_docs itself and hidden dirs
            if name == "obsidian_docs"
                || name.starts_with('.')
                || name == "documents"
                || name == "math"
            {
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

#[cfg(test)]
mod tests {
    use super::{AutolinkFix, apply_autolink_fixes, check_missing_hubs, find_autolink_fixes};
    use crate::config::Config;
    use crate::scanner::{Frontmatter, VaultNote};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn base_config(vault_path: &str, code_projects_path: Option<String>) -> Config {
        Config {
            vault_path: vault_path.to_string(),
            tasks_dir: Some("tasks".to_string()),
            outputs_dir: Some("outputs".to_string()),
            projects_dir: Some("01-projects".to_string()),
            code_projects_path,
            ignore_dirs: Some(vec![]),
            stale_days: Some(7),
        }
    }

    fn make_note(path: &str, stem: &str, body: &str, project: Option<&str>) -> VaultNote {
        VaultNote {
            path: PathBuf::from(path),
            rel_path: path.to_string(),
            stem: stem.to_string(),
            frontmatter: Frontmatter {
                note_type: None,
                status: None,
                created: None,
                updated: None,
                project: project.map(str::to_string),
            },
            body: body.to_string(),
            wikilinks: vec![],
        }
    }

    #[test]
    fn missing_hubs_returns_no_issues_without_code_projects_path() {
        let tmp = tempdir().expect("temp dir");
        let vault_path = tmp.path().join("vault");
        fs::create_dir_all(vault_path.join("01-projects")).expect("create vault projects dir");
        fs::write(vault_path.join("01-projects").join("alpha.md"), "# alpha")
            .expect("write project hub");

        let config = base_config(vault_path.to_str().expect("vault path str"), None);
        let issues = check_missing_hubs(&config);
        assert!(issues.is_empty());
    }

    #[test]
    fn missing_hubs_reports_code_project_without_hub() {
        let tmp = tempdir().expect("temp dir");
        let vault_path = tmp.path().join("vault");
        let code_projects_path = tmp.path().join("code-projects");

        fs::create_dir_all(vault_path.join("01-projects")).expect("create vault projects dir");
        fs::write(vault_path.join("01-projects").join("alpha.md"), "# alpha")
            .expect("write project hub");
        fs::create_dir_all(code_projects_path.join("alpha")).expect("create alpha project");
        fs::create_dir_all(code_projects_path.join("beta")).expect("create beta project");

        let config = base_config(
            vault_path.to_str().expect("vault path str"),
            Some(
                code_projects_path
                    .to_str()
                    .expect("code projects path str")
                    .to_string(),
            ),
        );
        let issues = check_missing_hubs(&config);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].note, "beta");
        assert!(issues[0].message.contains("no project hub"));
    }

    #[test]
    fn autolink_fixes_are_not_limited_to_tasks_or_outputs() {
        let tmp = tempdir().expect("temp dir");
        let vault_path = tmp.path().join("vault");
        fs::create_dir_all(vault_path.join("01-projects")).expect("create projects dir");
        fs::write(
            vault_path.join("01-projects").join("topstep.md"),
            "# topstep",
        )
        .expect("write hub");

        let config = base_config(vault_path.to_str().expect("vault path str"), None);
        let notes = vec![make_note(
            "notes/topstep-research.md",
            "topstep-research",
            "Plan for topstep experiments.",
            None,
        )];

        let fixes = find_autolink_fixes(&notes, &config);
        assert_eq!(fixes.len(), 1);
        assert_eq!(fixes[0].rel_path, "notes/topstep-research.md");
        assert_eq!(fixes[0].project_slug, "topstep");
    }

    #[test]
    fn autolink_does_not_fix_when_project_frontmatter_uses_wikilink() {
        let tmp = tempdir().expect("temp dir");
        let vault_path = tmp.path().join("vault");
        fs::create_dir_all(vault_path.join("01-projects")).expect("create projects dir");
        fs::write(
            vault_path.join("01-projects").join("topstep.md"),
            "# topstep",
        )
        .expect("write hub");

        let config = base_config(vault_path.to_str().expect("vault path str"), None);
        let notes = vec![make_note(
            "notes/topstep-log.md",
            "topstep-log",
            "topstep notes",
            Some("[[01-projects/topstep]]"),
        )];

        let fixes = find_autolink_fixes(&notes, &config);
        assert!(fixes.is_empty());
    }

    #[test]
    fn apply_autolink_adds_frontmatter_when_missing() {
        let tmp = tempdir().expect("temp dir");
        let note_path = tmp.path().join("note.md");
        fs::write(&note_path, "# Heading\nBody line").expect("write note");

        let fixes = vec![AutolinkFix {
            note_path: note_path.clone(),
            rel_path: "note.md".to_string(),
            project_slug: "topstep".to_string(),
        }];

        let applied = apply_autolink_fixes(&fixes).expect("apply fixes");
        assert_eq!(applied, 1);

        let updated = fs::read_to_string(&note_path).expect("read note");
        assert!(updated.starts_with("---\nproject: topstep\n---\n\n# Heading"));
    }
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
        if note.rel_path.starts_with("outputs/") && note.stem != "outputs" && note_type.is_empty() {
            issues.push(Issue {
                severity: Severity::Warning,
                category: Category::MissingFrontmatter,
                note: note.rel_path.clone(),
                message: "output note missing 'type' in frontmatter".to_string(),
            });
        }

        // Project hubs should have type: project
        if note.rel_path.starts_with("01-projects/")
            && note.stem != "projects"
            && note_type.is_empty()
        {
            issues.push(Issue {
                severity: Severity::Warning,
                category: Category::MissingFrontmatter,
                note: note.rel_path.clone(),
                message: "project hub missing 'type' in frontmatter".to_string(),
            });
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

/// Collect project slugs from the projects directory in the vault.
/// Returns a list of (slug, hub_stem) pairs derived from filenames — no hardcoded paths.
fn collect_project_slugs(config: &Config) -> Vec<(String, String)> {
    let projects_path = config.vault_path().join(config.projects_dir());
    let mut slugs = Vec::new();

    let entries = match std::fs::read_dir(&projects_path) {
        Ok(e) => e,
        Err(_) => return slugs,
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".md") {
            let stem = name.strip_suffix(".md").unwrap_or(&name).to_string();
            // Skip index files
            if stem == "projects" {
                continue;
            }
            slugs.push((stem.clone(), stem));
        } else if entry.path().is_dir() {
            // Directory-based project hub (project/project.md)
            let stem = name.clone();
            if stem == "projects" {
                continue;
            }
            slugs.push((stem.clone(), stem));
        }
    }

    slugs
}

fn project_aliases(slug: &str, hub_stem: &str) -> Vec<String> {
    let mut aliases = vec![slug.to_lowercase(), hub_stem.to_lowercase()];
    aliases.sort();
    aliases.dedup();
    aliases
}

fn tokens_from_alias(alias: &str) -> Vec<String> {
    alias
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect()
}

fn alias_matches_text(text_lower: &str, alias: &str) -> bool {
    if alias.is_empty() {
        return false;
    }

    let tokens = tokens_from_alias(alias);
    if tokens.is_empty() {
        return text_lower.contains(alias);
    }

    // Match "foo-bar", "foo bar", "foo_bar" and similar separator variants.
    let pattern = format!(r"\b{}\b", regex::escape(&tokens.join(r"[\s\-_]+")));
    match Regex::new(&pattern) {
        Ok(re) => re.is_match(text_lower),
        Err(_) => text_lower.contains(alias),
    }
}

fn note_has_project_link(note: &VaultNote, slug: &str, hub_stem: &str) -> bool {
    let aliases = project_aliases(slug, hub_stem);

    let existing_project = note
        .frontmatter
        .project
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_lowercase();

    if !existing_project.is_empty()
        && aliases
            .iter()
            .any(|alias| alias_matches_text(&existing_project, alias))
    {
        return true;
    }

    note.wikilinks.iter().any(|link| {
        let link_lower = link.to_lowercase();
        let link_stem = link_lower.rsplit('/').next().unwrap_or(&link_lower);
        aliases
            .iter()
            .any(|alias| link_lower == *alias || link_stem == alias || link_lower.ends_with(alias))
    })
}

fn note_mentions_project(note: &VaultNote, slug: &str, hub_stem: &str) -> bool {
    let aliases = project_aliases(slug, hub_stem);
    let stem_lower = note.stem.to_lowercase();
    let body_lower = note.body.to_lowercase();

    aliases.iter().any(|alias| {
        alias_matches_text(&stem_lower, alias) || alias_matches_text(&body_lower, alias)
    })
}

/// Check for notes that reference a project by slug in their filename or body
/// but are not linked to the project hub via wikilink or frontmatter project field.
pub fn check_unlinked_projects(notes: &[VaultNote], config: &Config) -> Vec<Issue> {
    let project_slugs = collect_project_slugs(config);
    let projects_dir = config.projects_dir();
    let mut issues = Vec::new();

    for note in notes {
        // Skip project hub notes themselves
        if note.rel_path.starts_with(&projects_dir) {
            continue;
        }

        for (slug, hub_stem) in &project_slugs {
            if note_has_project_link(note, slug, hub_stem) {
                continue;
            }

            if note_mentions_project(note, slug, hub_stem) {
                issues.push(Issue {
                    severity: Severity::Info,
                    category: Category::UnlinkedProject,
                    note: note.rel_path.clone(),
                    message: format!(
                        "references project '{}' but has no link or project field to [[{}]]",
                        slug, hub_stem
                    ),
                });
            }
        }
    }

    issues
}

/// Represents a fix to apply: adding a project field to a note's frontmatter.
#[derive(Debug)]
pub struct AutolinkFix {
    pub note_path: std::path::PathBuf,
    pub rel_path: String,
    pub project_slug: String,
}

/// Find notes that should be linked to a project and return the fixes to apply.
/// Considers all non-project-hub notes in the vault.
pub fn find_autolink_fixes(notes: &[VaultNote], config: &Config) -> Vec<AutolinkFix> {
    let project_slugs = collect_project_slugs(config);
    let projects_dir = config.projects_dir();
    let mut fixes = Vec::new();

    for note in notes {
        // Skip project hubs themselves.
        if note.rel_path.starts_with(&projects_dir) {
            continue;
        }

        // Find the best matching project (prefer filename match over body match)
        let mut best_match: Option<&str> = None;

        for (slug, hub_stem) in &project_slugs {
            if note_has_project_link(note, slug, hub_stem) {
                continue;
            }

            if alias_matches_text(&note.stem.to_lowercase(), slug)
                || alias_matches_text(&note.stem.to_lowercase(), hub_stem)
            {
                best_match = Some(slug);
                break; // Filename match is definitive
            }

            if best_match.is_none() && note_mentions_project(note, slug, hub_stem) {
                best_match = Some(slug);
            }
        }

        if let Some(project_slug) = best_match {
            fixes.push(AutolinkFix {
                note_path: note.path.clone(),
                rel_path: note.rel_path.clone(),
                project_slug: project_slug.to_string(),
            });
        }
    }

    fixes
}

/// Apply autolink fixes by setting the `project:` frontmatter field.
/// If a note has no frontmatter, a minimal frontmatter block is added.
pub fn apply_autolink_fixes(fixes: &[AutolinkFix]) -> Result<usize, anyhow::Error> {
    let mut applied = 0;

    for fix in fixes {
        let content = std::fs::read_to_string(&fix.note_path)?;
        let trimmed = content.trim_start();

        if !trimmed.starts_with("---") {
            // No frontmatter block — add a minimal one.
            let new_content = format!("---\nproject: {}\n---\n\n{}", fix.project_slug, content);
            std::fs::write(&fix.note_path, new_content)?;
            applied += 1;
            continue;
        }

        let after_open = &trimmed[3..];
        if let Some(close_pos) = after_open.find("\n---") {
            let yaml_block = &after_open[..close_pos];

            // Don't overwrite an existing project field
            if yaml_block
                .lines()
                .any(|l| l.trim_start().starts_with("project:"))
            {
                continue;
            }

            // Insert `project: <slug>` before the closing ---
            let new_yaml = format!("{}\nproject: {}", yaml_block, fix.project_slug);
            let rest = &after_open[close_pos..];
            let new_content = format!("---{}{}", new_yaml, rest);

            std::fs::write(&fix.note_path, new_content)?;
            applied += 1;
        }
    }

    Ok(applied)
}

pub fn run_all_checks(notes: &[VaultNote], config: &Config) -> Vec<Issue> {
    let mut issues = Vec::new();

    issues.extend(check_broken_links(notes));
    issues.extend(check_orphans(notes));
    issues.extend(check_stale(notes, config.stale_days()));
    issues.extend(check_missing_hubs(config));
    issues.extend(check_frontmatter(notes));
    issues.extend(check_duplicates(notes));
    issues.extend(check_unlinked_projects(notes, config));

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
