use crate::{
    constants::{PARQUET_SUFFIX, UPDATE_QUANTIZATION},
    error::MduResult,
    quantized_time::{Quantization, QuantizedTime, TopOfHour, TopOfMonth},
    s3::ObjectKey,
    subfolders::SubFolders,
};
use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};
use std::collections::HashSet;

/// A single update window that still needs to be ingested.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Window {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl Window {
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self { start, end }
    }

    pub fn file_path(&self) -> String {
        Planner::window_file_path(self.start)
    }
}

/// A complete set of source files that can be rolled up into one compacted file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompactionTarget {
    pub prefix: ObjectKey,
    pub source_files: Vec<ObjectKey>,
    pub compacted_file: ObjectKey,
}

pub struct Planner;

pub type Plan = Planner;

impl Planner {
    /// Plan the windows that are missing between the earliest known data and `end_time`.
    ///
    /// `backfill_start` can move the starting point earlier than the earliest existing
    /// file. Compacted periods are skipped rather than re-ingesting their source windows.
    pub fn plan_windows(
        existing: &HashSet<String>,
        end_time: DateTime<Utc>,
        backfill_start: Option<DateTime<Utc>>,
    ) -> MduResult<Vec<Window>> {
        let backfill_start = backfill_start
            .map(|start| QuantizedTime::timestamp(start, UPDATE_QUANTIZATION))
            .transpose()?;
        let existing_start = existing
            .iter()
            .filter_map(|path| Self::file_start_time(path))
            .min()
            .map(|start| QuantizedTime::timestamp(start, UPDATE_QUANTIZATION))
            .transpose()?;
        let start = match (backfill_start, existing_start) {
            (Some(backfill_start), Some(existing_start)) => {
                Some(backfill_start.min(existing_start))
            }
            (Some(backfill_start), None) => Some(backfill_start),
            (None, Some(existing_start)) => Some(existing_start),
            (None, None) => None,
        };
        let Some(start) = start.filter(|start| *start < end_time) else {
            return Ok(vec![Window::new(
                QuantizedTime::previous_top(end_time, UPDATE_QUANTIZATION)?,
                end_time,
            )]);
        };
        Self::plan_windows_between(existing, start, end_time)
    }

