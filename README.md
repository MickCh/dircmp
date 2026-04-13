# dircmp

A TUI tool for comparing the contents of two folders. Works on Linux and Windows.

## Features

- Recursive folder scanning with live two-phase comparison (structure first, then content)
- Unified list view with folder section headers (inspired by Total Commander)
- Configurable file comparison strategies:
  - **hash** – SHA-256 (default, most accurate)
  - **metadata** – size + modification date (fast)
  - **byte** – byte-by-byte
  - **text** – text-based with optional whitespace/case normalization
- Color-coded diff indicators
- External viewer, editor, and diff tool integration
- Independent filters for each entry type
- TOML configuration file (auto-created with defaults on first run)

## Installation

```bash
git clone <repo>
cd dircmp
cargo build --release
# Binary: target/release/dircmp
```

## Usage

```bash
dircmp <left_folder> <right_folder> [options]

# Examples:
dircmp ~/project_v1 ~/project_v2
dircmp /backup/docs /current/docs --config ~/.config/dircmp/config.toml
```

## Keybindings

| Key         | Action                                                          |
|-------------|-----------------------------------------------------------------|
| `F5`        | Re-scan both folders and restart comparison                     |
| `↑` / `↓`  | Navigate entries (skips folder headers)                         |
| `PgUp/PgDn` | Scroll by page height                                           |
| `Home/End`  | Jump to first/last entry                                        |
| `n` / `N`   | Jump to next/previous entry with the same status as current     |
| `Enter`     | Open: Different → diff tool; LeftOnly/RightOnly → viewer        |
| `[` / `]`   | View left / right file with viewer                              |
| `{` / `}`   | Edit left / right file with editor                              |
| `L`         | Toggle filter: left-only entries                                |
| `R`         | Toggle filter: right-only entries                               |
| `D`         | Toggle filter: different entries                                |
| `I`         | Toggle filter: identical entries                                |
| `q`         | Quit                                                            |

Keys for external tools are only shown when the tool is configured.

## Color Legend

| Symbol | Color   | Meaning                          |
|--------|---------|----------------------------------|
| `?`    | Orange  | Awaiting content comparison      |
| `◄`    | Red     | File exists only in left folder  |
| `►`    | Green   | File exists only in right folder |
| `≠`    | Red     | Files differ in content          |
| `=`    | Gray    | Files are identical              |
| `!`    | Magenta | Type conflict (file vs directory)|
| `✗`    | Red     | Read error                       |

## Configuration

Default location: `~/.config/dircmp/config.toml` (Linux/macOS) or `%APPDATA%\dircmp\config.toml` (Windows).
Created automatically with defaults if absent.

```toml
[comparison]
strategy = "hash"   # hash | metadata | byte | text
parallel = true     # false recommended for HDDs

[comparison.text]
ignore_whitespace = true
ignore_case = false

[scan]
ignore_patterns = [".git", ".DS_Store", "node_modules"]
follow_symlinks = false

[tools]
diff_tool = "nvim -d"            # invoked on Enter for Different files
viewer = "bat --paging=always"   # invoked on [ / ] or Enter for single-side files
editor = "nvim"                  # invoked on { / }
```

### How external tools are invoked

The command string is split by whitespace into a program and its arguments. File paths are
**appended as positional arguments** — there are no placeholders or shell interpolation:

```
diff_tool = "nvim -d"          →  nvim -d /left/file /right/file
viewer    = "bat --paging=always" →  bat --paging=always /path/to/file
editor    = "nvim"             →  nvim /path/to/file
```

The terminal is suspended while the external tool runs and fully restored on exit.

## Adding a New Comparison Strategy

Implement the `FileComparator` trait in a new file under `src/engine/comparator/`:

```rust
pub struct MyComparator;

impl FileComparator for MyComparator {
    fn compare(&self, a: &Path, b: &Path) -> Result<CompareResult> {
        // Your logic here
    }

    fn name(&self) -> &str { "my strategy" }
}
```

Then add a variant to `ComparisonStrategy` in `config/mod.rs` and register it in the factory in `engine/comparator/mod.rs`.

## Tests

```bash
cargo test
```

## Authorship

The entire codebase was written by [Claude Code](https://claude.ai/code) (Anthropic's AI coding assistant). Michal Chwirut authored the concept, requirements, and all design decisions — but not a single line of source code.

This project is an example of AI-assisted development taken to its logical conclusion: a human providing direction, an AI providing implementation.

---

*Built with [Claude Code](https://claude.ai/code)*
