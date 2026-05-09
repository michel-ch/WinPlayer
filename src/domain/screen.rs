#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Screen {
    AllSongs,
    AlbumsList,
    AlbumDetail(String),
    ArtistsList,
    ArtistDetail(String),
    Folders,
    Queue,
    NowPlaying,
    Equalizer,
    Settings,
}

impl Screen {
    pub fn shows_chrome(&self) -> bool {
        !matches!(self, Screen::NowPlaying | Screen::Settings | Screen::Equalizer)
    }
}
