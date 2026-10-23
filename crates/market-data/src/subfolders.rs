use crate::{constants::UPDATE_QUANTIZATION, quantized_time::Quantization};
use buildplane_errors::{BpError, BpResult, DataError};
use chrono::{Datelike, NaiveDate};

pub struct SubFolders;

impl SubFolders {
    /// Expected child entries for compaction, using the configured update quantization.
    pub fn subfolders_from_path(path: &str) -> BpResult<Vec<String>> {
        Self::subfolders_from_path_with_quantization(path, UPDATE_QUANTIZATION)
    }

    /// Expected child entries for compaction at `path` for a given quantization.
    ///
    /// The update path layout stores each window under the start of that window, so
    /// the expected children are the window starts at the next path level.
    pub fn subfolders_from_path_with_quantization(
        path: &str,
        quantization: Quantization,
    ) -> BpResult<Vec<String>> {
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        match segments.len() {
            0..=4 => Ok(Vec::new()),
            5 => {
                Self::parse_year(segments[4], path)?;
                Ok(Self::subfolders(&Self::months_in_year(quantization)))
            }
            6 => {
                let year = Self::parse_year(segments[4], path)?;
                let month = Self::parse_month(segments[5], path)?;
                Ok(Self::subfolders(&Self::days_in_month_for_quantization(
                    quantization,
                    year,
                    month,
                )?))
            }
            7 => {
                let year = Self::parse_year(segments[4], path)?;
                let month = Self::parse_month(segments[5], path)?;
                let days_in_month = Self::days_in_month(year, month)?;
                Self::parse_day(segments[6], days_in_month, path)?;
                Ok(Self::subfolders(&Self::hours_in_day(quantization)))
            }
            8 => {
                let year = Self::parse_year(segments[4], path)?;
                let month = Self::parse_month(segments[5], path)?;
                let days_in_month = Self::days_in_month(year, month)?;
                Self::parse_day(segments[6], days_in_month, path)?;
                Self::parse_hour(segments[7], path)?;
                Ok(Self::subfolders(&Self::minutes_in_hour(quantization)))
            }
            9 => Ok(Vec::new()),
            _ => Err(BpError::Data(DataError::TooManyPathSegments {
                path: path.to_string(),
            })),
        }
    }

    pub fn subfolders(numbers: &[i32]) -> Vec<String> {
        numbers.iter().map(|n| format!("{:02}/", n)).collect()
    }

    pub fn file_path(numbers: &[i32], suffix: &str) -> String {
        let raw_file_path = numbers
            .iter()
            .map(|n| format!("{:02}", n))
            .collect::<Vec<_>>()
            .join("/");
        format!("{}{}", raw_file_path, suffix)
    }

    fn months_in_year(quantization: Quantization) -> Vec<i32> {
        match quantization {
            Quantization::Minutes(_)
            | Quantization::Hour(_)
            | Quantization::Day
            | Quantization::Week(_) => (1..=12).collect(),
            Quantization::Month(top_of_month) => (1..=12).step_by(top_of_month as usize).collect(),
            Quantization::Year => vec![1],
        }
    }

    fn days_in_month_for_quantization(
        quantization: Quantization,
        year: i32,
        month: u32,
    ) -> BpResult<Vec<i32>> {
        let days_in_month = Self::days_in_month(year, month)?;
        match quantization {
            Quantization::Minutes(_) | Quantization::Hour(_) | Quantization::Day => {
                Ok((1..=days_in_month as i32).collect())
            }
            Quantization::Week(weekday) => Ok((1..=days_in_month)
                .filter(|day| {
                    NaiveDate::from_ymd_opt(year, month, *day)
                        .is_some_and(|date| date.weekday() == weekday)
                })
                .map(|day| day as i32)
                .collect()),
            Quantization::Month(top_of_month) => {
                Ok(if (month - 1).is_multiple_of(top_of_month as u32) {
                    vec![1]
                } else {
                    Vec::new()
                })
            }
            Quantization::Year => Ok(if month == 1 { vec![1] } else { Vec::new() }),
        }
    }

    fn hours_in_day(quantization: Quantization) -> Vec<i32> {
        match quantization {
            Quantization::Minutes(_) => (0..24).collect(),
            Quantization::Hour(top_of_hour) => (0..24).step_by(top_of_hour as usize).collect(),
            Quantization::Day
            | Quantization::Week(_)
            | Quantization::Month(_)
            | Quantization::Year => vec![0],
        }
    }

    fn minutes_in_hour(quantization: Quantization) -> Vec<i32> {
        match quantization {
            Quantization::Minutes(top_of_minute) => {
                (0..60).step_by(top_of_minute as usize).collect()
            }
            Quantization::Hour(_)
            | Quantization::Day
            | Quantization::Week(_)
            | Quantization::Month(_)
            | Quantization::Year => vec![0],
        }
    }

    fn parse_year(segment: &str, path: &str) -> BpResult<i32> {
        segment.parse::<i32>().map_err(|_| {
            BpError::Data(DataError::InvalidYearMonth {
                path: path.to_string(),
            })
        })
    }

