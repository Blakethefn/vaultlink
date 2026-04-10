use crate::checks::{Category, Issue, Severity};
use colored::Colorize;

pub fn print_issues(issues: &[Issue], verbose: bool) {
    if issues.is_empty() {
        println!("{}", "No issues found. Vault is clean.".green().bold());
        return;
    }

    let errors = issues
        .iter()
        .filter(|i| i.severity == Severity::Error)
        .count();
    let warnings = issues
        .iter()
        .filter(|i| i.severity == Severity::Warning)
        .count();
    let infos = issues
        .iter()
        .filter(|i| i.severity == Severity::Info)
        .count();

    println!("{}", "Vault Check Results".bold().underline());
    println!();

    // Summary line
    print!("  ");
    if errors > 0 {
        print!("{} ", format!("{} errors", errors).red().bold());
    }
    if warnings > 0 {
        print!("{} ", format!("{} warnings", warnings).yellow().bold());
    }
    if infos > 0 {
        print!("{} ", format!("{} info", infos).dimmed());
    }
    println!();
    println!();

    let show_info = verbose;

    for issue in issues {
        if issue.severity == Severity::Info && !show_info {
            continue;
        }

        let severity_badge = match issue.severity {
            Severity::Error => " ERR ".on_red().white().bold().to_string(),
            Severity::Warning => " WRN ".on_yellow().black().bold().to_string(),
            Severity::Info => " INF ".dimmed().to_string(),
        };

        let category_str = format!("[{}]", issue.category).dimmed();

        println!(
            "  {} {} {} {}",
            severity_badge,
            category_str,
            issue.note.white().bold(),
            issue.message.dimmed(),
        );
    }

    if !show_info && infos > 0 {
        println!();
        println!(
            "  {}",
            format!("{} info items hidden (use --verbose to show)", infos).dimmed()
        );
    }

    println!();
}

pub fn print_summary(issues: &[Issue], note_count: usize) {
    let errors = issues
        .iter()
        .filter(|i| i.severity == Severity::Error)
        .count();
    let warnings = issues
        .iter()
        .filter(|i| i.severity == Severity::Warning)
        .count();
    let infos = issues
        .iter()
        .filter(|i| i.severity == Severity::Info)
        .count();

    println!("{}", "Vault Health".bold().underline());
    println!();
    println!("  {} {}", "Notes scanned:".cyan().bold(), note_count);
    println!(
        "  {} {}",
        "Broken links:".cyan().bold(),
        count_by_category(issues, Category::BrokenLink),
    );
    println!(
        "  {} {}",
        "Orphan notes:".cyan().bold(),
        count_by_category(issues, Category::Orphan),
    );
    println!(
        "  {} {}",
        "Stale notes:".cyan().bold(),
        count_by_category(issues, Category::Stale),
    );
    println!(
        "  {} {}",
        "Missing hubs:".cyan().bold(),
        count_by_category(issues, Category::MissingHub),
    );
    println!(
        "  {} {}",
        "Frontmatter issues:".cyan().bold(),
        count_by_category(issues, Category::MissingFrontmatter),
    );
    println!(
        "  {} {}",
        "Duplicate stems:".cyan().bold(),
        count_by_category(issues, Category::Duplicate),
    );
    println!(
        "  {} {}",
        "Unlinked projects:".cyan().bold(),
        count_by_category(issues, Category::UnlinkedProject),
    );
    println!();

    if errors == 0 && warnings == 0 {
        println!("  {}", "Vault is healthy.".green().bold());
    } else if errors == 0 {
        println!("  {}", format!("{} warnings to review.", warnings).yellow());
    } else {
        println!(
            "  {}",
            format!("{} errors and {} warnings.", errors, warnings)
                .red()
                .bold()
        );
    }
    let _ = infos; // suppress unused
    println!();
}

fn count_by_category(issues: &[Issue], category: Category) -> String {
    let count = issues.iter().filter(|i| i.category == category).count();
    if count == 0 {
        "0".green().to_string()
    } else {
        count.to_string().yellow().to_string()
    }
}
