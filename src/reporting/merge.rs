//! Time maths: merged minutes, date ranges, totals and weekday averages.

use super::*;

#[derive(Debug, Serialize)]
pub struct WeekdayAverage {
    pub weekday: &'static str,
    pub average_hours: f64,
}

/// The weekday labels in report order, which is `Weekday::num_days_from_monday` order.
const WEEKDAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

/// Average hours per weekday Monday..Sunday (all seven, zero-filled): minutes of `sessions` on it over the distinct dates falling on it, by `start` date. Callers filter to one task or project first.
pub fn weekday_averages(sessions: &[Session], now: NaiveDateTime) -> Vec<WeekdayAverage> {
    // seven slots, so absent weekdays zero-fill for free
    let mut by_weekday: [(i64, HashSet<NaiveDate>); 7] = Default::default();

    for session in sessions {
        let date = session.start.date();
        let (minutes, days) = &mut by_weekday[date.weekday().num_days_from_monday() as usize];
        *minutes += session.duration_minutes(now);
        days.insert(date);
    }

    WEEKDAYS
        .into_iter()
        .zip(by_weekday)
        .map(|(weekday, (total_minutes, days))| WeekdayAverage {
            weekday,
            average_hours: match days.len() {
                0 => 0.0,
                distinct_days => (total_minutes as f64 / 60.0) / distinct_days as f64,
            },
        })
        .collect()
}


/// The days an `info` report covers: a single day (`--date`, default today) or an interval (`--from`/`--to`), never both; `from: None` is an open start.
/// Serialized as `date:` for a single day, `from:`/`to:` for an interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateRange {
    pub from: Option<NaiveDate>,
    pub to: NaiveDate,
}

impl DateRange {
    pub fn day(date: NaiveDate) -> Self {
        DateRange {
            from: Some(date),
            to: date,
        }
    }

    /// The one date this range covers, if exactly one: picks a `date:` header and whether session tables need a `Date` column.
    pub fn single_day(&self) -> Option<NaiveDate> {
        self.from.filter(|from| *from == self.to)
    }

    pub fn label(&self) -> String {
        match (self.single_day(), self.from) {
            (Some(day), _) => fmt_date(day),
            (None, Some(from)) => format!("{} to {}", fmt_date(from), fmt_date(self.to)),
            (None, None) => format!("up to {}", fmt_date(self.to)),
        }
    }
}

impl Serialize for DateRange {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        match self.single_day() {
            Some(day) => map.serialize_entry("date", &fmt_date(day))?,
            None => {
                if let Some(from) = self.from {
                    map.serialize_entry("from", &fmt_date(from))?;
                }
                map.serialize_entry("to", &fmt_date(self.to))?;
            }
        }
        map.end()
    }
}

/// The date format every report prints and `utils::report::parse_day` reads back.
pub fn fmt_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

pub fn round_to_half_hour(hours: f64) -> f64 {
    (hours * 2.0).round() / 2.0
}

/// An amount of time in the two shapes every report prints, derived together; `#[serde(flatten)]`ed into the YAML.
#[derive(Debug, Clone, Serialize)]
pub struct Total {
    pub total_hours: f64,
    pub total_hhmm: String,
    /// The exact figure the others round from, for the web report's bars; not printed.
    #[serde(skip)]
    #[cfg_attr(not(feature = "web"), allow(dead_code))]
    pub minutes: i64,
}

impl Total {
    pub fn new(total_minutes: i64) -> Self {
        Total {
            total_hours: round_to_half_hour(total_minutes as f64 / 60.0),
            total_hhmm: Duration(total_minutes).to_string(),
            minutes: total_minutes,
        }
    }

    pub(super) fn spent(&self) -> String {
        format!("{} ({:.1} h)", self.total_hhmm, self.total_hours)
    }
}

/// Total minutes of `sessions`, merging chronologically adjacent ones whose gap is under `gap_minutes`;
/// fed the union of a project's tasks' sessions it does not double-count parallel work.
pub fn merged_total_minutes(sessions: &[Session], now: NaiveDateTime, gap_minutes: i64) -> i64 {
    let mut spans: Vec<(NaiveDateTime, NaiveDateTime)> = sessions
        .iter()
        .map(|r| (r.start, r.end.unwrap_or(now)))
        .collect();
    spans.sort_by_key(|&(start, _)| start);

    let mut total = 0i64;
    let mut current: Option<(NaiveDateTime, NaiveDateTime)> = None;

    for (start, end) in spans {
        current = match current {
            None => Some((start, end)),
            Some((cur_start, cur_end)) if (start - cur_end).num_minutes() < gap_minutes => {
                Some((cur_start, end.max(cur_end)))
            }
            Some((cur_start, cur_end)) => {
                total += (cur_end - cur_start).num_minutes();
                Some((start, end))
            }
        };
    }
    if let Some((start, end)) = current {
        total += (end - start).num_minutes();
    }

    total.max(0)
}
