//! The server side: a SQLite table and the translation of a `GridQuery` into SQL.
//!
//! Compiled only with the `server` feature. The client build never sees
//! `rusqlite` or this module.

use crate::Employee;
use datagrid_core::{
    AggregateKind, AggregateValue, ColumnFilter, ColumnId, Condition, DistinctValues, FilterOp,
    GridQuery, GroupKey, GroupSummary, GroupedPage, Page, SortDirection, Value, ValueKind,
};
use rusqlite::{Connection, params_from_iter, types::Value as SqlValue};
use std::sync::{LazyLock, Mutex};

/// A column the client may sort, filter or search by, and the SQL it maps to.
///
/// **This list is the security boundary.** Column ids arrive from the client,
/// so they are looked up here and never put into SQL themselves. An id that is
/// not listed is ignored, the same way the grid ignores unknown ids locally.
struct SqlColumn {
    id: &'static str,
    /// Trusted SQL, written here rather than received.
    expr: &'static str,
    /// What the column holds, which decides how operands are read. Text
    /// columns also sort and compare case-insensitively, as the grid does.
    kind: ValueKind,
    /// Whether the global search scans this column.
    searchable: bool,
}

const COLUMNS: [SqlColumn; 4] = [
    SqlColumn {
        id: "name",
        expr: "name",
        kind: ValueKind::Text,
        searchable: true,
    },
    SqlColumn {
        id: "department",
        expr: "department",
        kind: ValueKind::Text,
        searchable: true,
    },
    SqlColumn {
        id: "city",
        expr: "city",
        kind: ValueKind::Text,
        searchable: true,
    },
    SqlColumn {
        id: "salary",
        expr: "salary",
        kind: ValueKind::Number,
        searchable: false,
    },
];

fn column(id: &str) -> Option<&'static SqlColumn> {
    COLUMNS.iter().find(|column| column.id == id)
}

