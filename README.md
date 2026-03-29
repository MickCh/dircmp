# folder-diff

Narzędzie TUI do porównywania zawartości dwóch folderów. Działa na Linux i Windows.

## Funkcje

- Rekurencyjne skanowanie folderów
- Dwa panele obok siebie (inspirowane Total Commanderem)
- Konfigurowalne strategie porównywania plików:
  - **hash** – SHA-256 (domyślna, najdokładniejsza)
  - **metadata** – rozmiar + data modyfikacji (szybka)
  - **byte** – bajt po bajcie
  - **text** – tekstowe z opcją ignorowania spacji/wielkości liter
- Kolorowe oznaczenia różnic
- Konfiguracja przez plik TOML

## Instalacja

```bash
git clone <repo>
cd folder-diff
cargo build --release
# Plik binarny: target/release/folder-diff
```

## Użycie

```bash
folder-diff <lewy_folder> <prawy_folder> [opcje]

# Przykłady:
folder-diff ~/projekt_v1 ~/projekt_v2
folder-diff /backup/docs /current/docs --config ~/.config/folder-diff/config.toml
```

## Klawisze

| Klawisz     | Akcja                        |
|-------------|------------------------------|
| `F5`        | Uruchom porównanie           |
| `Tab`       | Przełącz aktywny panel       |
| `↑` / `↓`  | Nawigacja po wierszach       |
| `PgUp/Dn`  | Przewijanie o kilka wierszy  |
| `q`         | Wyjście                      |

## Legenda kolorów

| Symbol | Kolor   | Znaczenie                        |
|--------|---------|----------------------------------|
| `◄`    | Czerwony| Plik tylko w lewym folderze      |
| `►`    | Zielony | Plik tylko w prawym folderze     |
| `≠`    | Żółty   | Pliki różnią się zawartością     |
| `=`    | Szary   | Pliki identyczne                 |
| `!`    | Magenta | Konflikt typów (plik vs katalog) |
| `✗`    | Czerwony| Błąd odczytu                    |

## Konfiguracja

Domyślna lokalizacja: `~/.config/folder-diff/config.toml`

```toml
[comparison]
# Strategia: "hash" | "metadata" | "byte" | "text"
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

## Dodawanie nowej strategii porównywania

Implementuj trait `FileComparator` w nowym pliku w `src/engine/comparator/`:

```rust
pub struct MyComparator;

impl FileComparator for MyComparator {
    fn compare(&self, a: &Path, b: &Path) -> Result<CompareResult> {
        // Twoja logika
    }

    fn name(&self) -> &str { "moja strategia" }
}
```

Następnie dodaj wariant do `ComparisonStrategy` w `config/mod.rs` i do fabryki w `engine/comparator/mod.rs`.

## Testy

```bash
cargo test
```
