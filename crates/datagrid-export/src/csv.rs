//! CSV, as RFC 4180 writes it, with the choices a file for Excel needs.

use datagrid_core::{ExportAggregate, ExportRow, ExportTable, Value};
use std::borrow::Cow;

/// How a line ends.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LineEnding {
    /// `\r\n`, which RFC 4180 specifies and every spreadsheet reads.
    #[default]
    Crlf,
    /// `\n`, which Unix tools expect.
    Lf,
}

/// What the cells say.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CsvValues {
    /// The values themselves, for a program to read: `4200.5`, `true`,
    /// `2026-10-16`. No thousands separator, no currency symbol, and a decimal
    /// point whatever the locale.
    #[default]
    Raw,
    /// The text the grid shows: `€4,200.50`, `Yes`, `16.10.2026`. For a person
    /// who will read the file as it is on screen.
    Shown,
}

/// How [`to_csv`] writes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CsvOptions {
    /// What separates the fields: a comma, or a semicolon for the spreadsheets
    /// of a locale that writes its decimal comma.
    pub delimiter: char,
    /// How a line ends.
    pub line_ending: LineEnding,
    /// Whether the first line names the columns.
    pub header: bool,
    /// Whether the cells carry the values or the text shown.
    pub values: CsvValues,
    /// Whether to start the file with a byte order mark. Excel reads a file
    /// without one as the system's code page and mangles every umlaut; most
    /// other readers accept it silently.
    pub bom: bool,
    /// Whether to defuse text that a spreadsheet would run as a formula.
    ///
    /// A cell that starts with `=`, `+`, `-`, `@`, a tab or a carriage return is
    /// a formula to Excel, and a grid shows what its users typed. Such text gets
    /// a `'` in front, which the spreadsheet shows as nothing. Numbers are not
    /// touched: `-5` stays `-5`. Turn it off only for a file that no
    /// spreadsheet will open.
    pub guard_formulas: bool,
}

impl Default for CsvOptions {
    fn default() -> Self {
        Self {
            delimiter: ',',
            line_ending: LineEnding::Crlf,
            header: true,
            values: CsvValues::Raw,
            bom: false,
            guard_formulas: true,
        }
    }
}

/// Writes the table as CSV.
///
/// One line per data row, under a line of the column labels. A group's heading
/// is a line of its own with the caption in the first field, and its aggregates
/// follow its rows as a line with each in its column's field: `Sum €7,300.50`, or
/// several of them joined with `; `. The totals line starts with the locale's
/// "Totals". Where the table has no groups, none of that shows.
///
/// ```
/// use datagrid_core::{ExportCell, ExportColumn, ExportRow, ExportTable, Value};
/// use datagrid_export::{CsvOptions, to_csv};
///
/// let table = ExportTable {
///     columns: vec![ExportColumn::new("note", "note")],
///     rows: vec![ExportRow::Data(vec![ExportCell {
///         value: Some(Value::Text("said \"hi\", left".into())),
///         text: "said \"hi\", left".into(),
///     }])],
/// };
/// assert_eq!(
///     to_csv(&table, &CsvOptions::default()),
///     "note\r\n\"said \"\"hi\"\", left\"\r\n",
/// );
/// ```
#[must_use]
pub fn to_csv(table: &ExportTable, options: &CsvOptions) -> String {
    let writer = Writer { options };
    let width = table.columns.len();
    let mut out = String::new();
    if options.bom {
        out.push('\u{feff}');
    }

    if options.header {
        let labels: Vec<String> = table
            .columns
            .iter()
            .map(|column| writer.guard(&column.label).into_owned())
            .collect();
        writer.record(&mut out, &labels);
    }

    for row in &table.rows {
        let fields: Vec<String> = match row {
            ExportRow::Data(cells) => cells
                .iter()
                .map(|cell| {
                    let text = writer.value_text(cell.value.as_ref(), &cell.text);
                    match &cell.value {
                        // Only text can be taken for a formula.
                        Some(Value::Text(_)) => writer.guard(&text).into_owned(),
                        _ => text,
                    }
                })
                .collect(),
            ExportRow::GroupHeader {
                caption,
                aggregates,
                ..
            } => {
                let mut fields = writer.aggregate_fields(aggregates);
                // The caption heads the line; what the first column aggregates
                // follows it, so nothing a closed group says is dropped.
                let first = fields.first().cloned().unwrap_or_default();
                let heading = writer.guard(caption);
                let head = if first.is_empty() {
                    heading.into_owned()
                } else {
                    format!("{heading}; {first}")
                };
                match fields.first_mut() {
                    Some(slot) => *slot = head,
                    None => fields.push(head),
                }
                fields
            }
            ExportRow::Aggregates { label, cells, .. } => {
                let mut fields = writer.aggregate_fields(cells);
                if let Some(label) = label {
                    let first = fields.first().cloned().unwrap_or_default();
                    let head = if first.is_empty() {
                        label.clone()
                    } else {
                        format!("{label}: {first}")
                    };
                    match fields.first_mut() {
                        Some(slot) => *slot = head,
                        None => fields.push(head),
                    }
                }
                fields
            }
            // A row kind added later is one this version does not know how to
            // write; skipping it is better than writing nonsense.
            _ => continue,
        };

        // Every line has one field per column, whatever the row had to say.
        let mut fields = fields;
        fields.resize(width.max(fields.len()), String::new());
        writer.record(&mut out, &fields);
    }
    out
}