    /// Plan missing windows in an explicit range, skipping compacted periods.
    pub fn plan_windows_between(
        existing: &HashSet<String>,
        start: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> MduResult<Vec<Window>> {
        let mut start = start;
        let mut windows = Vec::new();
        while start < end_time {
            if let Some(boundary) = Self::compacted_boundary(existing, start)? {
                start = boundary;
                continue;
            }
            let window_end = QuantizedTime::next_top(start, UPDATE_QUANTIZATION)?;
            if window_end > end_time {
                break;
            }
            if !existing.contains(&Self::window_file_path(start)) {
                windows.push(Window::new(start, window_end));
            }
            start = window_end;
        }
        Ok(windows)
    }

    /// Plan compaction for one prefix using the entries directly below it.
    ///
    /// Returns a target only when every expected source file is present.
    pub fn plan_compaction<I, K>(
        prefix: &ObjectKey,
        entries: I,
    ) -> MduResult<Option<CompactionTarget>>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<str>,
    {
        Self::plan_compaction_with_suffix(prefix, entries, PARQUET_SUFFIX)
    }

    /// Plan compaction for one prefix using a non-default file suffix.
    pub fn plan_compaction_with_suffix<I, K>(
        prefix: &ObjectKey,
        entries: I,
        suffix: &str,
    ) -> MduResult<Option<CompactionTarget>>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<str>,
    {
        let source_files = Self::expected_compaction_sources_with_suffix(prefix, suffix)?;
        if source_files.is_empty() {
            return Ok(None);
        }
        let entries = entries
            .into_iter()
            .map(|entry| entry.as_ref().to_string())
            .collect::<HashSet<_>>();
        if source_files
            .iter()
            .any(|source| !entries.contains(source.as_str()))
        {
            return Ok(None);
        }
        Ok(Some(CompactionTarget {
            prefix: prefix.clone(),
            source_files,
            compacted_file: Self::compacted_file_path(prefix, suffix),
        }))
    }

    /// Expected source files for compacting `prefix`, or an empty list for a leaf path.
    pub fn expected_compaction_sources(prefix: &ObjectKey) -> MduResult<Vec<ObjectKey>> {
        Self::expected_compaction_sources_with_suffix(prefix, PARQUET_SUFFIX)
    }

    /// Expected source files for compacting `prefix` with a non-default file suffix.
    pub fn expected_compaction_sources_with_suffix(
        prefix: &ObjectKey,
        suffix: &str,
    ) -> MduResult<Vec<ObjectKey>> {
        let expected_subfolders = SubFolders::subfolders_from_path(prefix.as_str())?;
        let separator = if prefix.as_str().ends_with('/') {
            ""
        } else {
            "/"
        };
        Ok(expected_subfolders
            .iter()
            .map(|subfolder| {
                ObjectKey::new(format!(
                    "{}{}{}{}",
                    prefix,
                    separator,
                    subfolder.strip_suffix('/').unwrap_or(subfolder),
                    suffix
                ))
            })
            .collect())
    }

    pub fn compacted_file_path(prefix: &ObjectKey, suffix: &str) -> ObjectKey {
        ObjectKey::new(format!(
            "{}{}",
            prefix.as_str().trim_end_matches('/'),
            suffix
        ))
    }

    pub fn window_file_path(start_time: DateTime<Utc>) -> String {
        let year = start_time.year();
        let month = start_time.month() as i32;
        let day = start_time.day() as i32;
        let hour = start_time.hour() as i32;
        let minute = start_time.minute() as i32;
        SubFolders::file_path(&[year, month, day, hour, minute], PARQUET_SUFFIX)
    }

    fn compacted_boundary(
        existing: &HashSet<String>,
        start_time: DateTime<Utc>,
    ) -> MduResult<Option<DateTime<Utc>>> {
        let compacted = Self::compacted_file_paths(start_time);
        let next_period_start = if existing.contains(&compacted[3]) {
            Self::next_year(start_time)?
        } else if existing.contains(&compacted[2]) {
            Self::next_month(start_time)?
        } else if existing.contains(&compacted[1]) {
            Self::next_day(start_time)?
        } else if existing.contains(&compacted[0]) {
            Self::next_hour(start_time)?
        } else {
            return Ok(None);
        };
        // The compacted period may end between update windows for coarse
        // quantizations, so resume at the first window at or after its end.
        Ok(Some(QuantizedTime::top_at_or_after(
            next_period_start,
            UPDATE_QUANTIZATION,
        )?))
    }

    fn compacted_file_paths(start_time: DateTime<Utc>) -> [String; 4] {
        let year = start_time.year();
        let month = start_time.month() as i32;
        let day = start_time.day() as i32;
        let hour = start_time.hour() as i32;
        [
            SubFolders::file_path(&[year, month, day, hour], PARQUET_SUFFIX),
            SubFolders::file_path(&[year, month, day], PARQUET_SUFFIX),
            SubFolders::file_path(&[year, month], PARQUET_SUFFIX),
            SubFolders::file_path(&[year], PARQUET_SUFFIX),
        ]
    }

    fn file_start_time(path: &str) -> Option<DateTime<Utc>> {
        let path = path.strip_suffix(PARQUET_SUFFIX)?;
        let mut segments = path.split('/');
        let year = segments.next()?.parse::<i32>().ok()?;
        let month = Self::file_path_segment(segments.next(), 1)?;
        let day = Self::file_path_segment(segments.next(), 1)?;
        let hour = Self::file_path_segment(segments.next(), 0)?;
        let minute = Self::file_path_segment(segments.next(), 0)?;
        if segments.next().is_some() {
            return None;
        }
        Utc.with_ymd_and_hms(year, month, day, hour, minute, 0)
            .single()
    }

    fn file_path_segment(segment: Option<&str>, default: u32) -> Option<u32> {
        match segment {
            Some(segment) => segment.parse().ok(),
            None => Some(default),
        }
    }

    fn next_hour(timestamp: DateTime<Utc>) -> MduResult<DateTime<Utc>> {
        let hour_start = QuantizedTime::timestamp(timestamp, Quantization::Hour(TopOfHour::One))?;
        QuantizedTime::next_top(hour_start, Quantization::Hour(TopOfHour::One))
    }

    fn next_day(timestamp: DateTime<Utc>) -> MduResult<DateTime<Utc>> {
        let day_start = QuantizedTime::timestamp(timestamp, Quantization::Day)?;
        QuantizedTime::next_top(day_start, Quantization::Day)
    }

    fn next_month(timestamp: DateTime<Utc>) -> MduResult<DateTime<Utc>> {
        let month_start =
            QuantizedTime::timestamp(timestamp, Quantization::Month(TopOfMonth::One))?;
        QuantizedTime::next_top(month_start, Quantization::Month(TopOfMonth::One))
    }

    fn next_year(timestamp: DateTime<Utc>) -> MduResult<DateTime<Utc>> {
        let year_start = QuantizedTime::timestamp(timestamp, Quantization::Year)?;
        QuantizedTime::next_top(year_start, Quantization::Year)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, TimeZone, Utc};

    fn utc(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, minute, 0)
            .single()
            .unwrap()
    }

