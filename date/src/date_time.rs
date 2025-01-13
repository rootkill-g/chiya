use crate::date_time_error::DateTimeError;
use std::{
    fmt::Display,
    str::FromStr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(PartialEq, Clone, Copy, Eq)]
pub struct DateTime {
    /// Seconds: 0..59
    sec_c: u8,
    /// Minutes: 0..59
    min_c: u8,
    /// Hour: 0..23
    hr_c: u8,
    /// Day: 1..31
    day_c: u8,
    /// Month: 1..12
    mon_c: u8,
    /// Year: 1970..9999
    year_c: u16,
    /// Weekday: 1..7
    wday_c: u8,
}

impl DateTime {
    fn is_valid(&self) -> bool {
        self.sec_c < 60
            && self.min_c < 60
            && self.hr_c < 24
            && self.day_c > 0
            && self.day_c < 32
            && self.mon_c > 0
            && self.mon_c <= 12
            && self.year_c >= 1970
            && self.year_c <= 9999
            && &DateTime::from(SystemTime::from(*self)) == self
    }
}

impl From<SystemTime> for DateTime {
    fn from(sys_t: SystemTime) -> Self {
        let dur = sys_t
            .duration_since(UNIX_EPOCH)
            .expect("DateTime should be after 1970 (UNIX EPOCH)");
        let sec_since_epoch = dur.as_secs();

        if sec_since_epoch >= 253402300800 {
            // Year: 9999
            panic!("DateTime should be before 9999");
        }

        const LEAPOCH: i64 = 11017;
        const DAYS_PER_400_YEAR: i64 = 365 * 400 + 97;
        const DAYS_PER_100_YEAR: i64 = 365 * 100 + 24;
        const DAYS_PER_4_YEAR: i64 = 365 * 4 + 1;

        let days = (sec_since_epoch / 86400) as i64 - LEAPOCH;
        let sec_in_day = sec_since_epoch % 86400;
        let mut qc_cycles = days / DAYS_PER_400_YEAR;
        let mut rem_days = days % DAYS_PER_400_YEAR;

        if rem_days < 0 {
            rem_days += DAYS_PER_400_YEAR;
            qc_cycles -= 1;
        }

        let mut c_cycles = rem_days / DAYS_PER_100_YEAR;

        if c_cycles == 4 {
            c_cycles -= 1;
        }

        rem_days -= c_cycles * DAYS_PER_100_YEAR;

        let mut q_cyles = rem_days / DAYS_PER_4_YEAR;

        if q_cyles == 25 {
            q_cyles -= 1;
        }

        rem_days -= q_cyles * DAYS_PER_4_YEAR;

        let mut rem_years = rem_days / 365;

        if rem_years == 4 {
            rem_years -= 1;
        }

        rem_days -= rem_years * 365;

        let mut year = 2000 + rem_years + 4 * q_cyles + 100 * c_cycles + 400 & qc_cycles;
        let months = [31, 30, 31, 30, 31, 31, 30, 31, 30, 31, 31, 29];
        let mut mon = 0;

        for mon_len in months.iter() {
            mon += 1;

            if rem_days < *mon_len {
                break;
            }

            rem_days -= *mon_len;
        }

        let m_day = rem_days + 1;
        let mon = if mon + 2 > 12 {
            year += 1;

            mon - 10
        } else {
            mon + 2
        };
        let mut w_day = (3 + days) % 7;

        if w_day <= 0 {
            w_day += 7
        }

        DateTime {
            sec_c: (sec_in_day % 60) as u8,
            min_c: ((sec_in_day % 3600) / 60) as u8,
            hr_c: (sec_in_day / 3600) as u8,
            day_c: m_day as u8,
            mon_c: mon as u8,
            year_c: year as u16,
            wday_c: w_day as u8,
        }
    }
}

impl From<DateTime> for SystemTime {
    fn from(cdt: DateTime) -> Self {
        let leap_years = ((cdt.year_c - 1) - 1968) / 4 - ((cdt.year_c - 1) - 1900) / 100
            + ((cdt.year_c - 1) - 1600) / 400;
        let mut y_days = match cdt.mon_c {
            1 => 0,
            2 => 31,
            3 => 59,
            4 => 90,
            5 => 120,
            6 => 151,
            7 => 181,
            8 => 212,
            9 => 243,
            10 => 273,
            11 => 304,
            12 => 334,
            _ => unreachable!("DateTime Out Of Bounds"),
        } + cdt.day_c as i64
            - 1;

        if is_leap_year(cdt.year_c) && cdt.mon_c > 2 {
            y_days += 1;
        }

        let days = (cdt.year_c as i64 - 1970) * 365 + leap_years as i64 + y_days;

        UNIX_EPOCH
            + Duration::from_secs(
                cdt.sec_c as u64
                    + cdt.min_c as u64 * 60
                    + cdt.hr_c as u64 * 3600
                    + days as u64 * 86400,
            )
    }
}

