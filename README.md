# EasySheet

A small native spreadsheet editor written in Rust with **egui / eframe**. No account, network service, telemetry, AI, or model dependency. Formula completion comes from a built-in function catalogue and deterministic rules. The application works offline; Cargo needs internet access only when downloading build dependencies.

## Run

```sh
cargo run --release
# Open a workbook at startup:
cargo run --release -- /absolute/path/workbook.ods
```

Requires a recent stable Rust toolchain. The GUI supports macOS, Windows, and Linux through eframe; this version has been built and exercised on macOS. Linux may require native graphics, font, and file-dialog development packages.

On macOS, build a double-clickable application:

```sh
./scripts/package-macos.sh
open target/release/bundle/EasySheet.app
```

The local bundle is unsigned. Signing, notarization, installers, and Finder file associations are not configured.

## What works

- Native, virtualized spreadsheet grid with pinned row and column labels.
- Click and type; double-click or F2 to edit. A formula bar shows the underlying cell content.
- Arrow navigation, Shift-click or drag range selection, and address navigation.
- Plain text, numbers, booleans, and percentages; imported date/time and rich text values display as plain text.
- Bold, italic, font size, text color, fill, alignment, general / decimal / percent formats, plus configurable currency formatting.
- Numeric conditional formatting with six comparison operators. Both ODF style maps and LibreOffice's numeric conditional-format extension render locally.
- Multiple sheets, adding and renaming sheets, undo/redo, selection sum and count.
- Tab-separated clipboard copy, cut, and paste, including tables copied from LibreOffice.
- Native Open / Save / Save as dialogs, drag-and-drop opening, atomic file replacement, and unsaved-change prompts.
- A four-step guide, searchable function recipes, prefix suggestions, Tab completion, and a **Quick sum** suggestion based on the selected range or the populated cells above.

### Formula support

Use `=`, A1 references, `$A$1` references, rectangular ranges, parentheses, arithmetic (`+ - * / ^`), comparisons (`= <> < > <= >=`), concatenation (`&`), and percentages. Arguments can use semicolons or commas. References are currently to the active sheet only.

Functions: `SUM`, `SUMIF`, `SUMIFS`, `AVERAGE`, `MEDIAN`, `MIN`, `MAX`, `COUNT`, `COUNTA`, `PRODUCT`, `IF`, `IFERROR`, `ROUND`, `ABS`, `SQRT`, `POWER`, `MOD`, `AND`, `OR`, `NOT`, `CONCAT`, `CONCATENATE`, and `LEN`. `IF` and `IFERROR` evaluate branches lazily. Text and boolean formula results are supported. Dependencies recalculate when cells change; cycles and invalid arithmetic produce visible errors.

