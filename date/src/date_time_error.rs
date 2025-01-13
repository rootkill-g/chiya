use std::{error::Error, fmt::Display, io, time::SystemTime};

use crate::date_time::DateTime;

#[derive(Debug)]
pub struct DateTimeError(pub ());

impl Error for DateTimeError {}

impl Display for DateTimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Error occured while parsing date. (Unavailable/Invalid)")
    }
}

impl From<DateTimeError> for io::Error {
    fn from(derr: DateTimeError) -> Self {
        io::Error::new(io::ErrorKind::Other, derr)
    }
}

/// Parse a date form an HTTP Header field
pub fn parse_httph_date(httph_date: &str) -> Result<SystemTime, DateTimeError> {
    httph_date.parse::<DateTime>().map(|d| d.into())
}

/// Format a date to be used in HTTP header field
pub fn fmt_httph_date(sys_t: SystemTime) -> String {
    format!("{}", DateTime::from(sys_t))
}