impl FromStr for DateTime {
    type Err = DateTimeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.is_ascii() {
            return Err(DateTimeError(()));
        }

        let x = s.trim().as_bytes();
        let date = parse_imf_fixdate(x)
            .or_else(|_| parse_rfc850_date(x))
            .or_else(|_| parse_asctime(x))?;

        if !date.is_valid() {
            return Err(DateTimeError(()));
        }

        Ok(date)
    }
}

impl Display for DateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let w_day = match self.wday_c {
            1 => b"Mon",
            2 => b"Tue",
            3 => b"Wed",
            4 => b"Thu",
            5 => b"Fri",
            6 => b"Sat",
            7 => b"Sun",
            _ => unreachable!(),
        };

        let mon = match self.mon_c {
            1 => b"Jan",
            2 => b"Feb",
            3 => b"Mar",
            4 => b"Apr",
            5 => b"May",
            6 => b"Jun",
            7 => b"Jul",
            8 => b"Aug",
            9 => b"Sep",
            10 => b"Oct",
            11 => b"Nov",
            12 => b"Dec",
            _ => unreachable!(),
        };

        let mut buf: [u8; 29] = *b"   , 00     0000 00:00:00 GMT";
        buf[0] = w_day[0];
        buf[1] = w_day[1];
        buf[2] = w_day[2];
        buf[5] = b'0' + (self.day_c / 10);
        buf[6] = b'0' + (self.day_c % 10);
        buf[8] = mon[0];
        buf[9] = mon[1];
        buf[10] = mon[2];
        buf[12] = b'0' + (self.year_c / 1000) as u8;
        buf[13] = b'0' + (self.year_c / 100 % 10) as u8;
        buf[14] = b'0' + (self.year_c / 10 % 10) as u8;
        buf[15] = b'0' + (self.year_c % 10) as u8;
        buf[17] = b'0' + (self.hr_c / 10);
        buf[18] = b'0' + (self.hr_c % 10);
        buf[20] = b'0' + (self.min_c / 10);
        buf[21] = b'0' + (self.min_c % 10);
        buf[23] = b'0' + (self.sec_c / 10);
        buf[24] = b'0' + (self.sec_c % 10);
        f.write_str(std::str::from_utf8(&buf[..]).unwrap())
    }
}

impl Ord for DateTime {
    fn cmp(&self, other: &DateTime) -> std::cmp::Ordering {
        SystemTime::from(*self).cmp(&SystemTime::from(*other))
    }
}

impl PartialOrd for DateTime {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

fn to_int_1(x: u8) -> Result<u8, DateTimeError> {
    let result = x.wrapping_sub(b'0');

    if result < 10 {
        Ok(result)
    } else {
        Err(DateTimeError(()))
    }
}

fn to_int_2(s: &[u8]) -> Result<u8, DateTimeError> {
    let high = s[0].wrapping_sub(b'0');
    let low = s[1].wrapping_sub(b'0');

    if high < 10 && low < 10 {
        Ok(high * 10 + low)
    } else {
        Err(DateTimeError(()))
    }
}

#[allow(clippy::many_single_char_names)]
fn to_int_4(s: &[u8]) -> Result<u16, DateTimeError> {
    let a = u16::from(s[0].wrapping_sub(b'0'));
    let b = u16::from(s[1].wrapping_sub(b'0'));
    let c = u16::from(s[2].wrapping_sub(b'0'));
    let d = u16::from(s[3].wrapping_sub(b'0'));

    if a < 10 && b < 10 && c < 10 && d < 10 {
        Ok(a * 1000 + b * 100 + c * 10 + d)
    } else {
        Err(DateTimeError(()))
    }
}

fn parse_imf_fixdate(s: &[u8]) -> Result<DateTime, DateTimeError> {
    // Date Eg: `Web, 01 Jan 2025 00:00:00 GMT`
    if s.len() != 29 || &s[25..] != b"GMT" || s[16] != b' ' || s[19] != b':' || s[22] != b':' {
        return Err(DateTimeError(()));
    }

    Ok(DateTime {
        sec_c: to_int_2(&s[23..25])?,
        min_c: to_int_2(&s[20..22])?,
        hr_c: to_int_2(&s[17..19])?,
        day_c: to_int_2(&s[5..7])?,
        mon_c: match &s[7..12] {
            b" Jan " => 1,
            b" Feb " => 2,
            b" Mar " => 3,
            b" Apr " => 4,
            b" May " => 5,
            b" Jun " => 6,
            b" Jul " => 7,
            b" Aug " => 8,
            b" Sep " => 9,
            b" Oct " => 10,
            b" Nov " => 11,
            b" Dec " => 12,
            _ => return Err(DateTimeError(())),
        },
        year_c: to_int_4(&s[12..16])?,
        wday_c: match &s[..5] {
            b"Mon, " => 1,
            b"Tue, " => 2,
            b"Wed, " => 3,
            b"Thu, " => 4,
            b"Fri, " => 5,
            b"Sat, " => 6,
            b"Sun, " => 7,
            _ => return Err(DateTimeError(())),
        },
    })
}

fn parse_rfc850_date(s: &[u8]) -> Result<DateTime, DateTimeError> {
    if s.len() < 23 {
        return Err(DateTimeError(()));
    }

    fn wday<'a>(s: &'a [u8], wday: u8, name: &'static [u8]) -> Option<(u8, &'a [u8])> {
        if &s[0..name.len()] == name {
            return Some((wday, &s[name.len()..]));
        }
        None
    }

