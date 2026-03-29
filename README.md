# dircmp

A TUI tool for comparing the contents of two folders. Works on Linux and Windows.

## Features

- Recursive folder scanning
- Two side-by-side panels (inspired by Total Commander)
- Configurable file comparison strategies:
  - **hash** – SHA-256 (default, most accurate)
  - **metadata** – size + modification date (fast)
  - **byte** – byte-by-byte
  - **text** – text-based with optional whitespace/case normalization
- Color-coded diff indicators
- TOML configuration file

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

| Key         | Action                         |
|-------------|--------------------------------|
| `F5`        | Run comparison                 |
| `F`         | Toggle filter (all / diffs only) |
| `Tab`       | Switch active panel            |
| `↑` / `↓`  | Navigate rows                  |
| `PgUp/PgDn` | Scroll by several rows        |
| `Home/End`  | Jump to first/last entry       |
| `q`         | Quit                           |

## Color Legend

| Symbol | Color   | Meaning                          |
|--------|---------|----------------------------------|
| `◄`    | Red     | File exists only in left folder  |
| `►`    | Green   | File exists only in right folder |
| `≠`    | Yellow  | Files differ in content          |
| `=`    | Gray    | Files are identical              |
| `!`    | Magenta | Type conflict (file vs directory)|
| `✗`    | Red     | Read error                       |

## Configuration

Default location: `~/.config/dircmp/config.toml`

```toml
[comparison]
# Strategy: "hash" | "metadata" | "byte" | "text"
strategy = "hash"

[comparison.text]
ignore_whitespace = true
ignore_case = false

[ui]
panel_scroll_step = 3

[scan]
ignore_patterns = [".git", ".DS_Store", "node_modules"]
follow_symlinks = false
```

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