    fn paths(paths: &[&str]) -> HashSet<String> {
        paths.iter().map(|path| (*path).to_string()).collect()
    }

    fn window(start: DateTime<Utc>, end: DateTime<Utc>) -> Window {
        Window::new(start, end)
    }

    #[test]
    fn no_existing_files_plans_only_the_latest_window() {
        let end = utc(2024, 1, 1, 12, 0);
        assert_eq!(
            Planner::plan_windows(&HashSet::new(), end, None).unwrap(),
            vec![window(utc(2024, 1, 1, 11, 54), end)]
        );
    }

    #[test]
    fn backfill_plans_missing_windows_from_the_requested_start() {
        let end = utc(2024, 1, 1, 0, 18);
        assert_eq!(
            Planner::plan_windows(&HashSet::new(), end, Some(utc(2024, 1, 1, 0, 0))).unwrap(),
            vec![
                window(utc(2024, 1, 1, 0, 0), utc(2024, 1, 1, 0, 6)),
                window(utc(2024, 1, 1, 0, 6), utc(2024, 1, 1, 0, 12)),
                window(utc(2024, 1, 1, 0, 12), utc(2024, 1, 1, 0, 18)),
            ]
        );
    }

    #[test]
    fn existing_windows_are_skipped_and_the_earliest_existing_sets_the_start() {
        let existing = paths(&["2024/01/01/00/00.parquet", "2024/01/01/00/12.parquet"]);
        assert_eq!(
            Planner::plan_windows(&existing, utc(2024, 1, 1, 0, 18), None).unwrap(),
            vec![window(utc(2024, 1, 1, 0, 6), utc(2024, 1, 1, 0, 12))]
        );
    }

    #[test]
    fn backfill_can_start_before_the_earliest_existing_window() {
        let existing = paths(&["2024/01/01/00/12.parquet"]);
        assert_eq!(
            Planner::plan_windows(
                &existing,
                utc(2024, 1, 1, 0, 18),
                Some(utc(2024, 1, 1, 0, 0))
            )
            .unwrap(),
            vec![
                window(utc(2024, 1, 1, 0, 0), utc(2024, 1, 1, 0, 6)),
                window(utc(2024, 1, 1, 0, 6), utc(2024, 1, 1, 0, 12)),
            ]
        );
    }

