//! The server side: a SQLite table and the translation of a `GridQuery` into SQL.
//!
//! Compiled only with the `server` feature. The client build never sees
//! `rusqlite` or this module.

use crate::Employee;
use datagrid_core::{GridQuery, Page, SortDirection};
use rusqlite::{Connection, params_from_iter, types::Value};
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
    /// Text columns sort case-insensitively, as the grid does by default.
    text: bool,
    /// Whether the global search scans this column.
    searchable: bool,
}

const COLUMNS: [SqlColumn; 4] = [
    SqlColumn {
        id: "name",
        expr: "name",
        text: true,
        searchable: true,
    },
    SqlColumn {
        id: "department",
        expr: "department",
        text: true,
        searchable: true,
    },
    SqlColumn {
        id: "city",
        expr: "city",
        text: true,
        searchable: true,
    },
    SqlColumn {
        id: "salary",
        expr: "salary",
        text: false,
        searchable: false,
    },
];

fn column(id: &str) -> Option<&'static SqlColumn> {
    COLUMNS.iter().find(|column| column.id == id)
}

/// A `LIKE` pattern matching `text` anywhere, with its wildcards taken literally.
fn contains_pattern(text: &str) -> Value {
    let escaped = text
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    Value::Text(format!("%{escaped}%"))
}

/// `WHERE`, `ORDER BY` and the bound parameters for `query`.
///
/// User text only ever travels as a bound parameter. SQLite's `LIKE` is
/// case-insensitive for ASCII, matching the grid's local filtering.
struct Sql {
    where_clause: String,
    order_by: String,
    params: Vec<Value>,
}

fn translate(query: &GridQuery) -> Sql {
    let mut conditions = Vec::new();
    let mut params = Vec::new();

    // Column filters: every one must match.
    for (id, text) in &query.column_filters {
        if let Some(column) = column(id.as_str()) {
            conditions.push(format!("{} LIKE ? ESCAPE '\\'", column.expr));
            params.push(contains_pattern(text));
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
            let collation = if column.text { " COLLATE NOCASE" } else { "" };
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

/// Runs `query` against `connection`: one page of rows and the total match count.
pub fn query_employees(
    connection: &Connection,
    query: &GridQuery,
) -> rusqlite::Result<Page<Employee>> {
    let sql = translate(query);

    let total: i64 = connection.query_row(
        &format!("SELECT COUNT(*) FROM employees {}", sql.where_clause),
        params_from_iter(sql.params.iter()),
        |row| row.get(0),
    )?;

    let mut page_params = sql.params;
    page_params.push(Value::Integer(
        i64::try_from(query.page_size).unwrap_or(i64::MAX),
    ));
    page_params.push(Value::Integer(
        i64::try_from(query.offset()).unwrap_or(i64::MAX),
    ));

    let mut statement = connection.prepare(&format!(
        "SELECT id, name, department, city, salary FROM employees {} {} LIMIT ? OFFSET ?",
        sql.where_clause, sql.order_by
    ))?;
    let rows = statement
        .query_map(params_from_iter(page_params.iter()), |row| {
            Ok(Employee {
                id: row.get(0)?,
                name: row.get(1)?,
                department: row.get(2)?,
                city: row.get(3)?,
                salary: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(Page::new(rows, usize::try_from(total).unwrap_or(0)))
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
    use datagrid_core::{ColumnSpec, GridState, PageState, SortState, compute_view};

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
}
