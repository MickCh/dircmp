# dircmp – Project Knowledge Base

## Project Overview

A Rust TUI application for comparing two folder trees side-by-side, inspired by Total Commander's sync tool. Targets Linux and Windows.

- **Crate name:** `dircmp` (binary + lib), edition 2024, MSRV 1.85
- **Key dependencies:** ratatui 0.29, crossterm 0.28, sha2, walkdir, rayon, serde/toml, anyhow, dirs

## Architecture

Three clearly separated layers:

```
src/
├── main.rs              – CLI arg parsing, terminal init/cleanup
├── lib.rs               – re-exports all modules
├── app.rs               – App struct, main event loop, key handling
├── config/mod.rs        – Config structs + TOML loading
├── engine/
│   ├── scanner.rs       – Recursive folder scan → EntryMap (HashMap<PathBuf, Entry>)
│   ├── diff.rs          – DiffEngine: merges two EntryMaps → DiffResult
│   └── comparator/
│       ├── mod.rs       – FileComparator trait + create_comparator() factory
│       ├── hash.rs      – SHA-256 comparison
│       ├── metadata.rs  – size + mtime comparison
│       ├── byte.rs      – byte-by-byte comparison
│       └── text.rs      – text with optional whitespace/case normalization
└── ui/
    ├── layout.rs        – AppLayout: header (1 line) + main + statusbar (2 lines)
    ├── panel.rs         – DiffView (unified list), ViewRow, build_view_rows(), nav helpers
    ├── statusbar.rs     – StatusBar: stats + keybinding hints
    └── theme.rs         – Theme: all Style constants
```

### Key Types

| Type | Location | Purpose |
|------|----------|---------|
| `EntryMap` | `engine/scanner.rs` | `HashMap<PathBuf, Entry>` – scan result |
| `Entry` | `engine/scanner.rs` | `{ is_dir: bool }` |
| `DiffResult` | `engine/diff.rs` | `Vec<DiffEntry>` – full comparison result |
| `DiffEntry` | `engine/diff.rs` | Single entry: `relative_path`, `status`, `is_dir` |
| `DiffStatus` | `engine/diff.rs` | `LeftOnly\|RightOnly\|Identical\|Different\|DirectoryPresent\|TypeConflict\|Error` |
| `FileComparator` | `engine/comparator/mod.rs` | Trait: `compare(&Path, &Path) -> Result<CompareResult>` |
| `CompareResult` | `engine/comparator/mod.rs` | `Identical\|Different\|Error(String)` |
| `ViewRow` | `ui/panel.rs` | `FolderHeader(String)\|Entry(DiffEntry)` – unified list rows |
| `DiffView` | `ui/panel.rs` | Single-cursor list widget; `list_state: ListState` |
| `DiffFilter` | `app.rs` | `All\|DifferencesOnly` – view filter toggled with F |
| `AppState` | `app.rs` | `Idle\|Scanning\|Comparing{done,total}\|Ready` |

## Extending: Adding a New Comparator

1. Create `src/engine/comparator/myname.rs` implementing `FileComparator` trait
2. Add variant to `ComparisonStrategy` enum in `config/mod.rs`
3. Register in `create_comparator()` factory in `engine/comparator/mod.rs`
4. Add default value to `Config::default()` if needed

## Configuration

Default path: `~/.config/dircmp/config.toml`. Falls back to built-in defaults if file is absent.

```toml
[comparison]
strategy = "hash"       # hash | metadata | byte | text

[comparison.text]
ignore_whitespace = true
ignore_case = false

[ui]
panel_scroll_step = 3

[scan]
ignore_patterns = [".git", ".DS_Store", "Thumbs.db", "node_modules"]
follow_symlinks = false
```

## UI Behavior

- **Two-phase diff:** phase 1 = filesystem scan → list shown immediately with `?` markers; phase 2 = background thread compares files and updates entries live via `mpsc::channel`
- **Startup:** scan + display runs immediately on launch; F5 re-scans and restarts background comparison
- **Parallel vs sequential:** SSD → rayon `par_iter` (all CPU cores); HDD → sequential loop (avoids seek overhead). Auto-detected on Linux via `/sys/block/<dev>/queue/rotational`; configurable via `parallel = true/false` in `[comparison]`
- **Unified list view:** no left/right panel split — one list, one cursor
- **Folder header bars:** entries grouped by parent directory; each group starts with a full-width colored bar showing the directory path (`./`, `src/`, `src/engine/` etc.)
- **Directory entries** (`DirectoryPresent`) are skipped — represented only as header bars
- **Cursor navigation:** `↑↓ PgUp/PgDn Home/End`; cursor skips folder header rows (lands only on file entries)
- **Filter (F key):** toggles between showing all entries and differences only
- **Overlay:** `Scanning` shows a spinner overlay; `Idle` shows "press F5" hint
- **No file sizes shown** — removed from `DiffEntry` entirely (not just hidden)

### Row layout (fixed-width columns)

```
 left_filename           ≠   right_filename
 left_filename           =   left_filename
 left_filename           ◄
                         ►   right_filename
```

Column widths computed dynamically: `left = right = (area.width - 5) / 2`

### Symbol/color legend

| Status | Symbol | Color |
|--------|--------|-------|
| Identical | `=` | Gray |
| Different | `≠` | Red |
| LeftOnly | `◄` | Red (right side gray empty) |
| RightOnly | `►` | Green (left side gray empty) |
| TypeConflict | `!` | Magenta |
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
