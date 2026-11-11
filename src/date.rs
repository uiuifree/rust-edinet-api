use crate::EdinetError;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

/// Calendar date, written `YYYY-MM-DD`.
///
/// # Example
///
/// ```
/// use edinet_api::Date;
///
/// let date: Date = "2026-06-30".parse()?;
/// assert_eq!(date, Date::new(2026, 6, 30)?);
/// assert_eq!(date.to_string(), "2026-06-30");
/// assert!("2026-02-30".parse::<Date>().is_err());
/// # Ok::<(), edinet_api::EdinetError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    year: u16,
    month: u8,
    day: u8,
}

impl Date {
    /// Checked constructor. Years must be between 1 and 9999.
    pub fn new(year: u16, month: u8, day: u8) -> Result<Self, EdinetError> {
        if !(1..=9999).contains(&year) || !(1..=12).contains(&month) {
            return Err(invalid_date(year, month, day));
        }
        let leap =
            year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
        let days = match month {
            2 if leap => 29,
            2 => 28,
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        };
        if !(1..=days).contains(&day) {
            return Err(invalid_date(year, month, day));
        }
        Ok(Date { year, month, day })
    }

    /// Year.
    pub fn year(&self) -> u16 {
        self.year
    }

    /// Month, 1 to 12.
    pub fn month(&self) -> u8 {
        self.month
    }

    /// Day of the month, from 1.
    pub fn day(&self) -> u8 {
        self.day
    }
}

fn invalid_date(year: u16, month: u8, day: u8) -> EdinetError {
    EdinetError::InvalidArgument(format!("invalid date: {year:04}-{month:02}-{day:02}"))
}

/// Parses a fixed-width number made only of ASCII digits.
fn digits<T: FromStr>(s: &str) -> Option<T> {
    if s.bytes().all(|b| b.is_ascii_digit()) {
        s.parse().ok()
    } else {
        None
    }
}

impl FromStr for Date {
    type Err = EdinetError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || EdinetError::InvalidArgument(format!("expected YYYY-MM-DD: {s:?}"));
        let b = s.as_bytes();
        if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
            return Err(invalid());
        }
        let year = digits(&s[0..4]).ok_or_else(invalid)?;
        let month = digits(&s[5..7]).ok_or_else(invalid)?;
        let day = digits(&s[8..10]).ok_or_else(invalid)?;
        Date::new(year, month, day)
    }
}

impl Display for Date {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// Date and time in Japan Standard Time, written `YYYY-MM-DD hh:mm`.
///
/// # Example
///
/// ```
/// use edinet_api::DateTime;
///
/// let submitted: DateTime = "2026-06-30 15:00".parse()?;
/// assert_eq!(submitted.date().to_string(), "2026-06-30");
/// assert_eq!((submitted.hour(), submitted.minute()), (15, 0));
/// # Ok::<(), edinet_api::EdinetError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DateTime {
    date: Date,
    hour: u8,
    minute: u8,
}

impl DateTime {
    /// Checked constructor.
    pub fn new(date: Date, hour: u8, minute: u8) -> Result<Self, EdinetError> {
        if hour > 23 || minute > 59 {
            return Err(EdinetError::InvalidArgument(format!(
                "invalid time: {hour:02}:{minute:02}"
            )));
        }
        Ok(DateTime { date, hour, minute })
    }

    /// Date part.
    pub fn date(&self) -> Date {
        self.date
    }

    /// Hour, 0 to 23.
    pub fn hour(&self) -> u8 {
        self.hour
    }

    /// Minute, 0 to 59.
    pub fn minute(&self) -> u8 {
        self.minute
    }
}

impl FromStr for DateTime {
    type Err = EdinetError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || EdinetError::InvalidArgument(format!("expected YYYY-MM-DD hh:mm: {s:?}"));
        let b = s.as_bytes();
        if b.len() != 16 || b[10] != b' ' || b[13] != b':' {
            return Err(invalid());
        }
        let date = s[0..10].parse()?;
        let hour = digits(&s[11..13]).ok_or_else(invalid)?;
        let minute = digits(&s[14..16]).ok_or_else(invalid)?;
        DateTime::new(date, hour, minute)
    }
}

impl Display for DateTime {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {:02}:{:02}", self.date, self.hour, self.minute)
    }
}

macro_rules! serde_as_string {
    ($($name:ident),*) => {$(
        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let s = String::deserialize(deserializer)?;
                s.parse().map_err(D::Error::custom)
            }
        }
    )*};
}

serde_as_string!(Date, DateTime);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_calendar() {
        assert!(Date::new(2024, 2, 29).is_ok());
        assert!(Date::new(2000, 2, 29).is_ok());
        assert!(Date::new(1900, 2, 29).is_err());
        assert!(Date::new(2023, 2, 29).is_err());
        assert!(Date::new(2023, 2, 28).is_ok());
        assert!(Date::new(2023, 4, 31).is_err());
        assert!(Date::new(2023, 12, 31).is_ok());
        assert!(Date::new(2023, 13, 1).is_err());
        assert!(Date::new(2023, 1, 0).is_err());
        assert!(Date::new(0, 1, 1).is_err());
        let date = Date::new(2023, 4, 3).unwrap();
        assert_eq!((date.year(), date.month(), date.day()), (2023, 4, 3));
    }

    #[test]
    fn parses_strictly() {
        assert_eq!(
            "2023-04-03".parse::<Date>().unwrap(),
            Date::new(2023, 4, 3).unwrap()
        );
        for bad in [
            "2023-4-3",
            "2023/04/03",
            "2023-04-0x",
            "2023-0x-03",
            "+023-04-03",
            "2023-04-03 ",
        ] {
            assert!(bad.parse::<Date>().is_err(), "{bad}");
        }
        let error = "2023-02-30".parse::<Date>().unwrap_err();
        assert_eq!(
            error.to_string(),
            "invalid argument: invalid date: 2023-02-30"
        );
    }

    #[test]
    fn parses_date_times() {
        let value: DateTime = "2023-04-03 09:05".parse().unwrap();
        assert_eq!(value.to_string(), "2023-04-03 09:05");
        assert_eq!(value.date(), Date::new(2023, 4, 3).unwrap());
        assert_eq!((value.hour(), value.minute()), (9, 5));
        assert!("2023-04-03 24:00".parse::<DateTime>().is_err());
        assert!("2023-04-03 23:60".parse::<DateTime>().is_err());
        assert!("2023-04-03T09:05".parse::<DateTime>().is_err());
        assert!("2023-04-03 09-05".parse::<DateTime>().is_err());
        assert!("2023-04-03 0x:05".parse::<DateTime>().is_err());
        assert!("2023-04-03 09:x5".parse::<DateTime>().is_err());
        assert!("2023-04-32 09:05".parse::<DateTime>().is_err());
        assert!("2023-04-03 09:05:00".parse::<DateTime>().is_err());
    }

    #[test]
    fn round_trips_through_serde() {
        let value: DateTime = serde_json::from_str(r#""2023-04-03 13:01""#).unwrap();
        assert_eq!(
            serde_json::to_string(&value).unwrap(),
            r#""2023-04-03 13:01""#
        );
        let date: Date = serde_json::from_str(r#""2023-04-03""#).unwrap();
        assert_eq!(serde_json::to_string(&date).unwrap(), r#""2023-04-03""#);
        assert!(serde_json::from_str::<Date>(r#""2023-04""#).is_err());
        assert!(serde_json::from_str::<Date>("20230403").is_err());
    }
}
