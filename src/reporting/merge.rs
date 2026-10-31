//! Time maths: merged minutes, date ranges, totals and weekday averages.

use super::*;

#[derive(Debug, Serialize)]
pub struct WeekdayAverage {
    pub weekday: &'static str,
    pub average_hours: f64,
}

/// The seven weekdays' report labels in report order, which is the order
/// `Weekday::num_days_from_monday` indexes -- so the label table and the
/// slot a session lands in are the same list, and can't drift apart.
const WEEKDAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

/// For each weekday Monday..Sunday (always all seven, zero-filled when
/// there's no data), the average hours logged on that weekday: total
/// minutes across `sessions` that fall on that weekday, divided by the
/// number of distinct calendar dates in `sessions` that fall on it. A
/// session is attributed to its `start` date's weekday. Callers filter
/// `sessions` to one task (or one project) beforehand.
pub fn weekday_averages(sessions: &[Session], now: NaiveDateTime) -> Vec<WeekdayAverage> {
    // Seven fixed slots rather than two maps keyed by weekday: the
    // zero-fill an absent weekday needs is what an array gives for free.
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

// ---- the period a report covers ---------------------------------------

/// The days an `info` report covers. Either a single day (`--date`,
/// defaulting to today) or an interval (`--from`/`--to`) -- the CLI lets
/// you write one formulation or the other, never both. `from: None` is an
/// open start: every session up to and including `to`.
/// Serialized as the header's own date fields: `date:` for a single day,
/// `from:`/`to:` for an interval -- the same split [`Self::single_day`]
/// makes everywhere else, so the range is stored once and rendered from,
/// rather than stored again as three strings.
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

    /// The one date this range covers, when it covers exactly one -- what
    /// decides between a `date:` and a `from:`/`to:` header, and whether
    /// session tables need a `Date` column at all.
    pub fn single_day(&self) -> Option<NaiveDate> {
        self.from.filter(|from| *from == self.to)
    }

    /// How the period reads in a text report's summary line.
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

/// The date format every report prints and `main::parse_day` reads back.
pub fn fmt_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

pub fn round_to_half_hour(hours: f64) -> f64 {
    (hours * 2.0).round() / 2.0
}

/// An amount of time spent, in the two shapes every report prints it in.
/// They are one number twice over, so they are derived together, once,
/// rather than re-paired at each of the places a total is built.
///
/// `#[serde(flatten)]`ed wherever it appears, so the two keys sit in the
/// YAML exactly where the two fields used to.
#[derive(Debug, Clone, Serialize)]
pub struct Total {
    pub total_hours: f64,
    pub total_hhmm: String,
    /// The exact figure the two above are rounded from -- for the web
    /// report's bars; not part of the printed report.
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

    /// How a total reads in a report body, e.g. `03:30 (3.5 h)`.
    pub(super) fn spent(&self) -> String {
        format!("{} ({:.1} h)", self.total_hhmm, self.total_hours)
    }
}

/// Total minutes covered by `sessions`, merging any two chronologically
/// adjacent sessions (by start time) whose gap is under `gap_minutes` into
/// one continuous span. This is what makes a project-level report correct
/// even when two of its tasks were worked on in parallel: feed it the union
/// of every task's sessions for the day and overlapping/near spans collapse
/// into one, instead of being double-counted.
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
