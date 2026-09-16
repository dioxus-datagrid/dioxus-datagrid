//! Benchmarks for [`compute_view`].
//!
//! The acceptance criterion from `PLAN.md` phase 1 is the
//! `sort_two_columns/100000` case: sorting 100,000 rows by two columns must
//! stay under 50 ms in a release build.
//!
//! Run with `cargo bench -p datagrid-core`.

// `criterion_group!` expands to an undocumented public function, which the
// workspace's `missing_docs` lint would otherwise reject.
#![allow(missing_docs, clippy::unwrap_used, clippy::indexing_slicing)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use datagrid_core::{ColumnSpec, GridState, SortState, compute_view};
use std::hint::black_box;

#[derive(Clone)]
struct Row {
    name: String,
    department: String,
    age: u32,
}

/// Deterministic pseudo-random rows, so runs are comparable.
///
/// Uses a small xorshift rather than pulling in a random number generator: the
/// data only has to be varied and reproducible, not statistically sound.
fn rows(count: usize) -> Vec<Row> {
    let departments = [
        "engineering",
        "sales",
        "support",
        "finance",
        "legal",
        "research",
    ];

    let mut seed = 0x2545_F491_4F6C_DD1D_u64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };

    (0..count)
        .map(|index| {
            let value = next();
            Row {
                // Mixed case on purpose: the default collation folds case, which
                // is the expensive path.
                name: if value % 2 == 0 {
                    format!("Person {:06}", value % 1_000_000)
                } else {
                    format!("person {:06}", value % 1_000_000)
                },
                // Few distinct values, so the primary sort produces long runs of
                // ties that the secondary column has to break.
                department: departments[index % departments.len()].to_owned(),
                age: (value % 60) as u32 + 18,
            }
        })
        .collect()
}

fn columns() -> Vec<ColumnSpec<Row>> {
    vec![
        ColumnSpec::new("department")
            .sort_by_text(|row: &Row| row.department.as_str())
            .filter_by(|row: &Row| row.department.clone()),
        ColumnSpec::new("name")
            .sort_by_text(|row: &Row| row.name.as_str())
            .filter_by(|row: &Row| row.name.clone()),
        ColumnSpec::new("age").sort_by_value(|row: &Row| row.age),
    ]
}

fn bench_sort(c: &mut Criterion) {
    let mut group = c.benchmark_group("sort_two_columns");

    for count in [1_000_usize, 10_000, 100_000] {
        let rows = rows(count);
        let columns = columns();
        let state = GridState {
            sort: vec![SortState::asc("department"), SortState::asc("name")],
            ..GridState::new()
        };

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| compute_view(black_box(&rows), black_box(&columns), black_box(&state)));
        });
    }

    group.finish();
}

/// Isolates what the two-string case above is actually paying for.
///
/// `numeric` avoids both the `String` clone in the key closure and the text
/// comparison; `mixed` pays for one string instead of two. Together they show
/// how much of the headline number is string handling rather than sorting.
fn bench_sort_shapes(c: &mut Criterion) {
    let mut group = c.benchmark_group("sort_shapes");
    let rows = rows(100_000);
    let columns = columns();
    group.throughput(Throughput::Elements(100_000));

    let numeric = GridState {
        sort: vec![SortState::asc("age"), SortState::desc("age")],
        ..GridState::new()
    };
    let mixed = GridState {
        sort: vec![SortState::asc("age"), SortState::asc("name")],
        ..GridState::new()
    };
    let single_string = GridState {
        sort: vec![SortState::asc("name")],
        ..GridState::new()
    };

    group.bench_function("numeric/100000", |b| {
        b.iter(|| compute_view(black_box(&rows), black_box(&columns), black_box(&numeric)));
    });
    group.bench_function("mixed/100000", |b| {
        b.iter(|| compute_view(black_box(&rows), black_box(&columns), black_box(&mixed)));
    });
    group.bench_function("single_string/100000", |b| {
        b.iter(|| {
            compute_view(
                black_box(&rows),
                black_box(&columns),
                black_box(&single_string),
            )
        });
    });

    group.finish();
}

fn bench_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter");
    let rows = rows(100_000);
    let columns = columns();

    let mut column_filter = GridState::new();
    column_filter.set_filter("department", "eng");

    let mut search = GridState::new();
    search.set_search("0042");

    group.throughput(Throughput::Elements(100_000));
    group.bench_function("column_filter/100000", |b| {
        b.iter(|| {
            compute_view(
                black_box(&rows),
                black_box(&columns),
                black_box(&column_filter),
            )
        });
    });
    group.bench_function("search/100000", |b| {
        b.iter(|| compute_view(black_box(&rows), black_box(&columns), black_box(&search)));
    });

    group.finish();
}

fn bench_unsorted(c: &mut Criterion) {
    let rows = rows(100_000);
    let columns = columns();
    let state = GridState::new();

    // The floor: no sort, no filter. Everything else is measured against this.
    c.bench_function("passthrough/100000", |b| {
        b.iter(|| compute_view(black_box(&rows), black_box(&columns), black_box(&state)));
    });
}

criterion_group!(
    benches,
    bench_sort,
    bench_sort_shapes,
    bench_filter,
    bench_unsorted
);
criterion_main!(benches);
