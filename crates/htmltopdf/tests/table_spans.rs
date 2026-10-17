//! `colspan` values a browser accepts render instead of crashing (F55). Blitz passed the attribute to Taffy as a grid
//! span unchecked: `colspan="0"` panicked in Taffy's track sizing, `colspan="65535"` and a row whose spans add up to
//! more than Taffy's `i16` grid lines panicked in grid placement, so the whole document failed.
//!
//! HTML §4.9.11 parses `colspan` with the rules for parsing non-negative integers (leading whitespace skipped, the
//! leading digits taken, so `" 2"` and `"2.5"` are 2) and clamps it: 0 or an unparsable value is 1, above 1000 is
//! 1000. Column positions are asserted against the next row, so the numbers do not depend on font metrics.
//!
//! F59: a `colspan` wider than the table (`colspan="99"`, the "full-width row" idiom) must not create columns. In
//! automatic layout a column in which no cell starts has no width (Chrome gives it none; HTML calls such a column a
//! table model error and defines no rendering), so a cell spans the columns that exist in its range. Each case is
//! compared with an exact-span twin (`colspan=3` in a three-column table): both must produce the same grid, which
//! does not depend on font metrics either.

use htmltopdf::{RenderOptions, Renderer};

const CSS: &str = "body { margin: 0 } td { padding: 4px } table { border-spacing: 0 }";

fn render(body: &str) -> htmltopdf::Result<Vec<u8>> {
    Renderer::builder().css(CSS).build().render(
        &format!("<!doctype html><html><body>{body}</body></html>"),
        &RenderOptions::default(),
    )
}

