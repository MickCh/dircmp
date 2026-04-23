# dircmp – Project Knowledge Base

## Project Overview

A Rust TUI application for comparing two folder trees side-by-side, inspired by Total Commander's sync tool. Targets Linux and Windows.

- **Crate name:** `dircmp` (binary + lib), edition 2024, MSRV 1.95
- **Key dependencies:** ratatui 0.30, crossterm 0.29, blake3, walkdir, rayon, serde/toml, anyhow, dirs

## Architecture

Three clearly separated layers:

```
src/
├── main.rs              – CLI arg parsing, terminal init/cleanup
├── lib.rs               – re-exports all modules
├── app.rs               – App struct, main event loop, key handling, external tool launch
├── config/mod.rs        – Config structs + TOML loading
├── engine/
│   ├── scanner.rs       – Recursive folder scan → EntryMap (HashMap<PathBuf, Entry>)
│   ├── diff.rs          – DiffEngine: merges two EntryMaps → DiffResult
│   └── comparator/
│       ├── mod.rs       – FileComparator trait + create_comparator() factory
│       ├── hash.rs      – BLAKE3 comparison
│       ├── metadata.rs  – size + mtime comparison
│       ├── byte.rs      – byte-by-byte comparison
│       └── text.rs      – text with optional whitespace/case normalization
└── ui/
    ├── layout.rs        – AppLayout: header (1 line) + main + statusbar (2 lines)
    ├── panel.rs         – DiffView (unified list), ViewRow, build_view_rows(), nav helpers (next/prev/nth entry, next/prev matching)
    ├── statusbar.rs     – StatusBar: stats + keybinding hints (dynamic, tool-aware)
    └── theme.rs         – Theme: all Style constants
```

### Key Types

| Type | Location | Purpose |
|------|----------|---------|
| `EntryMap` | `engine/scanner.rs` | `HashMap<PathBuf, Entry>` – scan result |
| `Entry` | `engine/scanner.rs` | `{ is_dir: bool }` |
| `DiffResult` | `engine/diff.rs` | `Vec<DiffEntry>` + cached per-status counts – full comparison result |
| `DiffEntry` | `engine/diff.rs` | Single entry: `relative_path`, `status`, `is_dir` |
| `DiffStatus` | `engine/diff.rs` | `Pending\|LeftOnly\|RightOnly\|Identical\|Different\|DirectoryPresent\|TypeConflict\|Error` |
| `FileComparator` | `engine/comparator/mod.rs` | Trait: `compare(&Path, &Path) -> Result<CompareResult>` |
| `CompareResult` | `engine/comparator/mod.rs` | `Identical\|Different\|Error(String)` |
| `ViewRow` | `ui/panel.rs` | `FolderHeader(String)\|Entry(usize)` – index into DiffResult entries |
| `DiffView` | `ui/panel.rs` | Single-cursor list widget; `list_state: ListState` |
| `DiffFilter` | `app.rs` | Struct with four independent bool flags: `show_left_only`, `show_right_only`, `show_different`, `show_identical` |
| `AppState` | `app.rs` | `Idle\|Scanning\|Comparing{done,total}\|Ready` |
| `ExternalAction` | `app.rs` | `Diff{left,right}\|ViewLeft\|EditLeft\|ViewRight\|EditRight` – queued before terminal handoff |

## Extending: Adding a New Comparator

1. Create `src/engine/comparator/myname.rs` implementing `FileComparator` trait
2. Add variant to `ComparisonStrategy` enum in `config/mod.rs`
3. Register in `create_comparator()` factory in `engine/comparator/mod.rs`
4. Add default value to `Config::default()` if needed

## Configuration

Default path: `~/.config/dircmp/config.toml` (Linux/macOS) or `%APPDATA%\dircmp\config.toml` (Windows).
Created automatically with defaults on first run. Override with `--config <file>` / `-c <file>`.

```toml
[comparison]
strategy = "hash"       # hash | metadata | byte | text
parallel = true         # true = rayon par_iter; false = sequential (prefer for HDDs)

[comparison.text]
ignore_whitespace = true
ignore_case = false

[scan]
ignore_patterns = [".git", ".DS_Store", "Thumbs.db", "node_modules"]
follow_symlinks = false

[tools]
diff_tool = "nvim -d"           # invoked on Enter for Different entries
viewer = "bat --paging=always"  # invoked on [ (left) or ] (right)
editor = "nvim"                 # invoked on { (left) or } (right)
```

All `[tools]` fields are optional. When a tool is not configured, the corresponding key bindings are hidden in the status bar and do nothing.

### How external tools are invoked