    #[test]
    fn fully_ingested_latest_window_has_no_work() {
        let existing = paths(&["2024/01/01/00/00.parquet"]);
        assert!(
            Planner::plan_windows(&existing, utc(2024, 1, 1, 0, 6), None)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn compacted_hours_are_skipped() {
        let existing = paths(&["2024/01/01/03.parquet"]);
        assert_eq!(
            Planner::plan_windows(
                &existing,
                utc(2024, 1, 1, 4, 12),
                Some(utc(2024, 1, 1, 3, 0))
            )
            .unwrap(),
            vec![
                window(utc(2024, 1, 1, 4, 0), utc(2024, 1, 1, 4, 6)),
                window(utc(2024, 1, 1, 4, 6), utc(2024, 1, 1, 4, 12)),
            ]
        );
    }

    #[test]
    fn compacted_days_are_skipped() {
        let existing = paths(&["2024/01/01.parquet"]);
        assert_eq!(
            Planner::plan_windows(
                &existing,
                utc(2024, 1, 2, 0, 12),
                Some(utc(2024, 1, 1, 12, 0))
            )
            .unwrap(),
            vec![
                window(utc(2024, 1, 2, 0, 0), utc(2024, 1, 2, 0, 6)),
                window(utc(2024, 1, 2, 0, 6), utc(2024, 1, 2, 0, 12)),
            ]
        );
    }

    #[test]
    fn compacted_months_are_skipped() {
        let existing = paths(&["2024/01.parquet"]);
        assert_eq!(
            Planner::plan_windows(
                &existing,
                utc(2024, 2, 1, 0, 12),
                Some(utc(2024, 1, 15, 0, 0))
            )
            .unwrap(),
            vec![
                window(utc(2024, 2, 1, 0, 0), utc(2024, 2, 1, 0, 6)),
                window(utc(2024, 2, 1, 0, 6), utc(2024, 2, 1, 0, 12)),
            ]
        );
    }

    #[test]
    fn compacted_years_are_skipped() {
        let existing = paths(&["2024.parquet"]);
        assert_eq!(
            Planner::plan_windows(
                &existing,
                utc(2025, 1, 1, 0, 12),
                Some(utc(2024, 7, 1, 0, 0))
            )
            .unwrap(),
            vec![
                window(utc(2025, 1, 1, 0, 0), utc(2025, 1, 1, 0, 6)),
                window(utc(2025, 1, 1, 0, 6), utc(2025, 1, 1, 0, 12)),
            ]
        );
    }

    #[test]
    fn complete_hour_produces_a_compaction_target() {
        let prefix = ObjectKey::new("paper/binance/bitcoin/tetherusd/2024/01/01/00/");
        let source_files = (0..60)
            .step_by(6)
            .map(|minute| ObjectKey::new(format!("{prefix}{minute:02}.parquet")))
            .collect::<Vec<_>>();
        let target = Planner::plan_compaction(&prefix, &source_files)
            .unwrap()
            .unwrap();
        assert_eq!(
            target,
            CompactionTarget {
                prefix: prefix.clone(),
                source_files,
                compacted_file: ObjectKey::new(
                    "paper/binance/bitcoin/tetherusd/2024/01/01/00.parquet"
                ),
            }
        );
    }

    #[test]
    fn complete_day_produces_a_compaction_target() {
        let prefix = ObjectKey::new("paper/binance/bitcoin/tetherusd/2024/01/01/");
        let source_files = (0..24)
            .map(|hour| ObjectKey::new(format!("{prefix}{hour:02}.parquet")))
            .collect::<Vec<_>>();
        let target = Planner::plan_compaction(&prefix, &source_files)
            .unwrap()
            .unwrap();
        assert_eq!(
            target.compacted_file,
            ObjectKey::new("paper/binance/bitcoin/tetherusd/2024/01/01.parquet")
        );
        assert_eq!(target.source_files, source_files);
    }

    #[test]
    fn missing_source_file_does_not_produce_a_target() {
        let prefix = ObjectKey::new("paper/binance/bitcoin/tetherusd/2024/01/01/00/");
        let mut source_files = (0..60)
            .step_by(6)
            .map(|minute| ObjectKey::new(format!("{prefix}{minute:02}.parquet")))
            .collect::<Vec<_>>();
        source_files.pop();
        assert_eq!(
            Planner::plan_compaction(&prefix, &source_files).unwrap(),
            None
        );
    }

    #[test]
    fn leaf_path_does_not_produce_a_target() {
        let prefix = ObjectKey::new("paper/binance/bitcoin/tetherusd/2024/01/01/00/00/");
        assert_eq!(
            Planner::plan_compaction(&prefix, [] as [ObjectKey; 0]).unwrap(),
            None
        );
    }
}