struct Writer<'a> {
    options: &'a CsvOptions,
}

impl Writer<'_> {
    /// One cell's text, as a value or as shown.
    fn value_text(&self, value: Option<&Value>, shown: &str) -> String {
        if self.options.values == CsvValues::Shown {
            return shown.to_owned();
        }
        match value {
            None => String::new(),
            Some(Value::Text(text)) => text.clone(),
            Some(Value::Int(number)) => number.to_string(),
            // A float that is not a number has no value to write; what the grid
            // showed for it is the best there is.
            Some(Value::Float(number)) if number.is_finite() => number.to_string(),
            Some(Value::Bool(flag)) => flag.to_string(),
            // A date in ISO 8601, which is what a program reads; the text shown is
            // in the locale's pattern.
            Some(other) => other.iso_date().unwrap_or_else(|| shown.to_owned()),
        }
    }

    /// The text of one column's aggregates: `Sum €7,300.50; Count 2`.
    fn aggregate_fields(&self, columns: &[Vec<ExportAggregate>]) -> Vec<String> {
        columns
            .iter()
            .map(|aggregates| {
                aggregates
                    .iter()
                    .filter(|aggregate| aggregate.value.is_some())
                    .map(|aggregate| {
                        // The text shown, whatever `values` says: an aggregate line is
                        // a caption for a reader (`Average 29.33`), not a cell of data.
                        format!("{} {}", aggregate.name, aggregate.text)
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .collect()
    }

    /// Defuses text a spreadsheet would run as a formula.
    fn guard<'t>(&self, text: &'t str) -> Cow<'t, str> {
        let risky = matches!(
            text.chars().next(),
            Some('=' | '+' | '-' | '@' | '\t' | '\r')
        );
        if self.options.guard_formulas && risky {
            Cow::Owned(format!("'{text}"))
        } else {
            Cow::Borrowed(text)
        }
    }

    /// One line: the fields, quoted where they need it, and the line ending.
    fn record(&self, out: &mut String, fields: &[String]) {
        // A table with no columns has no lines either, not a blank one.
        if fields.is_empty() {
            return;
        }
        let delimiter = self.options.delimiter;
        for (index, field) in fields.iter().enumerate() {
            if index > 0 {
                out.push(delimiter);
            }
            let needs_quotes = field
                .chars()
                .any(|c| c == delimiter || matches!(c, '"' | '\n' | '\r'));
            if needs_quotes {
                out.push('"');
                for c in field.chars() {
                    if c == '"' {
                        out.push('"');
                    }
                    out.push(c);
                }
                out.push('"');
            } else {
                out.push_str(field);
            }
        }
        out.push_str(match self.options.line_ending {
            LineEnding::Crlf => "\r\n",
            LineEnding::Lf => "\n",
        });
    }
}
