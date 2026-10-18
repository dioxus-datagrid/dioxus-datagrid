//! Excel (`.xlsx`): typed cells that keep their number formats.

use datagrid_core::{
    AggregateKind, CellAlign, CellFormat, ExportAggregate, ExportColumn, ExportRow, ExportTable,
    Value,
};
use rust_xlsxwriter::{ExcelDateTime, Format, FormatAlign, FormatBorder, Workbook, Worksheet};
use std::fmt;

/// The longest text an Excel cell holds, in characters.
const MAX_CELL_CHARS: usize = 32_767;
/// The deepest outline Excel draws.
const MAX_OUTLINE_LEVELS: usize = 7;
/// How many rows are measured to find a column's width.
const WIDTH_SAMPLE_ROWS: usize = 1_000;
/// The narrowest and the widest a column is made, in characters.
const WIDTH_RANGE: (f64, f64) = (8.0, 60.0);

/// How [`to_xlsx`] writes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XlsxOptions {
    /// The name of the sheet. Excel allows 31 characters and none of
    /// `[ ] : * ? / \`; what does not fit is changed rather than refused.
    pub sheet_name: String,
    /// Whether to write the column labels, in bold over a line, and keep them in
    /// view as the sheet scrolls.
    pub header: bool,
    /// Whether to put a filter on each column's label. Only a table of nothing but
    /// data rows gets one: a filter would hide a group's heading or its totals
    /// along with the rows.
    pub autofilter: bool,
    /// Whether to size each column to what it holds.
    pub auto_width: bool,
    /// Whether to write each open group's rows as an outline that Excel can
    /// collapse.
    pub outline: bool,
}

impl Default for XlsxOptions {
    fn default() -> Self {
        Self {
            sheet_name: "Export".to_owned(),
            header: true,
            autofilter: true,
            auto_width: true,
            outline: true,
        }
    }
}

/// Why a workbook could not be written: a sheet has at most 1,048,576 rows and
/// 16,384 columns, and a workbook has to fit in memory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XlsxError {
    message: String,
}

impl fmt::Display for XlsxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for XlsxError {}

impl From<rust_xlsxwriter::XlsxError> for XlsxError {
    fn from(error: rust_xlsxwriter::XlsxError) -> Self {
        Self {
            message: error.to_string(),
        }
    }
}

/// Writes the table as an Excel workbook, with one sheet.
///
/// - **Typed cells.** A number is a number, a boolean a boolean, a date a date,
///   so a column sums, sorts and filters in Excel. Text is always text: Excel
///   never runs it as a formula, so nothing needs defusing as it does in a CSV.
///   An integer beyond 2^53 and a float that is not a number are written as the
///   text the grid showed, because a spreadsheet cannot hold them.
/// - **Number formats.** A column's [`CellFormat`] becomes an Excel number format:
///   `#,##0.00`, `0.0%`, a currency symbol before or after the number as the grid
///   put it, a date pattern. Excel shows it in the language of whoever opens the
///   file, so `1,234.50` reads `1.234,50` on a German machine.
/// - **Groups.** A group's heading is a bold row, indented by its level, and its
///   rows an outline Excel can collapse. A closed group has its aggregates on the
///   heading row.
/// - **Aggregates.** A column with one aggregate is a number in its column's format
///   (a count is a whole number); several are text, as in a CSV. The totals row
///   starts with the locale's "Totals".
/// - **Columns under a column group** get one header row per level, merged across
///   the columns of the group.
///
/// ```
/// use datagrid_core::{ExportCell, ExportColumn, ExportRow, ExportTable, Value};
/// use datagrid_export::{XlsxOptions, to_xlsx};
///
/// let table = ExportTable {
///     columns: vec![ExportColumn::new("age", "Age")],
///     rows: vec![ExportRow::Data(vec![ExportCell {
///         value: Some(Value::Int(31)),
///         text: "31".into(),
///     }])],
/// };
/// let bytes = to_xlsx(&table, &XlsxOptions::default())?;
/// // An `.xlsx` file is a zip archive.
/// assert_eq!(&bytes[..2], b"PK");
/// # Ok::<(), datagrid_export::XlsxError>(())
/// ```
///
/// # Errors
///
/// When the table does not fit in a sheet: more than 1,048,576 rows or 16,384
/// columns.
pub fn to_xlsx(table: &ExportTable, options: &XlsxOptions) -> Result<Vec<u8>, XlsxError> {
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet.set_name(sheet_name(&options.sheet_name))?;
    write_sheet(sheet, table, options)?;
    Ok(workbook.save_to_buffer()?)
}