/// `(xMin, text)` of every word, in extraction order.
fn words(pdf: &[u8]) -> Vec<(f32, String)> {
    let path = std::env::temp_dir().join(format!(
        "htmltopdf-spans-{}-{}.pdf",
        std::process::id(),
        N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::write(&path, pdf).unwrap();
    let out = std::process::Command::new("pdftotext")
        .arg("-bbox")
        .arg(&path)
        .arg("-")
        .output()
        .expect("pdftotext (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "pdftotext failed");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let rest = l.trim().strip_prefix("<word ")?;
            let (attrs, word) = rest.split_once('>')?;
            let x = attrs
                .split_whitespace()
                .find_map(|a| a.strip_prefix("xMin=\""))?
                .trim_end_matches('"')
                .parse()
                .ok()?;
            Some((x, word.strip_suffix("</word>")?.to_string()))
        })
        .collect()
}

static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn x_of(found: &[(f32, String)], text: &str) -> f32 {
    found
        .iter()
        .find(|(_, w)| w == text)
        .unwrap_or_else(|| panic!("{text} is not printed; printed words: {found:?}"))
        .0
}

/// A first row `<td colspan=…>A</td><td>B</td>` over a three-cell row `C D E`: `B` starts where the cell under it
/// in the second row starts, which tells how many columns `A` spans.
fn spanned_columns(colspan: &str) -> usize {
    let pdf = render(&format!(
        "<table><tr><td colspan=\"{colspan}\">A</td><td>B</td></tr>\
         <tr><td>Ccc</td><td>Ddd</td><td>Eee</td></tr></table>"
    ))
    .unwrap_or_else(|e| panic!("colspan={colspan:?}: {e}"));
    let found = words(&pdf);
    let b = x_of(&found, "B");
    let d = x_of(&found, "Ddd");
    let e = x_of(&found, "Eee");
    if (b - d).abs() < 0.5 {
        1
    } else if (b - e).abs() < 0.5 {
        2
    } else {
        panic!("colspan={colspan:?}: B at {b} is under neither D ({d}) nor E ({e})")
    }
}

#[test]
fn a_zero_or_unparsable_colspan_spans_one_column() {
    for value in ["0", "00", "-1", "abc", "", "+1"] {
        assert_eq!(spanned_columns(value), 1, "colspan={value:?}");
    }
}

#[test]
fn colspan_takes_the_leading_digits_after_whitespace() {
    for value in ["2", " 2", "\t2", "2.5", "2px", "02"] {
        assert_eq!(spanned_columns(value), 2, "colspan={value:?}");
    }
}

#[test]
fn a_huge_colspan_renders() {
    for value in ["1000", "1001", "65535", "65536", "99999999999999999999"] {
        let pdf = render(&format!(
            "<table><tr><td colspan=\"{value}\">A</td><td>B</td></tr><tr><td>C</td></tr></table>"
        ))
        .unwrap_or_else(|e| panic!("colspan={value:?}: {e}"));
        let found = words(&pdf);
        for w in ["A", "B", "C"] {
            x_of(&found, w);
        }
    }
}

#[test]
fn a_row_whose_spans_exceed_the_grid_line_limit_renders_every_cell() {
    // 40 × 1000 raw columns are beyond Taffy's i16 grid lines; the document still renders.
    // The text is small so that all 40 columns fit on the page (a cell off the page is not printed).
    let cells: String = (0..40)
        .map(|i| format!("<td colspan=1000>X{i}</td>"))
        .collect();
    let pdf = render(&format!(
        "<table style=\"font-size: 5px\"><tr>{cells}</tr><tr><td>Last</td></tr></table>"
    ))
    .expect("render");
    let found = words(&pdf);
    x_of(&found, "Last");
    x_of(&found, "X0");
    // Each of the 40 cells starts a column of its own: the last ones are not stacked on one x.
    assert!(x_of(&found, "X39") > x_of(&found, "X38"));
}

#[test]
fn a_zero_or_huge_rowspan_renders() {
    for value in ["0", "65535", "70000", "-3"] {
        let pdf = render(&format!(
            "<table><tr><td rowspan=\"{value}\">A</td><td>B</td></tr><tr><td>C</td></tr></table>"
        ))
        .unwrap_or_else(|e| panic!("rowspan={value:?}: {e}"));
        let found = words(&pdf);
        for w in ["A", "B", "C"] {
            x_of(&found, w);
        }
    }
}

// ---- F59: a colspan wider than the table ------------------------------------------------------------------------

/// Asserts that every word in `words_` starts at the same x (±0.5 pt) in the two documents, and that there is at
/// least one word to compare.
fn assert_same_grid(what: &str, body: &str, twin: &str, words_: &[&str]) {
    let a = words(&render(body).unwrap_or_else(|e| panic!("{what}: {e}")));
    let b = words(&render(twin).expect("twin renders"));
    for w in words_ {
        let (xa, xb) = (x_of(&a, w), x_of(&b, w));
        assert!(
            (xa - xb).abs() <= 0.5,
            "{what}: {w} is at {xa} pt, its exact-span twin has it at {xb} pt"
        );
    }
}

/// `A1 B1 C1` / `FULL` spanning `n` / `A3 B3 C3`; the full row first, in the middle or last.
fn three_columns_with_full_row(table_style: &str, n: u32, full: &str, at: usize) -> String {
    let mut rows = vec![
        "<tr><td>A1</td><td>B1</td><td>C1</td></tr>".to_string(),
        "<tr><td>A3</td><td>B3</td><td>C3</td></tr>".to_string(),
    ];
    rows.insert(at, format!("<tr><td colspan=\"{n}\">{full}</td></tr>"));
    format!("<table style=\"{table_style}\">{}</table>", rows.concat())
}

#[test]
fn a_colspan_wider_than_the_table_spans_the_real_columns() {
    for n in [4, 5, 99, 1000] {
        for at in [0, 1, 2] {
            assert_same_grid(
                &format!("colspan={n}, full row at {at}"),
                &three_columns_with_full_row("width: 600px", n, "FULL", at),
                &three_columns_with_full_row("width: 600px", 3, "FULL", at),
                &["B1", "C1", "B3", "C3"],
            );
        }
    }
}

#[test]
fn a_wide_span_in_a_shrink_to_fit_table_gives_its_content_to_the_real_columns_only() {
    // The 60-character cell is wider than the three columns together: they grow to hold it, as for `colspan=3`.
    let long = format!("FULL{}", "x".repeat(56));
    for n in [4, 99] {
        assert_same_grid(
            &format!("colspan={n}, long content"),
            &three_columns_with_full_row("", n, &long, 1),
            &three_columns_with_full_row("", 3, &long, 1),
            &["B1", "C1", "B3", "C3"],
        );
    }
}

#[test]
fn a_full_width_header_row_does_not_disturb_the_columns_below_it() {
    let table = |n: u32| {
        format!(
            "<table style=\"width: 600px\"><tr><th colspan=\"{n}\">Hlavicka</th></tr>\
             <tr><td style=\"width: 100px\">a1</td><td>b1</td><td>c1</td></tr>\
             <tr><td>a2</td><td>b2</td><td>c2</td></tr></table>"
        )
    };
    assert_same_grid(
        "header idiom",
        &table(99),
        &table(3),
        &["a1", "b1", "c1", "a2", "b2", "c2"],
    );
}

#[test]
fn a_column_only_spans_cover_has_no_width_of_its_own() {
    // Every row is `X` over two columns, then `Y`: the second column starts no cell. Chrome merges it with the
    // first, with `border-spacing` or without, so `Y` stands where it does in a two-cell row.
    let rows = |first: &str| {
        (0..2)
            .map(|i| format!("<tr><td{first}>X{i}</td><td>Y{i}</td></tr>"))
            .collect::<String>()
    };
    for style in ["width: 600px", ""] {
        assert_same_grid(
            &format!("inner span-only column ({style:?})"),
            &format!(
                "<table style=\"{style}\">{}</table>",
                rows(" colspan=\"2\"")
            ),
            &format!("<table style=\"{style}\">{}</table>", rows("")),
            &["X0", "Y0", "Y1"],
        );
    }
    // The same inside a wider table: `B1` spans two columns, the row below spans three of them.
    let mix = |span: u32| {
        format!(
            "<table style=\"width: 600px\"><tr><td>A1</td><td colspan=\"{span}\">B1</td><td>D1</td></tr>\
             <tr><td colspan=\"{}\">X</td><td>Y</td></tr></table>",
            span + 1
        )
    };
    assert_same_grid("mixed spans", &mix(2), &mix(1), &["B1", "D1", "Y"]);
}

#[test]
fn rows_that_all_span_form_one_column() {
    // Rows spanning 5 and 3 columns, then a row of a 4-column cell and one more: the columns that exist are the
    // one all of them start in and the one `R3b` starts.
    let table = "<table style=\"width: 600px\"><tr><td colspan=\"5\">R1a</td></tr>\
                 <tr><td colspan=\"3\">R2a</td></tr>\
                 <tr><td colspan=\"4\">R3a</td><td>R3b</td></tr></table>";
    let twin = "<table style=\"width: 600px\"><tr><td colspan=\"2\">R1a</td></tr>\
                <tr><td colspan=\"1\">R2a</td></tr>\
                <tr><td colspan=\"1\">R3a</td><td>R3b</td></tr></table>";
    assert_same_grid("all rows span", table, twin, &["R1a", "R2a", "R3a", "R3b"]);
}

#[test]
fn twenty_cells_of_colspan_1000_are_twenty_columns() {
    // Rows of 20 × 1000 raw columns: since F55 the cells past the 16th shared the last column.
    let row = |span: &str| {
        let cells: String = (0..20)
            .map(|i| format!("<td colspan=\"{span}\">X{i}</td>"))
            .collect();
        format!("<tr>{cells}</tr>")
    };
    let table = |span: &str| {
        format!(
            "<table style=\"width: 600px\">{}{}</table>",
            row(span),
            row(span)
        )
    };
    let names: Vec<String> = (0..20).map(|i| format!("X{i}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    assert_same_grid("20 × colspan=1000", &table("1000"), &table("1"), &names);
}

#[test]
fn nested_wide_spans_are_exact_spans_too() {
    let inner = |n: u32| {
        format!(
            "<table style=\"width: 100%\"><tr><td>a1</td><td>b1</td><td>c1</td></tr>\
             <tr><td colspan=\"{n}\">INNERFULL</td></tr></table>"
        )
    };
    let outer = |n: u32, m: u32| {
        format!(
            "<table style=\"width: 600px\"><tr><td>A1</td><td>B1</td><td>C1</td></tr>\
             <tr><td colspan=\"{n}\">{}</td></tr></table>",
            inner(m)
        )
    };
    assert_same_grid(
        "nested",
        &outer(99, 99),
        &outer(3, 3),
        &["A1", "B1", "C1", "a1", "b1", "c1"],
    );
}

// Green before and after the fix: what the rule must not change.

#[test]
fn fixed_layout_keeps_the_columns_only_a_span_covers() {
    // `table-layout: fixed` with a table width: the 96 columns a `colspan=99` row adds are real columns that
    // divide the width that is left (CSS 2.1 §17.5.2.1), so `TAIL` lies far to the right, as in Chrome. Merged
    // like in automatic layout it would stand one column after `C1`.
    let pdf = render(
        "<table style=\"table-layout: fixed; width: 600px\"><tr><td style=\"width: 100px\">A1</td>\
         <td style=\"width: 200px\">B1</td><td>C1</td></tr>\
         <tr><td colspan=\"99\">SPANFIXW</td><td>TAIL</td></tr></table>",
    )
    .expect("render");
    let found = words(&pdf);
    let (c1, tail) = (x_of(&found, "C1"), x_of(&found, "TAIL"));
    assert!(tail > c1 + 100.0, "TAIL at {tail} pt, C1 at {c1} pt");
}

#[test]
fn a_fixed_layout_table_without_a_width_is_laid_out_automatically() {
    // CSS Tables 3: `table-layout: fixed` applies only to a table whose `width` is not `auto`; Chrome lays
    // this one out automatically, so the column only the span covers has no width.
    let pdf = render(
        "<table style=\"table-layout: fixed\"><tr><td>A1</td><td>B1</td></tr>\
         <tr><td colspan=\"99\">SPAN</td><td>TAIL</td></tr></table>",
    )
    .expect("render");
    let found = words(&pdf);
    let (b1, tail) = (x_of(&found, "B1"), x_of(&found, "TAIL"));
    assert!(
        (tail - b1).abs() < 40.0,
        "TAIL at {tail} pt, B1 at {b1} pt: a column was added for the span"
    );
}

#[test]
fn a_table_whose_only_row_is_one_wide_cell_renders() {
    for style in ["width: 600px", ""] {
        let pdf = render(&format!(
            "<table style=\"{style}\"><tr><td colspan=\"99\">ONLY</td></tr></table>"
        ))
        .expect("render");
        x_of(&words(&pdf), "ONLY");
    }
}

#[test]
fn exact_spans_keep_their_columns() {
    // The shape of fixture 03: a totals row whose label spans the four columns before the amount.
    let pdf = render(
        "<table style=\"width: 600px\"><tr><td>A1</td><td>B1</td><td>C1</td><td>D1</td><td>E1</td></tr>\
         <tr><td colspan=\"4\">Total</td><td>E2</td></tr></table>",
    )
    .expect("render");
    let found = words(&pdf);
    assert!((x_of(&found, "E1") - x_of(&found, "E2")).abs() <= 0.5);
}

#[test]
fn a_row_of_17000_cells_renders() {
    // More cells than Taffy has grid lines: the cells past `MAX_COLUMNS` share the last column.
    let cells: String = (0..17_000).map(|i| format!("<td>c{i}</td>")).collect();
    let pdf = render(&format!(
        "<table><tr>{cells}</tr><tr><td>Last</td></tr></table>"
    ))
    .expect("render");
    x_of(&words(&pdf), "Last");
}
