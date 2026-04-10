mod checks;
mod config;
mod formatter;
mod scanner;

use anyhow::Result;
use clap::{Parser, Subcommand};
use config::Config;

#[derive(Parser)]
#[command(name = "vaultlink")]
#[command(about = "Obsidian vault integrity checker")]
#[command(version)]
struct Cli {
    /// Initialize default config
    #[arg(long)]
    init: bool,

    /// Show info-level issues (orphans, duplicates)
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run all checks and show results
    Check {
        /// Show info-level issues
        #[arg(short, long)]
        verbose: bool,
    },

    /// Show vault health summary only
    Summary,

    /// Check only broken wikilinks
    Links {
        /// Show info-level issues
        #[arg(short, long)]
        verbose: bool,
    },

    /// Check for orphan notes (no inbound links)
    Orphans,

    /// Check for stale active tasks
    Stale {
        /// Override stale threshold in days
        #[arg(short, long)]
        days: Option<i64>,
    },

    /// Check for projects missing obsidian hubs
    Hubs,

    /// Check frontmatter consistency
    Frontmatter,

    /// Find or fix notes that reference a project but aren't linked to it
    Autolink {
        /// Actually apply project frontmatter fixes (set/add project field)
        #[arg(long)]
        fix: bool,

        /// Actually apply wikilink fixes (append project wikilink in note body)
        #[arg(long)]
        fix_wikilinks: bool,

        /// Preview fixes without writing files
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.init {
        return Config::init_default();
    }

    let config = Config::load()?;
    let notes = scanner::scan_vault(&config.vault_path(), &config.ignore_dirs())?;

    match cli.command {
        Some(Commands::Check { verbose }) => {
            let issues = checks::run_all_checks(&notes, &config);
            formatter::print_issues(&issues, verbose);
        }
        Some(Commands::Summary) => {
            let issues = checks::run_all_checks(&notes, &config);
            formatter::print_summary(&issues, notes.len());
        }
        Some(Commands::Links { verbose }) => {
            let issues = checks::check_broken_links(&notes);
            formatter::print_issues(&issues, verbose);
        }
        Some(Commands::Orphans) => {
            let issues = checks::check_orphans(&notes);
            formatter::print_issues(&issues, true);
        }
        Some(Commands::Stale { days }) => {
            let stale_days = days.unwrap_or(config.stale_days());
            let issues = checks::check_stale(&notes, stale_days);
            formatter::print_issues(&issues, true);
        }
        Some(Commands::Hubs) => {
            let issues = checks::check_missing_hubs(&config);
            formatter::print_issues(&issues, true);
        }
        Some(Commands::Frontmatter) => {
            let issues = checks::check_frontmatter(&notes);
            formatter::print_issues(&issues, true);
        }
        Some(Commands::Autolink {
            fix,
            fix_wikilinks,
            dry_run,
        }) => {
            if !fix && !fix_wikilinks {
                let issues = checks::check_unlinked_projects(&notes, &config);
                formatter::print_issues(&issues, true);
                return Ok(());
            }

            if fix {
                let fixes = checks::find_autolink_fixes(&notes, &config);
                if fixes.is_empty() {
                    println!("No frontmatter fixes found.");
                } else {
                    println!("Found {} frontmatter fixes:\n", fixes.len());
                    for f in &fixes {
                        println!("  {} -> project: {}", f.rel_path, f.project_slug);
                    }
                    println!();
                    if dry_run {
                        println!("Dry run: would apply {} frontmatter fixes.", fixes.len());
                    } else {
                        match checks::apply_autolink_fixes(&fixes) {
                            Ok(count) => println!("Applied {} frontmatter fixes.", count),
                            Err(e) => eprintln!("Error applying frontmatter fixes: {}", e),
                        }
                    }
                }
            }

            if fix_wikilinks {
                let fixes = checks::find_autolink_wikilink_fixes(&notes, &config);
                if fixes.is_empty() {
                    println!("No wikilink fixes found.");
                } else {
                    println!("Found {} wikilink fixes:\n", fixes.len());
                    for f in &fixes {
                        println!(
                            "  {} -> [[{}]] (project: {})",
                            f.rel_path, f.project_link_target, f.project_slug
                        );
                    }
                    println!();
                    if dry_run {
                        println!("Dry run: would apply {} wikilink fixes.", fixes.len());
                    } else {
                        match checks::apply_autolink_wikilink_fixes(&fixes) {
                            Ok(count) => println!("Applied {} wikilink fixes.", count),
                            Err(e) => eprintln!("Error applying wikilink fixes: {}", e),
                        }
                    }
                }
            }
        }
        None => {
            // Default: run all checks
            let issues = checks::run_all_checks(&notes, &config);
            formatter::print_issues(&issues, cli.verbose);
        }
    }

    Ok(())
}