/// A name Excel accepts: at most 31 characters, none of `[]:*?/\`, not empty and
/// not the reserved "History".
fn sheet_name(wanted: &str) -> String {
    let cleaned: String = wanted
        .chars()
        .map(|c| if "[]:*?/\\".contains(c) { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim_matches('\'');
    let name: String = trimmed.chars().take(31).collect();
    if name.trim().is_empty() {
        "Sheet1".to_owned()
    } else if name.eq_ignore_ascii_case("history") {
        "History_".to_owned()
    } else {
        name
    }
}

/// The formats a column's cells use, made once.
struct Style {
    /// Values of the column, in its number format and alignment.
    value: Format,
    /// The same in bold, for an aggregate.
    bold: Format,
    /// A date in a column that does not say how to show one.
    date: Format,
    /// A date and time in the same case.
    date_time: Format,
    /// A count: a whole number in bold, whatever the column shows.
    count: Format,
    /// An aggregate of the application's own, in bold, in no unit.
    custom: Format,
}

impl Style {
    fn new(column: &ExportColumn, symbol_after: bool) -> Self {
        let align = match column.align {
            CellAlign::Start => None,
            CellAlign::Center => Some(FormatAlign::Center),
            CellAlign::End => Some(FormatAlign::Right),
        };
        let with = |number: Option<&str>| {
            let mut format = Format::new();
            if let Some(align) = align {
                format = format.set_align(align);
            }
            if let Some(number) = number {
                format = format.set_num_format(number);
            }
            format
        };
        let number = number_format(&column.format, symbol_after);
        let value = with(number.as_deref());
        // A date column says how to show a date; any other falls back to ISO.
        let is_date = matches!(
            column.format,
            CellFormat::Date | CellFormat::DateTime | CellFormat::DatePattern(_)
        ) && number.is_some();
        let (date, date_time) = if is_date {
            (value.clone(), value.clone())
        } else {
            (with(Some("yyyy-mm-dd")), with(Some("yyyy-mm-dd hh:mm:ss")))
        };
        Self {
            bold: value.clone().set_bold(),
            count: with(Some("0")).set_bold(),
            custom: Format::new().set_bold(),
            value,
            date,
            date_time,
        }
    }
}

/// The Excel number format for a column's [`CellFormat`], or `None` for General.
fn number_format(format: &CellFormat, symbol_after: bool) -> Option<String> {
    let decimals = |count: u8| {
        let count = usize::from(count.min(30));
        if count == 0 {
            String::new()
        } else {
            format!(".{}", "0".repeat(count))
        }
    };
    match format {
        CellFormat::Plain => None,
        CellFormat::Number { decimals: count } => Some(format!("#,##0{}", decimals(*count))),
        CellFormat::Currency {
            symbol,
            decimals: count,
        } => {
            let body = format!("#,##0{}", decimals(*count));
            // Quoted, so that `CHF` is not read as format letters.
            let symbol = format!("\"{}\"", symbol.replace('"', ""));
            Some(if symbol_after {
                format!("{body} {symbol}")
            } else {
                format!("{symbol}{body}")
            })
        }
        CellFormat::Percent { decimals: count } => Some(format!("0{}%", decimals(*count))),
        CellFormat::Date => Some("yyyy-mm-dd".to_owned()),
        CellFormat::DateTime => Some("yyyy-mm-dd hh:mm".to_owned()),
        CellFormat::DatePattern(pattern) => date_pattern(pattern),
    }
}

/// A `chrono` pattern as an Excel date format, or `None` where it uses something
/// Excel has no word for.
fn date_pattern(pattern: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = pattern.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            // Letters and digits would be read as format codes.
            if c.is_alphanumeric() {
                out.push('\\');
            }
            out.push(c);
            continue;
        }
        out.push_str(match chars.next()? {
            'Y' => "yyyy",
            'y' => "yy",
            'm' => "mm",
            'd' => "dd",
            'e' => "d",
            'B' => "mmmm",
            'b' => "mmm",
            'A' => "dddd",
            'a' => "ddd",
            'H' => "hh",
            'M' => "mm",
            'S' => "ss",
            'F' => "yyyy-mm-dd",
            'T' => "hh:mm:ss",
            '%' => "%",
            _ => return None,
        });
    }
    Some(out)
}

