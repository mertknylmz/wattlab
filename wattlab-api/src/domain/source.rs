#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Ftms,
    CyclingPower,
}
impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ftms => "ftms",
            Self::CyclingPower => "cycling_power",
        }
    }
}
