#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RewardOverlayEntry {
    pub name: String,
    pub platinum: Option<u32>,
    pub ducats: Option<u32>,
    pub volume: Option<u32>,
    pub vaulted: bool,
    pub mastered: bool,
    pub owned_count: Option<u32>,
    pub required_count: Option<u32>,
    pub highlight: RewardHighlight,
}

impl RewardOverlayEntry {
    pub fn name_only(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            platinum: None,
            ducats: None,
            volume: None,
            vaulted: false,
            mastered: false,
            owned_count: None,
            required_count: None,
            highlight: RewardHighlight::None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RewardHighlight {
    #[default]
    None,
    BestPlatinum,
    BestDucats,
    Needed,
}