/// Whether a currency column shows its symbol after the number, as the grid
/// wrote it. Read from the first cell that has any text.
fn symbol_after(table: &ExportTable, index: usize, symbol: &str) -> bool {
    table
        .rows
        .iter()
        .find_map(|row| match row {
            ExportRow::Data(cells) => cells.get(index).filter(|cell| !cell.text.is_empty()),
            _ => None,
        })
        .is_some_and(|cell| cell.text.trim_end().ends_with(symbol))
}

fn column(index: usize) -> Result<u16, XlsxError> {
    u16::try_from(index).map_err(|_| XlsxError {
        message: format!("column {} is past the last column of a sheet", index + 1),
    })
}

/// Text cut to what a cell holds.
fn clip(text: &str) -> String {
    text.chars().take(MAX_CELL_CHARS).collect()
}

/// An integer as Excel holds it: a double, which is exact up to 2^53.
fn exact_float(number: i64) -> Option<f64> {
    const LIMIT: i64 = 1 << 53;
    #[allow(clippy::cast_precision_loss)]
    (-LIMIT..=LIMIT).contains(&number).then_some(number as f64)
}

/// A date as Excel holds it; `None` where Excel has no such date (it starts in
/// 1900).
fn excel_date(value: &Value) -> Option<ExcelDateTime> {
    let parts = value.date_parts()?;
    let day =
        ExcelDateTime::from_ymd(u16::try_from(parts.year).ok()?, parts.month, parts.day).ok()?;
    match parts.time {
        None => Some(day),
        Some((hour, minute, second)) => day.and_hms(u16::from(hour), minute, second).ok(),
    }
}

/// Writes one value into a cell, as the kind of cell it is.
fn write_value(
    sheet: &mut Worksheet,
    row: u32,
    col: u16,
    value: &Value,
    shown: &str,
    style: &Style,
) -> Result<(), XlsxError> {
    if let Some(parts) = value.date_parts() {
        // A date is a number of days to Excel, shown as a date by its format.
        match excel_date(value) {
            Some(date) => {
                let format = if parts.time.is_some() {
                    &style.date_time
                } else {
                    &style.date
                };
                sheet.write_datetime_with_format(row, col, &date, format)?;
            }
            None => {
                sheet.write_string_with_format(row, col, clip(shown), &style.value)?;
            }
        }
        return Ok(());
    }
    match value {
        Value::Int(number) => match exact_float(*number) {
            Some(float) => {
                sheet.write_number_with_format(row, col, float, &style.value)?;
            }
            None => {
                sheet.write_string_with_format(row, col, clip(shown), &style.value)?;
            }
        },
        Value::Float(number) if number.is_finite() => {
            sheet.write_number_with_format(row, col, *number, &style.value)?;
        }
        Value::Bool(flag) => {
            sheet.write_boolean_with_format(row, col, *flag, &style.value)?;
        }
        Value::Text(text) => {
            sheet.write_string_with_format(row, col, clip(text), &style.value)?;
        }
        // A value this version does not know how to type is the text the grid
        // showed.
        _ => {
            sheet.write_string_with_format(row, col, clip(shown), &style.value)?;
        }
    }
    Ok(())
}

