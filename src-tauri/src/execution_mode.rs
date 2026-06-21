use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionMode {
    Normal,
    Yolo,
    Unsafe,
}

impl ExecutionMode {
    pub fn runtime_flags(self) -> (bool, bool) {
        match self {
            Self::Normal => (false, false),
            Self::Yolo => (true, false),
            Self::Unsafe => (false, true),
        }
    }

    pub fn from_runtime_flags(yolo_mode: bool, unsafe_mode: bool) -> Self {
        if unsafe_mode {
            Self::Unsafe
        } else if yolo_mode {
            Self::Yolo
        } else {
            Self::Normal
        }
    }

    pub fn from_db(value: &str) -> Self {
        value.parse().unwrap_or(Self::Normal)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Yolo => "yolo",
            Self::Unsafe => "unsafe",
        }
    }

    pub fn include_hard_approvals(self) -> Option<bool> {
        match self {
            Self::Normal => None,
            Self::Yolo => Some(false),
            Self::Unsafe => Some(true),
        }
    }
}

impl std::str::FromStr for ExecutionMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "normal" => Ok(Self::Normal),
            "yolo" => Ok(Self::Yolo),
            "unsafe" => Ok(Self::Unsafe),
            _ => Err(format!("Unknown execution mode: {}", value)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ExecutionMode;

    #[test]
    fn from_runtime_flags_prefers_unsafe_over_yolo() {
        assert_eq!(
            ExecutionMode::from_runtime_flags(true, true),
            ExecutionMode::Unsafe
        );
    }

    #[test]
    fn from_db_falls_back_to_normal_for_unknown_values() {
        assert_eq!(ExecutionMode::from_db("invalid"), ExecutionMode::Normal);
    }

    #[test]
    fn as_str_round_trips_through_from_str() {
        for mode in [
            ExecutionMode::Normal,
            ExecutionMode::Yolo,
            ExecutionMode::Unsafe,
        ] {
            assert_eq!(mode.as_str().parse::<ExecutionMode>().unwrap(), mode);
        }
    }
}
