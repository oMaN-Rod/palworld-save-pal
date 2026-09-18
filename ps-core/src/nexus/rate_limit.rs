use serde::Serialize;

const HOURLY_LIMIT: &str = "x-rl-hourly-limit";
const HOURLY_REMAINING: &str = "x-rl-hourly-remaining";
const HOURLY_RESET: &str = "x-rl-hourly-reset";
const DAILY_LIMIT: &str = "x-rl-daily-limit";
const DAILY_REMAINING: &str = "x-rl-daily-remaining";
const DAILY_RESET: &str = "x-rl-daily-reset";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RateLimit {
    pub hourly_limit: Option<u32>,
    pub hourly_remaining: Option<u32>,
    pub hourly_reset: Option<String>,
    pub daily_limit: Option<u32>,
    pub daily_remaining: Option<u32>,
    pub daily_reset: Option<String>,
}

impl RateLimit {
    pub fn from_headers<'a, F>(header: F) -> Option<Self>
    where
        F: Fn(&str) -> Option<&'a str>,
    {
        let number = |name: &str| header(name).and_then(|value| value.trim().parse::<u32>().ok());
        let text = |name: &str| {
            header(name)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        };
        let limit = Self {
            hourly_limit: number(HOURLY_LIMIT),
            hourly_remaining: number(HOURLY_REMAINING),
            hourly_reset: text(HOURLY_RESET),
            daily_limit: number(DAILY_LIMIT),
            daily_remaining: number(DAILY_REMAINING),
            daily_reset: text(DAILY_RESET),
        };
        (limit != Self::default()).then_some(limit)
    }

    pub fn reset_after_exhaustion(&self) -> Option<&str> {
        if self.daily_remaining == Some(0) {
            return self.daily_reset.as_deref();
        }
        self.hourly_reset.as_deref().or(self.daily_reset.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn headers_become_a_rate_limit() {
        let headers: HashMap<&str, &str> = HashMap::from([
            ("x-rl-hourly-limit", "500"),
            ("x-rl-hourly-remaining", "0"),
            ("x-rl-hourly-reset", "2026-09-16T21:00:00+00:00"),
            ("x-rl-daily-limit", "20000"),
            ("x-rl-daily-remaining", " 12 "),
            ("x-rl-daily-reset", "2026-09-17T00:00:00+00:00"),
        ]);
        let limit = RateLimit::from_headers(|name| headers.get(name).copied()).unwrap();
        assert_eq!(limit.hourly_remaining, Some(0));
        assert_eq!(limit.daily_remaining, Some(12));
        assert_eq!(
            limit.reset_after_exhaustion(),
            Some("2026-09-16T21:00:00+00:00")
        );
    }

    #[test]
    fn an_exhausted_day_reports_the_daily_reset_and_no_headers_is_none() {
        let limit = RateLimit {
            daily_remaining: Some(0),
            daily_reset: Some("tomorrow".to_string()),
            hourly_reset: Some("soon".to_string()),
            ..Default::default()
        };
        assert_eq!(limit.reset_after_exhaustion(), Some("tomorrow"));
        assert!(RateLimit::from_headers(|_| None).is_none());
        let daily_only = RateLimit {
            daily_reset: Some("d".to_string()),
            ..Default::default()
        };
        assert_eq!(daily_only.reset_after_exhaustion(), Some("d"));
    }
}