/// What a column's aggregates say as one string: `Sum 7,300.50; Count 2`.
fn aggregate_text(aggregates: &[ExportAggregate]) -> String {
    aggregates
        .iter()
        .map(|aggregate| format!("{} {}", aggregate.name, aggregate.text))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Writes one aggregate: a number where it is one, text where it is not.
fn write_aggregate(
    sheet: &mut Worksheet,
    row: u32,
    col: u16,
    aggregate: &ExportAggregate,
    style: &Style,
) -> Result<(), XlsxError> {
    let format = match aggregate.kind {
        AggregateKind::Count => &style.count,
        // What a function of the application's returns is not known to be in
        // the column's unit.
        AggregateKind::Custom(_) => &style.custom,
        _ => &style.bold,
    };
    let number = match &aggregate.value {
        Some(Value::Int(number)) => exact_float(*number),
        Some(Value::Float(number)) if number.is_finite() => Some(*number),
        _ => None,
    };
    match number {
        Some(number) => sheet.write_number_with_format(row, col, number, format)?,
        None => sheet.write_string_with_format(
            row,
            col,
            clip(&format!("{} {}", aggregate.name, aggregate.text)),
            &style.bold,
        )?,
    };
    Ok(())
}

/// What heads an aggregates row: a group's caption or the locale's "Totals", the
/// format to write it in, and what joins it to the first column's own aggregate.
type Lead<'a> = (&'a str, &'a Format, &'a str);

/// Writes a row of aggregates, one column at a time. `lead` is text for the first
/// column.
fn write_aggregates(
    sheet: &mut Worksheet,
    row: u32,
    columns: &[Vec<ExportAggregate>],
    lead: Option<Lead<'_>>,
    styles: &[Style],
) -> Result<(), XlsxError> {
    for (index, (aggregates, style)) in columns.iter().zip(styles).enumerate() {
        let col = column(index)?;
        // The first column is where a row is named, so what it aggregates is text
        // there: a bare `3` under "Name" would not say it is a count. A caption or
        // the totals label shares the cell with it.
        if index == 0 && (lead.is_some() || !aggregates.is_empty()) {
            let aggregated = aggregate_text(aggregates);
            let (text, format) = match lead {
                Some((lead, format, _)) if aggregated.is_empty() => (lead.to_owned(), format),
                Some((lead, format, joiner)) => (format!("{lead}{joiner}{aggregated}"), format),
                None => (aggregated, &style.bold),
            };
            sheet.write_string_with_format(row, col, clip(&text), format)?;
            continue;
        }
        match aggregates.as_slice() {
            [] => {}
            [one] => write_aggregate(sheet, row, col, one, style)?,
            _ => {
                sheet.write_string_with_format(
                    row,
                    col,
                    clip(&aggregate_text(aggregates)),
                    &style.bold,
                )?;
            }
        }
    }
    Ok(())
}

/// Closes the outlines at `level` and deeper, which end at `last`.
fn close_groups(
    open: &mut Vec<(usize, u32)>,
    level: usize,
    last: u32,
    spans: &mut Vec<(u32, u32)>,
) {
    while let Some(&(opened_at, first)) = open.last() {
        if opened_at < level {
            break;
        }
        open.pop();
        if first <= last && opened_at < MAX_OUTLINE_LEVELS {
            spans.push((first, last));
        }
    }
}

/// The header: a row for each level of column group, then the labels. Returns the
/// row the labels are on.
fn write_header(
    sheet: &mut Worksheet,
    table: &ExportTable,
    heading: &Format,
) -> Result<u32, XlsxError> {
    let width = table.columns.len();
    let levels = table
        .columns
        .iter()
        .map(|column| column.group_path.len())
        .max()
        .unwrap_or(0);
    let mut row: u32 = 0;
    for level in 0..levels {
        // Runs of neighbouring columns that sit under the same group at this
        // level, each as (first column, last column, the group's name).
        let mut runs: Vec<(usize, usize, &str)> = Vec::new();
        for (index, spec) in table.columns.iter().enumerate() {
            let Some(label) = spec.group_path.get(level) else {
                continue;
            };
            let same_group = |previous: &(usize, usize, &str)| {
                previous.1 + 1 == index
                    && table
                        .columns
                        .get(previous.0)
                        .and_then(|first| first.group_path.get(..=level))
                        == spec.group_path.get(..=level)
            };
            match runs.last_mut() {
                Some(previous) if same_group(previous) => previous.1 = index,
                _ => runs.push((index, index, label)),
            }
        }
        for (first, last, label) in runs {
            if last > first {
                sheet.merge_range(
                    row,
                    column(first)?,
                    row,
                    column(last)?,
                    &clip(label),
                    heading,
                )?;
            } else {
                sheet.write_string_with_format(row, column(first)?, clip(label), heading)?;
            }
        }
        row += 1;
    }
    for (index, spec) in table.columns.iter().enumerate() {
        sheet.write_string_with_format(row, column(index)?, clip(&spec.label), heading)?;
    }
    if width > 0 {
        sheet.set_freeze_panes(row + 1, 0)?;
    }
    Ok(row)
}

