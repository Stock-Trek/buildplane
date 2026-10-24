use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Tier {
    Raw,
    Hourly,
    Daily,
    Monthly,
    Yearly,
}

impl Tier {
    pub const ALL: [Self; 5] = [
        Self::Raw,
        Self::Hourly,
        Self::Daily,
        Self::Monthly,
        Self::Yearly,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Hourly => "hourly",
            Self::Daily => "daily",
            Self::Monthly => "monthly",
            Self::Yearly => "yearly",
        }
    }

    pub const fn source_depth(self) -> usize {
        match self {
            Self::Raw => 4,
            Self::Hourly => 3,
            Self::Daily => 2,
            Self::Monthly => 1,
            Self::Yearly => 0,
        }
    }

    pub const fn target(self) -> Option<Self> {
        match self {
            Self::Raw => Some(Self::Hourly),
            Self::Hourly => Some(Self::Daily),
            Self::Daily => Some(Self::Monthly),
            Self::Monthly => Some(Self::Yearly),
            Self::Yearly => None,
        }
    }

    pub const fn source(self) -> Option<Self> {
        match self {
            Self::Raw => None,
            Self::Hourly => Some(Self::Raw),
            Self::Daily => Some(Self::Hourly),
            Self::Monthly => Some(Self::Daily),
            Self::Yearly => Some(Self::Monthly),
        }
    }

    pub fn from_segment(segment: &str) -> Option<Self> {
        match segment {
            "raw" => Some(Self::Raw),
            "hourly" => Some(Self::Hourly),
            "daily" => Some(Self::Daily),
            "monthly" => Some(Self::Monthly),
            "yearly" => Some(Self::Yearly),
            _ => None,
        }
    }

    pub fn from_path(path: &str) -> Option<Self> {
        path_segments(path)
            .get(4)
            .and_then(|segment| Self::from_segment(segment))
    }

    pub fn time_segment_count(path: &str) -> Option<usize> {
        let segments = path_segments(path);
        Self::from_segment(segments.get(4)?)?;
        Some(segments.len() - 5)
    }
}

impl fmt::Display for Tier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

fn path_segments(path: &str) -> Vec<&str> {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}
