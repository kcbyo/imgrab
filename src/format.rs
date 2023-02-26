use std::fmt::Display;

use chrono::Duration;

pub trait DurationFormat {
    fn into_formatter(self) -> DurationFormatter;
}

impl DurationFormat for Duration {
    fn into_formatter(self) -> DurationFormatter {
        DurationFormatter(self)
    }
}

pub struct DurationFormatter(Duration);

impl Display for DurationFormatter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let elapsed = self.0;
        write!(
            f,
            "{}+{:02}:{:02}",
            elapsed.num_hours(),
            elapsed.num_minutes() % 60,
            elapsed.num_seconds() % 60
        )
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration;

    #[test]
    fn duration_format() {
        let duration = Duration::hours(3) + Duration::minutes(3) + Duration::seconds(13);
        let duration = super::DurationFormatter(duration).to_string();
        assert_eq!("3+03:13", duration);
    }
}