    fn parse_number(segment: &str, path: &str) -> BpResult<u32> {
        segment.parse::<u32>().map_err(|_| {
            BpError::Data(DataError::InvalidYearMonth {
                path: path.to_string(),
            })
        })
    }

    fn parse_month(segment: &str, path: &str) -> BpResult<u32> {
        let month = Self::parse_number(segment, path)?;
        if (1..=12).contains(&month) {
            Ok(month)
        } else {
            Err(BpError::Data(DataError::InvalidMonth { month }))
        }
    }

    fn parse_day(segment: &str, days_in_month: u32, path: &str) -> BpResult<u32> {
        let day = Self::parse_number(segment, path)?;
        if (1..=days_in_month).contains(&day) {
            Ok(day)
        } else {
            Err(BpError::Data(DataError::InvalidDay { day }))
        }
    }

    fn parse_hour(segment: &str, path: &str) -> BpResult<u32> {
        let hour = Self::parse_number(segment, path)?;
        if hour < 24 {
            Ok(hour)
        } else {
            Err(BpError::Data(DataError::InvalidHour { hour }))
        }
    }

    fn days_in_month(year: i32, month: u32) -> BpResult<u32> {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => Ok(31),
            4 | 6 | 9 | 11 => Ok(30),
            2 => {
                if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                    Ok(29)
                } else {
                    Ok(28)
                }
            }
            _ => Err(BpError::Data(DataError::InvalidMonth { month })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quantized_time::{TopOfHour, TopOfMinute, TopOfMonth};
    use chrono::Weekday;

    const PAIR: &str = "paper/binance/bitcoin/tetherusd";

    #[test]
    fn minutes_follow_quantization() {
        let path = format!("{PAIR}/2024/01/01/00/");
        assert_eq!(
            SubFolders::subfolders_from_path_with_quantization(
                &path,
                Quantization::Minutes(TopOfMinute::Five)
            )
            .unwrap(),
            SubFolders::subfolders(&[0, 5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55])
        );
        assert_eq!(
            SubFolders::subfolders_from_path_with_quantization(
                &path,
                Quantization::Hour(TopOfHour::Six)
            )
            .unwrap(),
            SubFolders::subfolders(&[0])
        );
    }

    #[test]
    fn hours_follow_quantization() {
        let path = format!("{PAIR}/2024/01/01/");
        assert_eq!(
            SubFolders::subfolders_from_path_with_quantization(
                &path,
                Quantization::Minutes(TopOfMinute::Six)
            )
            .unwrap(),
            SubFolders::subfolders(&(0..24).collect::<Vec<_>>())
        );
        assert_eq!(
            SubFolders::subfolders_from_path_with_quantization(
                &path,
                Quantization::Hour(TopOfHour::Six)
            )
            .unwrap(),
            SubFolders::subfolders(&[0, 6, 12, 18])
        );
        assert_eq!(
            SubFolders::subfolders_from_path_with_quantization(&path, Quantization::Day).unwrap(),
            SubFolders::subfolders(&[0])
        );
    }

    #[test]
    fn days_follow_quantization() {
        let february = format!("{PAIR}/2024/02/");
        assert_eq!(
            SubFolders::subfolders_from_path_with_quantization(
                &february,
                Quantization::Minutes(TopOfMinute::Six)
            )
            .unwrap(),
            SubFolders::subfolders(&(1..=29).collect::<Vec<_>>())
        );
        assert_eq!(
            SubFolders::subfolders_from_path_with_quantization(
                &february,
                Quantization::Week(Weekday::Mon)
            )
            .unwrap(),
            SubFolders::subfolders(&[5, 12, 19, 26])
        );
        assert_eq!(
            SubFolders::subfolders_from_path_with_quantization(
                &february,
                Quantization::Month(TopOfMonth::Six)
            )
            .unwrap(),
            Vec::<String>::new()
        );

        let january = format!("{PAIR}/2024/01/");
        assert_eq!(
            SubFolders::subfolders_from_path_with_quantization(
                &january,
                Quantization::Month(TopOfMonth::Six)
            )
            .unwrap(),
            SubFolders::subfolders(&[1])
        );
    }

    #[test]
    fn months_follow_quantization() {
        let path = format!("{PAIR}/2024/");
        assert_eq!(
            SubFolders::subfolders_from_path_with_quantization(
                &path,
                Quantization::Minutes(TopOfMinute::Six)
            )
            .unwrap(),
            SubFolders::subfolders(&(1..=12).collect::<Vec<_>>())
        );
        assert_eq!(
            SubFolders::subfolders_from_path_with_quantization(
                &path,
                Quantization::Month(TopOfMonth::Four)
            )
            .unwrap(),
            SubFolders::subfolders(&[1, 5, 9])
        );
        assert_eq!(
            SubFolders::subfolders_from_path_with_quantization(&path, Quantization::Year).unwrap(),
            SubFolders::subfolders(&[1])
        );
    }
}