/// Escapes `LIKE` wildcards so user text matches literally.
fn escape_like(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// A `LIKE` pattern matching `text` anywhere, with its wildcards taken literally.
fn contains_pattern(text: &str) -> SqlValue {
    SqlValue::Text(format!("%{}%", escape_like(text)))
}

/// A filter operand as a bound parameter.
fn param(value: &Value) -> SqlValue {
    match value {
        Value::Text(text) => SqlValue::Text(text.clone()),
        Value::Int(value) => SqlValue::Integer(*value),
        Value::Float(value) => SqlValue::Real(*value),
        Value::Bool(value) => SqlValue::Integer(i64::from(*value)),
        // SQLite has no date type; ISO 8601 text sorts and compares correctly.
        Value::Date(value) => SqlValue::Text(value.format("%Y-%m-%d").to_string()),
        Value::DateTime(value) => SqlValue::Text(value.format("%Y-%m-%d %H:%M:%S").to_string()),
    }
}

/// SQL for one condition on `column`, pushing its operands onto `params`.
///
/// Mirrors [`Condition::matches`]: operands are read as the column's kind
/// first, text compares ignoring case (SQLite's `NOCASE`, which folds ASCII —
/// enough for this data), an empty cell passes "is empty" and nothing that
/// compares, and an operand that does not read as the column's kind compares
/// with nothing.
fn condition_sql(column: &SqlColumn, condition: &Condition, params: &mut Vec<SqlValue>) -> String {
    let expr = column.expr;
    let text = column.kind == ValueKind::Text;
    let collate = if text { " COLLATE NOCASE" } else { "" };
    let prepared = condition.prepared(column.kind);
    let operand = |index: usize| {
        prepared
            .values
            .get(index)
            .filter(|value| value.coerce(column.kind).is_some())
            .map(param)
    };
    // A number searched as text: its plain form, as locally.
    let as_text = if text {
        expr.to_owned()
    } else {
        format!("CAST({expr} AS TEXT)")
    };

    match prepared.op {
        FilterOp::IsEmpty if text => format!("({expr} IS NULL OR trim({expr}) = '')"),
        FilterOp::IsEmpty => format!("{expr} IS NULL"),
        FilterOp::IsNotEmpty if text => format!("({expr} IS NOT NULL AND trim({expr}) <> '')"),
        FilterOp::IsNotEmpty => format!("{expr} IS NOT NULL"),
        FilterOp::Contains | FilterOp::StartsWith | FilterOp::EndsWith => {
            // No operand filters nothing; a number operand is searched as text.
            let Some(needle) = prepared.values.first().map(Value::edit_text) else {
                return "1".to_owned();
            };
            let needle = needle.as_str();
            let pattern = match prepared.op {
                FilterOp::StartsWith => format!("{}%", escape_like(needle)),
                FilterOp::EndsWith => format!("%{}", escape_like(needle)),
                _ => format!("%{}%", escape_like(needle)),
            };
            params.push(SqlValue::Text(pattern));
            format!("{as_text} LIKE ? ESCAPE '\\'")
        }
        FilterOp::OneOf => {
            let values: Vec<SqlValue> = prepared
                .values
                .iter()
                .filter(|value| value.coerce(column.kind).is_some())
                .map(param)
                .collect();
            if values.is_empty() {
                return "0".to_owned();
            }
            let marks = vec!["?"; values.len()].join(", ");
            params.extend(values);
            format!("{expr}{collate} IN ({marks})")
        }
        FilterOp::NotEquals => match operand(0) {
            Some(value) => {
                params.push(value);
                format!("({expr} IS NOT NULL AND {expr}{collate} <> ?)")
            }
            // Nothing to be equal to, so every value differs.
            None => format!("{expr} IS NOT NULL"),
        },
        FilterOp::Between => match (operand(0), operand(1)) {
            // Either order, as locally: `20..10` is the range `10..20`.
            (Some(low), Some(high)) => {
                params.extend([low.clone(), high.clone(), high, low]);
                format!(
                    "(({expr}{collate} >= ? AND {expr}{collate} <= ?) \
                     OR ({expr}{collate} >= ? AND {expr}{collate} <= ?))"
                )
            }
            _ => "0".to_owned(),
        },
        op => {
            let symbol = match op {
                FilterOp::Less => "<",
                FilterOp::LessOrEqual => "<=",
                FilterOp::Greater => ">",
                FilterOp::GreaterOrEqual => ">=",
                _ => "=",
            };
            match operand(0) {
                Some(value) => {
                    params.push(value);
                    format!("{expr}{collate} {symbol} ?")
                }
                None => "0".to_owned(),
            }
        }
    }
}

/// SQL for a column's filter: its conditions joined with `AND` or `OR`.
fn filter_sql(column: &SqlColumn, filter: &ColumnFilter, params: &mut Vec<SqlValue>) -> String {
    let parts: Vec<String> = filter
        .conditions
        .iter()
        .map(|condition| condition_sql(column, condition, params))
        .collect();
    let joiner = if filter.any { " OR " } else { " AND " };
    format!("({})", parts.join(joiner))
}

/// `WHERE`, `ORDER BY` and the bound parameters for `query`.
///
/// User text only ever travels as a bound parameter. SQLite's `LIKE` is
/// case-insensitive for ASCII, matching the grid's local filtering.
struct Sql {
    where_clause: String,
    order_by: String,
    params: Vec<SqlValue>,
}

/// Translates `query`, leaving out the filters on `except`: a value list shows
/// the values its own column's filter would hide.
fn translate(query: &GridQuery, except: Option<&ColumnId>) -> Sql {
    let mut conditions = Vec::new();
    let mut params = Vec::new();

    // The filter bar's text, read exactly as the grid reads it locally, then
    // the typed filters of the filter menus.
    let bar = query.column_filters.iter().filter_map(|(id, text)| {
        let column = column(id.as_str())?;
        Some((id, column, ColumnFilter::from_bar_text(text, column.kind)?))
    });
    let typed = query
        .filters
        .iter()
        .filter_map(|(id, filter)| Some((id, column(id.as_str())?, filter.clone())));
    for (id, column, filter) in bar.chain(typed) {
        if except != Some(id) && !filter.is_empty() {
            conditions.push(filter_sql(column, &filter, &mut params));
        }
    }

    // Search: any searchable column may match.
    if let Some(text) = &query.search {
        let alternatives: Vec<String> = COLUMNS
            .iter()
            .filter(|column| column.searchable)
            .map(|column| {
                params.push(contains_pattern(text));
                format!("{} LIKE ? ESCAPE '\\'", column.expr)
            })
            .collect();
        conditions.push(format!("({})", alternatives.join(" OR ")));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    // Sort in priority order, then by id: a total order, so a row cannot move
    // between pages from one request to the next. Ids ascend in insertion order,
    // which also reproduces the grid's stable sort.
    let mut keys: Vec<String> = query
        .sort
        .iter()
        .filter_map(|sort| {
            let column = column(sort.column.as_str())?;
            let collation = if column.kind == ValueKind::Text {
                " COLLATE NOCASE"
            } else {
                ""
            };
            let direction = match sort.direction {
                SortDirection::Asc => "ASC",
                SortDirection::Desc => "DESC",
            };
            Some(format!("{}{collation} {direction}", column.expr))
        })
        .collect();
    keys.push("id ASC".to_owned());

    Sql {
        where_clause,
        order_by: format!("ORDER BY {}", keys.join(", ")),
        params,
    }
}

/// `where_clause` with one more condition.
fn and(where_clause: &str, condition: &str) -> String {
    if where_clause.is_empty() {
        format!("WHERE {condition}")
    } else {
        format!("{where_clause} AND {condition}")
    }
}

/// The distinct values of `column_id` among the rows that pass every filter in
/// `query` but the column's own, with counts, at most `limit` of them: the
/// answer to a filter menu's value list.
pub fn distinct_values(
    connection: &Connection,
    column_id: &ColumnId,
    query: &GridQuery,
    limit: usize,
) -> rusqlite::Result<DistinctValues> {
    let Some(column) = column(column_id.as_str()) else {
        return Ok(DistinctValues::default());
    };
    let sql = translate(query, Some(column_id));
    let expr = column.expr;
    // As the grid sorts: text ignoring case, ties by exact text.
    let order = if column.kind == ValueKind::Text {
        format!("{expr} COLLATE NOCASE, {expr}")
    } else {
        expr.to_owned()
    };

    let mut params = sql.params.clone();
    // One more than wanted, to know whether there were more.
    params.push(SqlValue::Integer(
        i64::try_from(limit.saturating_add(1)).unwrap_or(i64::MAX),
    ));
    let mut statement = connection.prepare(&format!(
        "SELECT {expr}, COUNT(*) FROM employees {} GROUP BY {expr} ORDER BY {order} LIMIT ?",
        and(&sql.where_clause, &format!("{expr} IS NOT NULL")),
    ))?;
    let mut values = statement
        .query_map(params_from_iter(params.iter()), |row| {
            let value = match row.get::<_, SqlValue>(0)? {
                SqlValue::Integer(value) => Value::Int(value),
                SqlValue::Real(value) => Value::Float(value),
                SqlValue::Text(value) => Value::Text(value),
                other => Value::Text(format!("{other:?}")),
            };
            let count: i64 = row.get(1)?;
            Ok((value, usize::try_from(count).unwrap_or(0)))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let truncated = values.len() > limit;
    values.truncate(limit);

    let empty: i64 = connection.query_row(
        &format!(
            "SELECT COUNT(*) FROM employees {}",
            and(&sql.where_clause, &format!("{expr} IS NULL"))
        ),
        params_from_iter(sql.params.iter()),
        |row| row.get(0),
    )?;

    Ok(DistinctValues {
        values,
        empty: usize::try_from(empty).unwrap_or(0),
        truncated,
    })
}

/// Reads a SQL result as a grid value; `NULL` is no value.
fn value_of(value: SqlValue) -> Option<Value> {
    match value {
        SqlValue::Null => None,
        SqlValue::Integer(value) => Some(Value::Int(value)),
        SqlValue::Real(value) => Some(Value::Float(value)),
        SqlValue::Text(value) => Some(Value::Text(value)),
        SqlValue::Blob(_) => None,
    }
}

/// The SQL for an aggregate, as the grid computes it locally: sums and means
/// of numbers only, the smallest and largest in the column's order, counts of
/// values. `None` for a custom aggregate, which only the client knows.
fn aggregate_sql(column: &SqlColumn, kind: &AggregateKind) -> Option<String> {
    let expr = column.expr;
    let number = column.kind == ValueKind::Number;
    let ordered = if column.kind == ValueKind::Text {
        format!("{expr} COLLATE NOCASE")
    } else {
        expr.to_owned()
    };
    Some(match kind {
        AggregateKind::Sum if number => format!("SUM({expr})"),
        AggregateKind::Average if number => format!("AVG({expr})"),
        AggregateKind::Sum | AggregateKind::Average => "NULL".to_owned(),
        AggregateKind::Min => format!("MIN({ordered})"),
        AggregateKind::Max => format!("MAX({ordered})"),
        AggregateKind::Count => format!("COUNT({expr})"),
        AggregateKind::Custom(_) => return None,
    })
}

/// The aggregates `query` asks for that SQL can compute, with their SQL.
fn aggregates(query: &GridQuery) -> Vec<(ColumnId, AggregateKind, String)> {
    query
        .aggregates
        .iter()
        .filter_map(|(id, kind)| {
            let sql = aggregate_sql(column(id.as_str())?, kind)?;
            Some((id.clone(), kind.clone(), sql))
        })
        .collect()
}

/// Reads the aggregates from `row`, starting at column `first`.
fn read_aggregates(
    row: &rusqlite::Row<'_>,
    first: usize,
    wanted: &[(ColumnId, AggregateKind, String)],
) -> rusqlite::Result<Vec<AggregateValue>> {
    wanted
        .iter()
        .enumerate()
        .map(|(offset, (column, kind, _))| {
            Ok(AggregateValue {
                column: column.clone(),
                kind: kind.clone(),
                value: value_of(row.get(first + offset)?),
            })
        })
        .collect()
}

/// How a grouped column orders its groups: in its sort direction, ascending
/// if it is not sorted, empty values last as the grid has them.
fn group_order(query: &GridQuery, column: &SqlColumn) -> String {
    let direction = query
        .sort
        .iter()
        .find(|sort| sort.column.as_str() == column.id)
        .map_or(SortDirection::Asc, |sort| sort.direction);
    let (direction, nulls) = match direction {
        SortDirection::Asc => ("ASC", "NULLS LAST"),
        SortDirection::Desc => ("DESC", "NULLS FIRST"),
    };
    let expr = column.expr;
    if column.kind == ValueKind::Text {
        // Ignoring case, then by exact text, as the grid sorts text.
        format!("{expr} COLLATE NOCASE {direction} {nulls}, {expr} {direction}")
    } else {
        format!("{expr} {direction} {nulls}")
    }
}

/// A grouped page: the groups of each level counted by one `GROUP BY`, the
/// page planned from those counts, and only the rows the page shows fetched.
fn query_grouped(
    connection: &Connection,
    query: &GridQuery,
    grouped: &[&'static SqlColumn],
) -> rusqlite::Result<Page<Employee>> {
    let sql = translate(query, None);
    let wanted = aggregates(query);
    let aggregate_list: String = wanted
        .iter()
        .map(|(_, _, sql)| format!(", {sql}"))
        .collect();

    let mut levels = Vec::with_capacity(grouped.len());
    for depth in 1..=grouped.len() {
        let columns = grouped.get(..depth).unwrap_or_default();
        let keys: Vec<&str> = columns.iter().map(|column| column.expr).collect();
        let keys = keys.join(", ");
        let order: Vec<String> = columns
            .iter()
            .map(|column| group_order(query, column))
            .collect();
        let mut statement = connection.prepare(&format!(
            "SELECT {keys}, COUNT(*){aggregate_list} FROM employees {} GROUP BY {keys} ORDER BY {}",
            sql.where_clause,
            order.join(", ")
        ))?;
        let level = statement
            .query_map(params_from_iter(sql.params.iter()), |row| {
                let key = (0..depth)
                    .map(|index| row.get(index).map(value_of))
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                let count: i64 = row.get(depth)?;
                Ok(GroupSummary {
                    key: GroupKey(key),
                    count: usize::try_from(count).unwrap_or(0),
                    aggregates: read_aggregates(row, depth + 1, &wanted)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        levels.push(level);
    }

    // Plan with the columns the SQL knows, so levels and columns line up.
    let known = GridQuery {
        group_by: grouped
            .iter()
            .map(|column| ColumnId::new(column.id))
            .collect(),
        ..query.clone()
    };
    let plan = GroupedPage::plan(&levels, &known);

    let mut fetched = Vec::new();
    for (key, offset, limit) in plan.fetches() {
        let mut params = sql.params.clone();
        let mut conditions = Vec::new();
        for (column, value) in grouped.iter().zip(&key.0) {
            match value {
                Some(value) => {
                    conditions.push(format!("{} = ?", column.expr));
                    params.push(param(value));
                }
                None => conditions.push(format!("{} IS NULL", column.expr)),
            }
        }
        params.push(SqlValue::Integer(i64::try_from(limit).unwrap_or(i64::MAX)));
        params.push(SqlValue::Integer(i64::try_from(offset).unwrap_or(i64::MAX)));
        fetched.push(select_employees(
            connection,
            &format!(
                "{} {} LIMIT ? OFFSET ?",
                and(&sql.where_clause, &conditions.join(" AND ")),
                sql.order_by
            ),
            &params,
        )?);
    }

    let totals = totals(connection, &sql, &wanted)?;
    Ok(plan.into_page(fetched, totals))
}

/// The aggregates over every row that matches.
fn totals(
    connection: &Connection,
    sql: &Sql,
    wanted: &[(ColumnId, AggregateKind, String)],
) -> rusqlite::Result<Vec<AggregateValue>> {
    if wanted.is_empty() {
        return Ok(Vec::new());
    }
    let list: Vec<&str> = wanted.iter().map(|(_, _, sql)| sql.as_str()).collect();
    connection.query_row(
        &format!(
            "SELECT {} FROM employees {}",
            list.join(", "),
            sql.where_clause
        ),
        params_from_iter(sql.params.iter()),
        |row| read_aggregates(row, 0, wanted),
    )
}

/// Employees selected by the SQL after `FROM employees`.
fn select_employees(
    connection: &Connection,
    rest: &str,
    params: &[SqlValue],
) -> rusqlite::Result<Vec<Employee>> {
    let mut statement = connection.prepare(&format!(
        "SELECT id, name, department, city, salary FROM employees {rest}"
    ))?;
    statement
        .query_map(params_from_iter(params.iter()), |row| {
            Ok(Employee {
                id: row.get(0)?,
                name: row.get(1)?,
                department: row.get(2)?,
                city: row.get(3)?,
                salary: row.get(4)?,
            })
        })?
        .collect()
}

/// Runs `query` against `connection`: one page of rows and the total match
/// count, grouped if the query groups by columns the SQL knows.
pub fn query_employees(
    connection: &Connection,
    query: &GridQuery,
) -> rusqlite::Result<Page<Employee>> {
    let grouped: Vec<&'static SqlColumn> = query
        .group_by
        .iter()
        .filter_map(|id| column(id.as_str()))
        .collect();
    if !grouped.is_empty() {
        return query_grouped(connection, query, &grouped);
    }

    let sql = translate(query, None);

    let total: i64 = connection.query_row(
        &format!("SELECT COUNT(*) FROM employees {}", sql.where_clause),
        params_from_iter(sql.params.iter()),
        |row| row.get(0),
    )?;

    let mut page_params = sql.params.clone();
    page_params.push(SqlValue::Integer(
        i64::try_from(query.page_size).unwrap_or(i64::MAX),
    ));
    page_params.push(SqlValue::Integer(
        i64::try_from(query.offset()).unwrap_or(i64::MAX),
    ));
    let rows = select_employees(
        connection,
        &format!("{} {} LIMIT ? OFFSET ?", sql.where_clause, sql.order_by),
        &page_params,
    )?;

    let totals = totals(connection, &sql, &aggregates(query))?;
    Ok(Page::new(rows, usize::try_from(total).unwrap_or(0)).with_totals(totals))
}

/// Creates the table and fills it with `employees`.
pub fn seed(connection: &Connection, employees: &[Employee]) -> rusqlite::Result<()> {
    connection.execute_batch(
        "CREATE TABLE employees (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            department TEXT NOT NULL,
            city TEXT NOT NULL,
            salary INTEGER NOT NULL
        );",
    )?;
    let mut insert = connection.prepare(
        "INSERT INTO employees (id, name, department, city, salary) VALUES (?, ?, ?, ?, ?)",
    )?;
    for employee in employees {
        insert.execute((
            employee.id,
            &employee.name,
            &employee.department,
            &employee.city,
            employee.salary,
        ))?;
    }
    Ok(())
}

/// The highest salary the server accepts. The client does not know it, so an
/// edit above it fails on the server, where business rules belong.
pub const SALARY_BAND: u32 = 250_000;

/// Writes an edited employee back, after the server's own checks.
///
/// # Errors
///
/// A message for the user if a check fails or no such employee exists; the
/// database's error otherwise.
pub fn update_employee(connection: &Connection, employee: &Employee) -> Result<(), String> {
    if employee.name.trim().is_empty() {
        return Err("a name is required".into());
    }
    if employee.salary > SALARY_BAND {
        return Err(format!("{} is above the salary band", employee.salary));
    }
    let changed = connection
        .execute(
            "UPDATE employees SET name = ?1, department = ?2, city = ?3, salary = ?4 WHERE id = ?5",
            (
                &employee.name,
                &employee.department,
                &employee.city,
                employee.salary,
                employee.id,
            ),
        )
        .map_err(|error| error.to_string())?;
    if changed == 0 {
        return Err(format!("no employee {}", employee.id));
    }
    Ok(())
}

/// The server's database: in memory, seeded on first use.
///
/// A `Mutex` around one connection is plenty for an example. A real server
/// would use a connection pool and run queries off the async executor.
pub static DATABASE: LazyLock<Result<Mutex<Connection>, String>> = LazyLock::new(|| {
    let connection = Connection::open_in_memory().map_err(|error| error.to_string())?;
    seed(&connection, &crate::employees()).map_err(|error| error.to_string())?;
    Ok(Mutex::new(connection))
});

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;
    use datagrid_core::{Aggregate, ColumnSpec, GridState, PageState, SortState, compute_view};

    fn database() -> (Connection, Vec<Employee>) {
        let connection = Connection::open_in_memory().unwrap();
        let employees = crate::employees();
        seed(&connection, &employees).unwrap();
        (connection, employees)
    }

    /// The grid's own sorting and filtering, which the SQL must reproduce.
    fn local_columns() -> Vec<ColumnSpec<Employee>> {
        vec![
            ColumnSpec::new("name")
                .sort_by_text(|row: &Employee| row.name.as_str())
                .filter_by(|row: &Employee| row.name.clone()),
            ColumnSpec::new("department")
                .sort_by_text(|row: &Employee| row.department.as_str())
                .filter_by(|row: &Employee| row.department.clone()),
            ColumnSpec::new("city")
                .sort_by_text(|row: &Employee| row.city.as_str())
                .filter_by(|row: &Employee| row.city.clone()),
            ColumnSpec::new("salary").sort_by_value(|row: &Employee| row.salary),
        ]
    }

    fn local(employees: &[Employee], query: &GridQuery) -> Page<Employee> {
        let state = GridState {
            sort: query.sort.clone(),
            column_filters: query.column_filters.clone(),
            filters: query.filters.clone(),
            search: query.search.clone(),
            page: Some(PageState {
                index: query.page,
                size: query.page_size,
            }),
            ..GridState::new()
        };
        let view = compute_view(employees, &local_columns(), &state);
        let rows = view
            .indices
            .iter()
            .map(|&index| employees[index].clone())
            .collect();
        Page::new(rows, view.filtered_len)
    }

    fn query() -> GridQuery {
        GridQuery {
            page_size: 25,
            ..GridQuery::default()
        }
    }

    #[test]
    fn the_sql_answers_exactly_like_the_grid_would_locally() {
        let (connection, employees) = database();
        let mut cases = vec![query()];

        let mut sorted = query();
        sorted.sort = vec![
            SortState::new("department", SortDirection::Asc),
            SortState::new("salary", SortDirection::Desc),
        ];
        sorted.page = 3;
        cases.push(sorted);

        let mut searched = query();
        searched.search = Some("berLIN".to_owned());
        searched.sort = vec![SortState::new("name", SortDirection::Desc)];
        cases.push(searched);

        let mut filtered = query();
        filtered.column_filters = vec![
            ("department".into(), "eng".to_owned()),
            ("city".into(), "m".to_owned()),
        ];
        filtered.page = 1;
        cases.push(filtered);

        for case in cases {
            let remote = query_employees(&connection, &case).unwrap();
            let expected = local(&employees, &case);
            assert_eq!(remote.total, expected.total, "total for {case:?}");
            assert_eq!(remote.rows, expected.rows, "rows for {case:?}");
        }
    }

    #[test]
    fn unknown_column_ids_never_reach_the_sql() {
        let (connection, employees) = database();
        let mut hostile = query();
        hostile.sort = vec![SortState::new(
            "name; DROP TABLE employees; --",
            SortDirection::Asc,
        )];
        hostile.column_filters = vec![("1=1 OR name".into(), "x".to_owned())];

        let page = query_employees(&connection, &hostile).unwrap();
        // Ignored, like unknown ids in the grid: all rows, in id order.
        assert_eq!(page.total, employees.len());
        assert_eq!(page.rows[0].id, 1);
        // And the table is still there.
        assert!(query_employees(&connection, &query()).is_ok());
    }

    #[test]
    fn wildcards_and_quotes_in_user_text_are_literal() {
        let (connection, _) = database();
        for text in ["%", "_", "' OR '1'='1", "\\"] {
            let mut literal = query();
            literal.search = Some(text.to_owned());
            assert_eq!(
                query_employees(&connection, &literal).unwrap().total,
                0,
                "{text}"
            );
        }
    }

    #[test]
    fn a_page_past_the_end_is_empty_with_the_real_total() {
        let (connection, employees) = database();
        let mut far = query();
        far.page = 10_000;
        let page = query_employees(&connection, &far).unwrap();
        assert!(page.rows.is_empty());
        assert_eq!(page.total, employees.len());
    }

    /// Operands for every operator: numbers as the menu or the bar send them
    /// (text), typed numbers, text of both cases, empties and nonsense.
    fn operands() -> Vec<Value> {
        [
            "50000",
            "71919",
            "40000",
            "99999",
            "abc",
            "",
            " ",
            "berlin",
            "BERLIN",
            "Ber",
            "lin",
            "engineering",
            "a",
            "Ada Bauer",
            "50000.5",
            "5,5",
        ]
        .into_iter()
        .map(Value::from)
        .chain([
            Value::Int(60_000),
            Value::Float(71_919.0),
            Value::Float(45_000.5),
        ])
        .collect()
    }

    /// Phase 8 acceptance: for every operator, on text and on number columns,
    /// with every operand, the SQL filters exactly as the grid does locally.
    #[test]
    fn every_operator_filters_like_the_grid() {
        let (connection, employees) = database();
        let ops = [
            FilterOp::Contains,
            FilterOp::StartsWith,
            FilterOp::EndsWith,
            FilterOp::Equals,
            FilterOp::NotEquals,
            FilterOp::Less,
            FilterOp::LessOrEqual,
            FilterOp::Greater,
            FilterOp::GreaterOrEqual,
            FilterOp::Between,
            FilterOp::IsEmpty,
            FilterOp::IsNotEmpty,
            FilterOp::OneOf,
        ];
        let operands = operands();

        for id in ["city", "name", "salary"] {
            for op in ops {
                for (index, first) in operands.iter().enumerate() {
                    let second = &operands[(index + 3) % operands.len()];
                    let values = match op.operands() {
                        Some(0) => Vec::new(),
                        Some(2) => vec![first.clone(), second.clone()],
                        Some(_) => vec![first.clone()],
                        None => vec![first.clone(), second.clone()],
                    };
                    let mut case = query();
                    case.page_size = 10_000;
                    case.filters = vec![(id.into(), Condition::new(op, values).into())];

                    let remote = query_employees(&connection, &case).unwrap();
                    let expected = local(&employees, &case);
                    assert_eq!(remote.total, expected.total, "{id} {op:?} {first:?}");
                    assert_eq!(remote.rows, expected.rows, "{id} {op:?} {first:?}");
                }
            }
        }
    }

    /// The filter bar's shortcuts, and conditions joined with and and or.
    #[test]
    fn bar_text_and_combined_conditions_filter_like_the_grid() {
        let (connection, employees) = database();
        let mut cases = Vec::new();

        for (id, text) in [
            ("salary", ">90000"),
            ("salary", "<=41000"),
            ("salary", "50000..60000"),
            ("salary", "60000..50000"),
            ("salary", "71919"),
            ("salary", "!=71919"),
            ("salary", "7191"),
            ("city", "=berlin"),
            ("city", "!=Munich"),
            ("city", "ber"),
            ("city", " ber"),
            ("name", ">m"),
            ("name", "Ada..Ben"),
        ] {
            let mut case = query();
            case.page_size = 10_000;
            case.column_filters = vec![(id.into(), text.to_owned())];
            cases.push(case);
        }

        let mut combined = query();
        combined.filters = vec![
            (
                "salary".into(),
                ColumnFilter::new(Condition::less("45000")).or(Condition::greater("95000")),
            ),
            (
                "city".into(),
                ColumnFilter::new(Condition::one_of(["berlin", "Munich"])),
            ),
        ];
        combined.column_filters = vec![("department".into(), "eng".to_owned())];
        combined.sort = vec![SortState::new("salary", SortDirection::Desc)];
        combined.page = 1;
        cases.push(combined);

        for case in cases {
            let remote = query_employees(&connection, &case).unwrap();
            let expected = local(&employees, &case);
            assert_eq!(remote.total, expected.total, "total for {case:?}");
            assert_eq!(remote.rows, expected.rows, "rows for {case:?}");
        }
    }

    /// The value list from SQL matches the grid's, including leaving out the
    /// column's own filter.
    #[test]
    fn value_lists_match_the_grid() {
        let (connection, employees) = database();
        let mut case = query();
        case.column_filters = vec![
            ("salary".into(), ">80000".to_owned()),
            ("city".into(), "=Berlin".to_owned()),
        ];

        for (id, limit) in [("city", 100), ("department", 100), ("salary", 7)] {
            let column_id = ColumnId::from(id);
            let remote = distinct_values(&connection, &column_id, &case, limit).unwrap();
            let state = GridState {
                column_filters: case.column_filters.clone(),
                ..GridState::new()
            };
            let expected = datagrid_core::distinct_values(
                &employees,
                &local_columns(),
                &state,
                &column_id,
                limit,
            )
            .unwrap();
            assert_eq!(remote, expected, "{id}");
        }
    }

    #[test]
    fn hostile_operands_stay_parameters() {
        let (connection, employees) = database();
        let mut hostile = query();
        hostile.filters = vec![(
            "city".into(),
            Condition::one_of(["x') OR 1=1 --", "%", "_"]).into(),
        )];
        assert_eq!(query_employees(&connection, &hostile).unwrap().total, 0);

        // A filter on an unknown column is ignored, like locally.
        hostile.filters = vec![("1=1; --".into(), Condition::equals("x").into())];
        assert_eq!(
            query_employees(&connection, &hostile).unwrap().total,
            employees.len()
        );
    }

    #[test]
    fn an_edit_is_written_unless_the_server_refuses_it() {
        let (connection, employees) = database();
        let mut edited = employees[0].clone();
        edited.salary = 61_000;
        edited.city = "Munich".into();
        update_employee(&connection, &edited).unwrap();

        let mut case = query();
        case.filters = vec![(
            "name".into(),
            Condition::equals(edited.name.as_str()).into(),
        )];
        case.page_size = 10_000;
        let rows = query_employees(&connection, &case).unwrap().rows;
        assert!(rows.contains(&edited));

        let mut greedy = edited.clone();
        greedy.salary = SALARY_BAND + 1;
        assert!(
            update_employee(&connection, &greedy)
                .unwrap_err()
                .contains("salary band")
        );
        let mut nameless = edited.clone();
        nameless.name = " ".into();
        assert!(update_employee(&connection, &nameless).is_err());
        let mut unknown = edited;
        unknown.id = 999_999;
        assert!(update_employee(&connection, &unknown).is_err());
        // Nothing refused was written.
        assert!(
            query_employees(&connection, &case)
                .unwrap()
                .rows
                .iter()
                .any(|row| row.salary == 61_000)
        );
    }

    /// What the grid shows locally for a grouped query: the view of the
    /// query's state, with the aggregates it asks for, as a page.
    fn local_grouped(employees: &[Employee], query: &GridQuery) -> Page<Employee> {
        let mut columns = local_columns();
        Aggregate::apply_query(&mut columns, query);
        Page::from_view(
            &compute_view(employees, &columns, &query.state()),
            employees,
        )
    }

    #[test]
    fn grouped_pages_match_the_grid_page_for_page() {
        let (connection, employees) = database();
        let aggregates: Vec<(ColumnId, AggregateKind)> = vec![
            ("name".into(), AggregateKind::Count),
            ("city".into(), AggregateKind::Min),
            ("city".into(), AggregateKind::Max),
            ("salary".into(), AggregateKind::Sum),
            ("salary".into(), AggregateKind::Average),
            ("salary".into(), AggregateKind::Min),
            ("salary".into(), AggregateKind::Max),
        ];

        let mut by_department = query();
        by_department.group_by = vec!["department".into()];
        by_department.page_size = 7;

        let mut two_levels = by_department.clone();
        two_levels.group_by = vec!["department".into(), "city".into()];
        two_levels.aggregates.clone_from(&aggregates);
        two_levels.sort = vec![
            SortState::new("department", SortDirection::Desc),
            SortState::new("salary", SortDirection::Desc),
        ];

        let mut collapsed = two_levels.clone();
        collapsed.groups_collapsed = true;
        collapsed.toggled_groups = vec![GroupKey(vec![Some(Value::Text("sales".into()))])];

        let mut filtered = by_department.clone();
        filtered.aggregates = aggregates;
        filtered.search = Some("berlin".into());
        filtered.group_by = vec!["city".into(), "salary".into()];
        filtered.column_filters = vec![("salary".into(), ">60000".into())];

        for case in [by_department, two_levels, collapsed, filtered] {
            let first = query_employees(&connection, &case).unwrap();
            let pages = first.row_count.unwrap().div_ceil(case.page_size);
            assert!(pages > 1, "{case:?} spans pages");
            // The first pages, one in the middle, the last, and one past the
            // end, which shows the last.
            for page in [0, 1, 2, pages / 2, pages - 1, pages] {
                let at = GridQuery {
                    page,
                    ..case.clone()
                };
                let remote = query_employees(&connection, &at).unwrap();
                let expected = local_grouped(&employees, &at);
                assert_eq!(remote, expected, "page {page} of {at:?}");
            }
        }
    }

    #[test]
    fn totals_come_with_an_ungrouped_page_too() {
        let (connection, employees) = database();
        let mut case = query();
        case.aggregates = vec![("salary".into(), AggregateKind::Sum)];
        let page = query_employees(&connection, &case).unwrap();
        let sum: i64 = employees.iter().map(|row| i64::from(row.salary)).sum();
        assert_eq!(page.totals[0].value, Some(Value::Int(sum)));
        assert_eq!(page, local_grouped(&employees, &case));
    }
}