`=MEDIAN(A1:A10)` returns the middle numeric value, averaging the two middle values for an even count. It accepts ranges and individual arguments, includes zeros, and ignores text, booleans, and blanks in referenced cells. Direct numeric text and booleans are counted, following [Excel’s MEDIAN rules](https://support.microsoft.com/en-us/excel/functions/median-function). A range with no numeric values returns `#NUM!`.

The engine bounds nesting, dependency depth, formula length, and range expansion. It is intended for small everyday workbooks, not large financial models. Formula errors include `#DIV/0!`, `#CYCLE!`, `#VALUE!`, `#NAME?`, `#REF!`, `#NUM!`, `#SYNTAX!`, and `#LIMIT!`.

### Currency formatting

Select cells, then choose **Number → Currency…**. Pick AUD, USD, EUR, GBP, JPY, CAD, NZD, INR, or CHF; choose 0–6 decimal places; toggle thousands separators; and display negative amounts with a minus sign or parentheses, optionally in red. A live preview shows the result. Applying a format changes the display without rounding or converting stored values. These choices save as native ODF currency styles, including negative-number subformats and colors, and have been tested through LibreOffice. Formatting does not convert between currencies. This adds Currency options; Excel’s separate Accounting layout is not implemented.

### Conditional sums

```text
=SUMIF(A2:A10;"Food";C2:C10)
=SUMIF(C2:C10;">=100")
=SUMIFS(C2:C10;A2:A10;"Food";B2:B10;"East")
=SUMIFS(C2:C10;C2:C10;">="&F1;C2:C10;"<"&F2)
```

`SUMIF(criteria_range; criteria; [sum_range])` adds values meeting one condition; omitting the sum range sums the criteria range itself. `SUMIFS(sum_range; criteria_range; criteria; …)` requires every condition to match and supports up to 127 pairs. Ranges in `SUMIFS` must have the same height and width. `SUMIF` uses the sum range’s upper-left cell and the criteria range’s dimensions, including when given a single sum cell.

Criteria support case-insensitive text, numbers, booleans, cell references, `=`, `<>`, `<`, `>`, `<=`, `>=`, blank/nonblank tests (`""` / `"<>"`), and text wildcards `*` and `?`. Prefix a wildcard with `~` to match it literally. Both functions appear in autocomplete and the searchable function library. They use the existing engine’s same-sheet references and 100,000-cell range limit; date serial criteria and the full Excel criteria compatibility surface are not implemented.

The syntax and controls follow Microsoft’s documentation for [SUMIF](https://support.microsoft.com/en-us/excel/functions/sumif-function), [SUMIFS](https://support.microsoft.com/en-us/excel/functions/sumifs-function), and [Currency formatting](https://support.microsoft.com/en-gb/excel/format-numbers-as-currency-in-excel).

## OpenDocument compatibility: scope matters

**This is an initial editor, not a complete implementation of the OpenDocument and OpenFormula standards.** ODS is its native file format, rather than an export-only option. Basic workbooks can be saved here, opened and saved in LibreOffice, and reopened here. Interoperability is tested against the installed LibreOffice and a committed LibreOffice-saved fixture.

The editor retains the `spreadsheet-ods` workbook rather than rebuilding it from visible cells. That preserves other sheets, formula expressions, styles, and the extra document content supported by that library. Supported formulas also save calculated values for other readers. Unsupported imported formulas keep their original expression and cached value in the file, but display an error locally. This is **not a guarantee of lossless round trips for arbitrary ODS files**.

Not implemented or not fully rendered: the full OpenFormula function set and semantics; cross-sheet/external references; formula relocation on copy/paste and fill; named ranges; charts, images, pivot tables, macros, data validation, merged-cell layout, row/column sizing, complex conditional rules and color scales, exact locale-dependent formats, and print/page layout. Rich text is flattened for display and becomes plain text if edited. Imported conditional extensions are preserved and basic numeric rules display, but editing/deleting those imported rules is not implemented. Styles can render differently from LibreOffice. `.fods`, `.ots`, encrypted documents, and Excel formats are not supported.

Keep an original copy of complex workbooks. Complete ODF support requires a substantially larger compatibility effort and a broader conformance suite; it should not be inferred from the ability to open an `.ods` file.

## Verification

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
# Optional live LibreOffice test; set SOFFICE if installed at another location:
cargo test live_libreoffice_roundtrip -- --ignored --nocapture
# Generate a small example workbook without launching the GUI:
cargo run -- --export-example example.ods
```

Tests exercise formula evaluation and errors, ranges, text and lazy branches, save/open, styles and conditional rules, undo/redo, preservation of other sheets, and editing a LibreOffice-saved document. The optional live test creates an ODS workbook, asks LibreOffice to open/save it, reopens it in EasySheet, verifies values and highlights, and saves further edits.

## Code

- `src/main.rs`: native UI, keyboard interaction, walkthrough, and file workflows.
- `src/document.rs`: workbook editing, recalculation, styles, history, and atomic ODS saves.
- `src/formula.rs`: local parser, evaluator, OpenFormula reference conversion, function catalogue.
- `src/currency.rs`: currency settings, ODF currency subformats, and display rendering.
- `src/ods_rules.rs`: display of LibreOffice's numeric conditional-format XML extension.
- `tests/fixtures/libreoffice-budget.ods`: generated sample, opened and re-saved by LibreOffice.

The application is licensed under GPL-3.0-or-later (see `LICENSE`). Bundled Lato fonts are licensed under the SIL Open Font License in `assets/fonts/OFL.txt`. Rust dependencies retain their respective licenses.
