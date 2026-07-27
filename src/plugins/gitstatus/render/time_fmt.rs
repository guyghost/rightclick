//! Time-formatting helpers for the git status plugin.
use crate::ui::count_label;

pub(super) fn format_relative_time(date: &chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let duration = now.signed_duration_since(*date);

    if duration.num_minutes() < 1 {
        "just now".to_string()
    } else if duration.num_minutes() < 60 {
        relative_time_label(duration.num_minutes(), "min", "mins")
    } else if duration.num_hours() < 24 {
        relative_time_label(duration.num_hours(), "hour", "hours")
    } else if duration.num_days() < 7 {
        relative_time_label(duration.num_days(), "day", "days")
    } else {
        date.format("%Y-%m-%d").to_string()
    }
}

fn relative_time_label(count: i64, singular: &str, plural: &str) -> String {
    format!("{} ago", count_label(count as usize, singular, plural))
}

pub(super) fn count_title(
    singular: &str,
    plural: &str,
    count: usize,
    unit_singular: &str,
    unit_plural: &str,
) -> String {
    let title = if count == 1 { singular } else { plural };
    format!(
        " {} ({}) ",
        title,
        count_label(count, unit_singular, unit_plural)
    )
}
