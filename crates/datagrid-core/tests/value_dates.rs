#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

//! `Value::date_parts` and `Value::iso_date`: what a writer reads a date through, so
//! that it does not need `chrono` of its own.

use datagrid_core::{DateParts, Value};

#[test]
fn a_value_that_is_not_a_date_has_no_parts_and_no_iso_text() {
    for value in [
        Value::Text("2026-10-16".into()),
        Value::Int(20261016),
        Value::Float(1.5),
        Value::Bool(true),
    ] {
        assert_eq!(value.date_parts(), None);
        assert_eq!(value.iso_date(), None);
    }
}

#[cfg(feature = "chrono")]
mod with_chrono {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn a_date_has_its_parts_and_no_time() {
        let value = Value::Date(NaiveDate::from_ymd_opt(2026, 10, 16).unwrap());
        assert_eq!(
            value.date_parts(),
            Some(DateParts {
                year: 2026,
                month: 10,
                day: 16,
                time: None,
            })
        );
        assert_eq!(value.iso_date().as_deref(), Some("2026-10-16"));
    }

    #[test]
    fn a_date_and_time_has_its_time_too() {
        let at = NaiveDate::from_ymd_opt(2026, 1, 2)
            .unwrap()
            .and_hms_opt(18, 5, 9)
            .unwrap();
        let value = Value::DateTime(at);
        assert_eq!(
            value.date_parts(),
            Some(DateParts {
                year: 2026,
                month: 1,
                day: 2,
                time: Some((18, 5, 9)),
            })
        );
        assert_eq!(value.iso_date().as_deref(), Some("2026-01-02 18:05:09"));
    }
}