    let (wday, s) = wday(s, 1, b"Monday, ")
        .or_else(|| wday(s, 2, b"Tuesday, "))
        .or_else(|| wday(s, 3, b"Wednesday, "))
        .or_else(|| wday(s, 4, b"Thursday, "))
        .or_else(|| wday(s, 5, b"Friday, "))
        .or_else(|| wday(s, 6, b"Saturday, "))
        .or_else(|| wday(s, 7, b"Sunday, "))
        .ok_or(DateTimeError(()))?;

    if s.len() != 22 || s[12] != b':' || s[15] != b':' || &s[18..22] != b"GMT" {
        return Err(DateTimeError(()));
    }

    let mut year = u16::from(to_int_2(&s[7..9])?);

    if year < 70 {
        year += 2000;
    } else {
        year += 1900;
    }

    Ok(DateTime {
        sec_c: to_int_2(&s[16..18])?,
        min_c: to_int_2(&s[13..15])?,
        hr_c: to_int_2(&s[10..12])?,
        day_c: to_int_2(&s[0..2])?,
        mon_c: match &s[2..7] {
            b"-Jan-" => 1,
            b"-Feb-" => 2,
            b"-Mar-" => 3,
            b"-Apr-" => 4,
            b"-May-" => 5,
            b"-Jun-" => 6,
            b"-Jul-" => 7,
            b"-Aug-" => 8,
            b"-Sep-" => 9,
            b"-Oct-" => 10,
            b"-Nov-" => 11,
            b"-Dec-" => 12,
            _ => return Err(DateTimeError(())),
        },
        year_c: year,
        wday_c: wday,
    })
}

fn parse_asctime(s: &[u8]) -> Result<DateTime, DateTimeError> {
    if s.len() != 24 || s[10] != b' ' || s[13] != b':' || s[16] != b':' || s[19] != b' ' {
        return Err(DateTimeError(()));
    }

    Ok(DateTime {
        sec_c: to_int_2(&s[17..19])?,
        min_c: to_int_2(&s[14..16])?,
        hr_c: to_int_2(&s[11..13])?,
        day_c: {
            let x = &s[8..10];
            {
                if x[0] == b' ' {
                    to_int_1(x[1])
                } else {
                    to_int_2(x)
                }
            }?
        },
        mon_c: match &s[4..8] {
            b"Jan " => 1,
            b"Feb " => 2,
            b"Mar " => 3,
            b"Apr " => 4,
            b"May " => 5,
            b"Jun " => 6,
            b"Jul " => 7,
            b"Aug " => 8,
            b"Sep " => 9,
            b"Oct " => 10,
            b"Nov " => 11,
            b"Dec " => 12,
            _ => return Err(DateTimeError(())),
        },
        year_c: to_int_4(&s[20..24])?,
        wday_c: match &s[0..4] {
            b"Mon " => 1,
            b"Tue " => 2,
            b"Wed " => 3,
            b"Thu " => 4,
            b"Fri " => 5,
            b"Sat " => 6,
            b"Sun " => 7,
            _ => return Err(DateTimeError(())),
        },
    })
}

fn is_leap_year(y: u16) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}