fn write_sheet(
    sheet: &mut Worksheet,
    table: &ExportTable,
    options: &XlsxOptions,
) -> Result<(), XlsxError> {
    let width = table.columns.len();
    let styles: Vec<Style> = table
        .columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let after = match &column.format {
                CellFormat::Currency { symbol, .. } => symbol_after(table, index, symbol),
                _ => false,
            };
            Style::new(column, after)
        })
        .collect();
    let mut widths: Vec<usize> = table
        .columns
        .iter()
        .map(|column| column.label.chars().count())
        .collect();

    let bold = Format::new().set_bold();
    let heading = bold.clone().set_border_bottom(FormatBorder::Thin);
    let (label_row, mut row) = if options.header {
        let label_row = write_header(sheet, table, &heading)?;
        (label_row, label_row + 1)
    } else {
        (0, 0)
    };
    let first_data_row = row;

    let mut open: Vec<(usize, u32)> = Vec::new();
    let mut spans: Vec<(u32, u32)> = Vec::new();
    let mut only_data = true;
    let mut sampled = 0;

    for table_row in &table.rows {
        match table_row {
            ExportRow::Data(cells) => {
                for (index, (cell, style)) in cells.iter().zip(&styles).enumerate() {
                    if let Some(value) = &cell.value {
                        write_value(sheet, row, column(index)?, value, &cell.text, style)?;
                    }
                    if sampled < WIDTH_SAMPLE_ROWS
                        && let Some(slot) = widths.get_mut(index)
                    {
                        *slot = (*slot).max(cell.text.chars().count());
                    }
                }
                sampled += 1;
                row += 1;
            }
            ExportRow::GroupHeader {
                level,
                caption,
                expanded,
                aggregates,
                ..
            } => {
                only_data = false;
                if options.outline {
                    close_groups(&mut open, *level, row.saturating_sub(1), &mut spans);
                }
                let indent = u8::try_from(*level).unwrap_or(u8::MAX);
                let lead = bold.clone().set_indent(indent);
                if aggregates.is_empty() {
                    sheet.write_string_with_format(row, 0, clip(caption), &lead)?;
                } else {
                    write_aggregates(
                        sheet,
                        row,
                        aggregates,
                        Some((caption, &lead, "; ")),
                        &styles,
                    )?;
                }
                if options.outline && *expanded {
                    open.push((*level, row + 1));
                }
                row += 1;
            }
            ExportRow::Aggregates {
                level,
                label,
                cells,
            } => {
                only_data = false;
                // The totals end every group; a group's own aggregates are its
                // last row.
                if level.is_none() && options.outline {
                    close_groups(&mut open, 0, row.saturating_sub(1), &mut spans);
                }
                let lead = label.as_deref().map(|label| (label, &bold, ": "));
                write_aggregates(sheet, row, cells, lead, &styles)?;
                if let (Some(level), true) = (level, options.outline) {
                    close_groups(&mut open, *level, row, &mut spans);
                }
                row += 1;
            }
            // A row kind added later is one this version does not know how to
            // write; skipping it is better than writing nonsense.
            _ => {}
        }
    }
    if options.outline {
        close_groups(&mut open, 0, row.saturating_sub(1), &mut spans);
    }

    if !spans.is_empty() {
        // The button that folds a group sits on its heading, above its rows.
        sheet.group_symbols_above(true);
        for (first, last) in spans {
            sheet.group_rows(first, last)?;
        }
    }

    if options.autofilter && options.header && only_data && row > first_data_row && width > 0 {
        sheet.autofilter(label_row, 0, row - 1, column(width - 1)?)?;
    }

    if options.auto_width {
        for (index, chars) in widths.into_iter().enumerate() {
            #[allow(clippy::cast_precision_loss)]
            let wanted = (chars as f64 + 2.0).clamp(WIDTH_RANGE.0, WIDTH_RANGE.1);
            sheet.set_column_width(column(index)?, wanted)?;
        }
    }
    Ok(())
}
