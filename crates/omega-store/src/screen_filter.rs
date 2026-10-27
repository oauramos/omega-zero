//! How a game's picture is drawn, as Settings > Screen Filter picks.

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum ScreenFilter {
    /// The picture as it always was: a Game Boy or a GBA game through the LCD mask wherever
    /// the canvas holds it, everything else sharp.
    #[default]
    Lcd,
    /// Sharp everywhere: each game pixel kept square, with no mask over it.
    Sharp,
}

impl ScreenFilter {
    pub const ALL: [ScreenFilter; 2] = [ScreenFilter::Lcd, ScreenFilter::Sharp];

    pub fn key(self) -> &'static str {
        match self {
            ScreenFilter::Lcd => "lcd",
            ScreenFilter::Sharp => "sharp",
        }
    }

    pub fn from_key(key: &str) -> Option<ScreenFilter> {
        Self::ALL.into_iter().find(|f| f.key() == key)
    }
}
