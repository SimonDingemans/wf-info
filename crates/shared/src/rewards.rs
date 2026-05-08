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

    pub fn set_platinum(&mut self, platinum: u32) {
        self.platinum = Some(platinum);
    }

    pub fn set_ducats(&mut self, ducats: u32) {
        self.ducats = Some(ducats);
    }

    pub fn set_volume(&mut self, volume: u32) {
        self.volume = Some(volume);
    }

    pub fn set_vaulted(&mut self, vaulted: bool) {
        self.vaulted = vaulted;
    }

    pub fn with_platinum(mut self, platinum: u32) -> Self {
        self.set_platinum(platinum);
        self
    }

    pub fn with_ducats(mut self, ducats: u32) -> Self {
        self.set_ducats(ducats);
        self
    }

    pub fn with_volume(mut self, volume: u32) -> Self {
        self.set_volume(volume);
        self
    }

    pub fn with_vaulted(mut self, vaulted: bool) -> Self {
        self.set_vaulted(vaulted);
        self
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
