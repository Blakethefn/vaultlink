# vaultlink

`vaultlink` is a Rust CLI that checks Obsidian vault health:
- broken wikilinks
- orphan notes
- stale active tasks
- missing project hubs (optional)
- frontmatter issues
- duplicate note stems
- unlinked project references

## Install

### Build from source

```bash
git clone https://github.com/Blakethefn/vaultlink.git
cd vaultlink
cargo build --release
./target/release/vaultlink --help
```

### Optional: install globally

```bash
cargo install --path .
vaultlink --help
```

## Quick Start

1. Initialize config:

```bash
vaultlink --init
```

2. Edit config at `~/.config/vaultlink/config.toml`:

```toml
vault_path = "/path/to/your/obsidian-vault"
tasks_dir = "tasks"
outputs_dir = "outputs"
projects_dir = "01-projects"
stale_days = 7
ignore_dirs = [".obsidian", "templates", "assets"]

# Optional: only needed for `hubs` check.
# This should point to the directory that contains your code project folders.
code_projects_path = "/path/to/your/projects-root"
```

3. Run checks:

```bash
# all checks
vaultlink

# summary
vaultlink summary

# specific checks
vaultlink links
vaultlink orphans
vaultlink stale --days 14
vaultlink hubs
vaultlink frontmatter
vaultlink autolink
vaultlink autolink --fix
```

## Commands

- `check` - run all checks (default)
- `summary` - counts by check category
- `links` - broken wikilinks only
- `orphans` - notes with no inbound wikilinks
- `stale` - notes with `status: active|in_progress` older than threshold
- `hubs` - code project directories missing a project hub note
- `frontmatter` - missing `type`/`status` fields in key note folders
- `autolink` - detect project references not linked to project hubs
- `autolink --fix` - add/set `project:` field for unlinked notes

## Notes

- `hubs` check is skipped unless `code_projects_path` is configured.
- `autolink --fix` now scans all non-project-hub notes and can add minimal frontmatter when missing.
- Info-level checks are hidden by default; use `-v` or `--verbose`.

## Development

```bash
cargo fmt
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## License

MIT
