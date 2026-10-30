use chrono::{DateTime, Utc};
use std::{error::Error, fmt};

/// Functional Threshold Power used only as a display reference for zones.
/// It never changes trainer resistance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ftp(i32);

pub const DEFAULT_FTP_W: i32 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidFtp;
impl fmt::Display for InvalidFtp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("FTP must be an integer between 1 and 2000 W")
    }
}
impl Error for InvalidFtp {}

impl Ftp {
    pub fn new(watts: i64) -> Result<Self, InvalidFtp> {
        i32::try_from(watts)
            .ok()
            .filter(|w| (1..=2000).contains(w))
            .map(Self)
            .ok_or(InvalidFtp)
    }
    pub fn default_reference() -> Self {
        Self(DEFAULT_FTP_W)
    }
    pub fn watts(self) -> i32 {
        self.0
    }

    /// Z0 is a real zero reading; negative power is not assigned a zone.
    /// Boundaries in % of FTP: (0,60) [60,76) [76,90) [90,105) [105,119) [119,inf).
    pub fn zone(self, watts: i32) -> Option<usize> {
        if watts < 0 {
            return None;
        }
        if watts == 0 {
            return Some(0);
        }
        // Integer comparison avoids floating-point drift exactly at boundaries.
        let percent_x = i64::from(watts) * 100;
        let ftp = i64::from(self.0);
        Some(
            [60, 76, 90, 105, 119]
                .iter()
                .position(|&limit| percent_x < limit * ftp)
                .map_or(6, |index| index + 1),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AthleteSettings {
    pub ftp: Ftp,
    pub ftp_is_default: bool,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ftp_accepts_only_whole_watts_in_range() {
        assert!(Ftp::new(0).is_err());
        assert!(Ftp::new(2001).is_err());
        assert!(Ftp::new(i64::MAX).is_err());
        assert_eq!(Ftp::new(1).unwrap().watts(), 1);
        assert_eq!(Ftp::new(2000).unwrap().watts(), 2000);
    }

    #[test]
    fn zone_boundaries_match_the_display_contract() {
        let ftp = Ftp::default_reference();
        for (watts, zone) in [
            (0, 0),
            (1, 1),
            (119, 1),
            (120, 2),
            (151, 2),
            (152, 3),
            (179, 3),
            (180, 4),
            (209, 4),
            (210, 5),
            (237, 5),
            (238, 6),
        ] {
            assert_eq!(ftp.zone(watts), Some(zone), "{watts} W");
        }
        assert_eq!(ftp.zone(-1), None);
    }
}