Each field accepts either a whitespace-separated string or an explicit argument array. Use the array form when the program path contains spaces:

```toml
diff_tool = "nvim -d"                # string form — split by whitespace
diff_tool = ["/my tools/diff", "-d"] # array form — used as-is
```

File path(s) are **appended as additional positional arguments** — there are no placeholders or shell interpolation.

```
diff_tool = "nvim -d"              →  nvim -d /path/to/left /path/to/right
viewer = "bat --paging=always"     →  bat --paging=always /path/to/file
editor = "nvim"                    →  nvim /path/to/file
```

Before the external command runs, the TUI releases raw mode and the alternate screen buffer. After the command exits, the terminal is fully restored and the view refreshes.

## UI Behavior

- **Two-phase diff:** phase 1 = filesystem scan → list shown immediately with `?` (Pending) markers; phase 2 = background thread compares files and updates entries live via `mpsc::channel`
- **Startup:** scan + display runs immediately on launch; F5 re-scans and restarts background comparison
- **Parallel vs sequential:** SSD → rayon `par_iter` (all CPU cores, capped at 8); HDD → sequential loop (avoids seek overhead). Auto-detected on Linux via `/sys/block/<dev>/queue/rotational`; configurable via `parallel = true/false` in `[comparison]`. The rayon `ThreadPool` is created once in `App::new()` and reused across F5 rescans.
- **Non-blocking scan:** Phase 1 scan runs in a background thread (via `scan_rx` channel), so the UI remains responsive with a ⏳ Scanning… overlay during the scan.
- **Unified list view:** no left/right panel split — one list, one cursor
- **Folder header bars:** entries grouped by parent directory; each group starts with a full-width colored bar showing the directory path (`./`, `src/`, `src/engine/` etc.)
- **Directory entries** (`DirectoryPresent`) are skipped — represented only as header bars
- **Cursor navigation:** `↑↓ PgUp/PgDn Home/End`; cursor skips folder header rows (lands only on file entries)
- **Status-based jump:** `n`/`N` jump to the next/previous entry whose status matches the currently selected entry (e.g. standing on a `Different` entry, `n` finds the next `Different`); defaults to `Different` when nothing is selected
- **Filters:** four independent toggles (L/R/D/I keys) for left-only, right-only, different, identical entries
- **Overlay:** `Scanning` shows a spinner overlay; `Idle` shows "press F5" hint
- **No file sizes shown** — removed from `DiffEntry` entirely (not just hidden)
- **Virtual scrolling:** only the visible window of rows is rendered; handles 35 000+ entries without cloning

### Key bindings

| Key | Action |
|-----|--------|
| `↑` / `↓` | Move cursor one entry (skips folder headers) |
| `PgUp` / `PgDn` | Jump by page height |
| `Home` / `End` | Jump to first / last entry |
| `n` / `N` | Jump to next / previous entry with the same status as the current one |
| `F5` | Re-scan both folders and restart comparison |
| `Enter` | Smart open: Different → diff_tool; LeftOnly → viewer (left); RightOnly → viewer (right) |
| `[` / `]` | View left / right file with viewer |
| `{` / `}` | Edit left / right file with editor |
| `L` | Toggle show-left-only filter |
| `R` | Toggle show-right-only filter |
| `D` | Toggle show-different filter |
| `I` | Toggle show-identical filter |
| `Q` | Quit |

Keys for external tools are only shown in the status bar when the corresponding tool is configured.

### Row layout (fixed-width columns)

```
 left_filename           ≠   right_filename
 left_filename           =   left_filename
 left_filename           ►
                         ◄   right_filename
```

Column widths computed dynamically: `left = right = (area.width - 5) / 2`

### Symbol/color legend

| Status | Symbol | Color |
|--------|--------|-------|
| Pending (awaiting comparison) | `?` | Gray (dim) |
| Identical | `=` | White |
| Different | `≠` | Red |
| LeftOnly | `►` | Yellow (right side white/empty) |
| RightOnly | `◄` | Yellow (left side white/empty) |
| TypeConflict | `!` | Mauve |
| Error | `✗` | Red bold |

## User Preferences (from initial design session)

- SOLID principles, modular structure — presentation layer separate from engine
- Comparison strategy must be extensible (Strategy pattern via trait)
- No file size display — user explicitly removed it; not planned for the near future
- No left/right panel split — single unified list (TC Synchronize Directories style)
- Folder header bars as section separators (TC-style "belka")
- Iterative development: comparison first, sync features planned for later

## Tests

```bash
cargo test
```

Integration tests: `tests/engine_tests.rs`
Unit tests: inline in `src/engine/comparator/text.rs`
