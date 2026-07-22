use std::str::FromStr;

use freya::animation::*;
use freya::icons::lucide::{audio_lines, play};
use freya::prelude::*;
use freya::radio::use_radio;
use freya::router::RouterContext;
use ytdroid::models::YTItem;

use super::TextInfo;
use crate::app::{Data, DataChannel, NavCommand, Route};
use crate::audio::{AudioCommand, AudioQuality};

#[derive(Clone, PartialEq)]
pub struct SongInfo {
    id: String,
    title: String,
    artist: String,
    album: String,
    left: TextInfo,
    details: Option<TextInfo>,
    is_album: bool,
    is_artist: bool,
    is_video: bool,
    is_playlist: bool,
    thumbnail: String,
}

impl SongInfo {
    fn render_info(&self, on_click: impl Fn(String) + 'static + Clone) -> impl IntoElement {
        if self.is_artist {
            if let Some(details) = &self.details {
                return details.clone().into_element();
            }
            return rect().into_element();
        }

        let mut all_elements = self.left.get_inline_elements(on_click.clone());

        if let Some(details) = &self.details {
            all_elements.extend(details.get_inline_elements(on_click));
        }

        paragraph()
            .max_lines(2)
            .text_overflow(TextOverflow::Ellipsis)
            .text_align(TextAlign::Left)
            .spans_iter(all_elements.into_iter())
            .into_element()
    }
}

impl<'a> From<&'a YTItem> for SongInfo {
    fn from(value: &'a YTItem) -> Self {
        match value {
            YTItem::Album(v) => {
                let left = if !v.artists.is_empty() {
                    TextInfo::authors(v.artists.clone())
                } else {
                    TextInfo::plain("Album", None)
                };
                Self {
                    id: v.id.clone(),
                    title: v.title.clone(),
                    artist: v.artists.first().map(|a| a.name.clone()).unwrap_or_default(),
                    album: String::new(),
                    left,
                    details: v.year.as_ref().map(|y| TextInfo::plain(y.clone(), None)),
                    is_album: true,
                    is_artist: false,
                    is_video: false,
                    is_playlist: false,
                    thumbnail: v.thumbnail.clone().unwrap_or_default(),
                }
            }
            YTItem::Playlist(v) => {
                let left = v
                    .author
                    .as_ref()
                    .map(|a| TextInfo::plain(a.clone(), None))
                    .unwrap_or_else(TextInfo::none);
                let details = v
                    .song_count_text
                    .as_ref()
                    .map(|c| TextInfo::plain(c.clone(), None));
                Self {
                    id: v.id.clone(),
                    title: v.title.clone(),
                    artist: v.author.clone().unwrap_or_default(),
                    album: String::new(),
                    left,
                    details,
                    is_album: false,
                    is_artist: false,
                    is_video: false,
                    is_playlist: true,
                    thumbnail: v.thumbnail.clone().unwrap_or_default(),
                }
            }
            YTItem::Artist(v) => Self {
                id: v.id.clone(),
                title: v.title.clone(),
                artist: String::new(),
                album: String::new(),
                left: TextInfo::none(),
                details: v.subscribers.as_ref().map(|s| {
                    TextInfo::plain(format!("{s} de suscriptores"), Some(TextAlign::Center))
                }),
                is_album: false,
                is_artist: true,
                is_video: false,
                is_playlist: false,
                thumbnail: v.thumbnail.clone().unwrap_or_default(),
            },
            YTItem::Song(v) => {
                let left = if !v.artists.is_empty() {
                    TextInfo::authors(v.artists.clone())
                } else {
                    TextInfo::none()
                };
                let details = v.album.as_ref().map(|album| {
                    if !album.id.is_empty() {
                        TextInfo::clickable(album.id.clone(), album.name.clone())
                    } else {
                        TextInfo::plain(album.name.clone(), None)
                    }
                });
                Self {
                    id: v.id.clone(),
                    title: v.title.clone(),
                    artist: v.artists.first().map(|a| a.name.clone()).unwrap_or_default(),
                    album: v.album.as_ref().map(|a| a.name.clone()).unwrap_or_default(),
                    left,
                    details,
                    is_album: false,
                    is_artist: false,
                    is_video: v.is_video_song(),
                    is_playlist: false,
                    thumbnail: v.thumbnail.clone().unwrap_or_default(),
                }
            }
            YTItem::Podcast(v) => Self {
                id: v.id.clone(),
                title: v.title.clone(),
                artist: v.author.clone().unwrap_or_default(),
                album: String::new(),
                left: v
                    .author
                    .as_ref()
                    .map(|a| TextInfo::plain(a.clone(), None))
                    .unwrap_or_else(TextInfo::none),
                details: None,
                is_album: true,
                is_artist: false,
                is_video: false,
                is_playlist: false,
                thumbnail: v.thumbnail.clone().unwrap_or_default(),
            },
            YTItem::Episode(v) => Self {
                id: v.id.clone(),
                title: v.title.clone(),
                artist: v.podcast.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
                album: String::new(),
                left: v
                    .podcast
                    .as_ref()
                    .map(|p| TextInfo::plain(p.name.clone(), None))
                    .unwrap_or_else(TextInfo::none),
                details: v.date.as_ref().map(|d| TextInfo::plain(d.clone(), None)),
                is_album: false,
                is_artist: false,
                is_video: false,
                is_playlist: false,
                thumbnail: v.thumbnail.clone().unwrap_or_default(),
            },
        }
    }
}

impl Component for SongInfo {
    fn render(&self) -> impl IntoElement {
        let mut is_playing = use_state(|| false);
        let mut hover = use_state(|| false);
        let audio_radio = use_radio::<Data, DataChannel>(DataChannel::Player);
        let audio_cmd = audio_radio.read().audio_cmd.clone();
        let nav_radio = use_radio::<Data, DataChannel>(DataChannel::Navigation);
        let nav_cmd = nav_radio.read().nav_cmd.clone();

        let video_id = self.id.clone();
        let song_title = self.title.clone();
        let song_artist = self.artist.clone();
        let song_album = self.album.clone();
        let song_thumbnail = self.thumbnail.clone();

        let size = if self.is_video { 402. } else { 223. };
        let height = 223.;
        let play_btn_size = 48.;

        let mut anim_album_play = use_animation(|_| {
            (
                AnimColor::new(Color::BLACK.with_a(180), Color::BLACK)
                    .function(Function::Sine)
                    .ease(Ease::InOut)
                    .time(100),
                AnimNum::new(40., 60.)
                    .function(Function::Sine)
                    .ease(Ease::InOut)
                    .time(100),
            )
        });

        let (album_play_color, album_play_size) = anim_album_play.read().value();
        let album_play_size = Size::px(album_play_size);

        rect()
            .vertical()
            .spacing(12.)
            .max_width(Size::px(size))
            .child(
                rect()
                    .expanded()
                    .center()
                    .width(Size::px(size))
                    .height(Size::px(height))
                    .corner_radius(if self.is_artist { size } else { 8. })
                    .overflow(Overflow::Clip)
                    .on_pointer_enter(move |_| {
                        Cursor::set(CursorIcon::Pointer);
                        hover.set(true);
                    })
                    .on_pointer_leave(move |_| {
                        Cursor::set(CursorIcon::Default);
                        hover.set(false);
                    })
                    .on_secondary_down(move |_| ContextMenu::open(Menu::new()))
                    .child(
                        ImageViewer::new(Url::from_str(self.thumbnail.as_str()).unwrap())
                            .expanded()
                            .center()
                            .image_cover(ImageCover::Center)
                            .child(
                                rect()
                                    .expanded()
                                    .center()
                                    .on_press({
                                        let is_album = self.is_album;
                                        let is_artist = self.is_artist;
                                        let is_playlist = self.is_playlist;
                                        let audio_cmd = audio_cmd.clone();
                                        let nav_cmd = nav_cmd.clone();
                                        let video_id = video_id.clone();
                                        let title = song_title.clone();
                                        let artist = song_artist.clone();
                                        let album = song_album.clone();
                                        let thumbnail_url = song_thumbnail.clone();
                                        move |_| {
                                            if is_playlist {
                                                // Begin navigation: show loading bar and fetch
                                                // data before pushing the route (AppLayout
                                                // handles the push once data arrives).
                                                if let Some(tx) = nav_cmd.clone() {
                                                    tx.try_send(NavCommand::BeginNavigation {
                                                        playlist_id: video_id.clone(),
                                                    })
                                                    .ok();
                                                    tx.try_send(NavCommand::LoadPlaylist(
                                                        video_id.clone(),
                                                    ))
                                                    .ok();
                                                }
                                            } else if !is_album && !is_artist {
                                                is_playing.toggle();
                                                if let Some(tx) = audio_cmd.clone() {
                                                    tx.try_send(AudioCommand::Play {
                                                        video_id: video_id.clone(),
                                                        quality: AudioQuality::Medium,
                                                        title: title.clone(),
                                                        artist: artist.clone(),
                                                        album: album.clone(),
                                                        thumbnail_url: thumbnail_url.clone(),
                                                    })
                                                    .ok();
                                                }
                                            }
                                        }
                                    })
                                    .background(Color::BLACK.with_a(
                                        if *hover.read() && !self.is_artist {
                                            60
                                        } else {
                                            0
                                        },
                                    ))
                                    .maybe_child(
                                        (!self.is_album && !self.is_artist && !self.is_playlist)
                                            .then_some(
                                                SvgViewer::new(if *is_playing.read() {
                                                    audio_lines()
                                                } else {
                                                    play()
                                                })
                                                .fill(Color::WHITE)
                                                .color(Color::WHITE)
                                                .width(Size::px(play_btn_size))
                                                .height(Size::px(play_btn_size)),
                                            ),
                                    ),
                            )
                            .maybe_child(
                                (*hover.read() && (self.is_album || self.is_playlist)).then_some(
                                    rect()
                                        .width(album_play_size.clone())
                                        .height(album_play_size)
                                        .rounded_full()
                                        .padding(12.)
                                        .on_press({
                                            let is_playlist = self.is_playlist;
                                            let audio_cmd = audio_cmd.clone();
                                            let nav_cmd = nav_cmd.clone();
                                            let video_id = video_id.clone();
                                            let title = song_title.clone();
                                            let artist = song_artist.clone();
                                            let album = song_album.clone();
                                            let thumbnail_url = song_thumbnail.clone();
                                            move |_| {
                                                is_playing.toggle();
                                                if is_playlist {
                                                    if let Some(tx) = nav_cmd.clone() {
                                                        tx.try_send(NavCommand::PlayPlaylist(
                                                            video_id.clone(),
                                                        ))
                                                        .ok();
                                                    }
                                                } else if let Some(tx) = audio_cmd.clone() {
                                                    tx.try_send(AudioCommand::Play {
                                                        video_id: video_id.clone(),
                                                        quality: AudioQuality::Medium,
                                                        title: title.clone(),
                                                        artist: artist.clone(),
                                                        album: album.clone(),
                                                        thumbnail_url: thumbnail_url.clone(),
                                                    })
                                                    .ok();
                                                }
                                            }
                                        })
                                        .on_pointer_enter(move |_| {
                                            anim_album_play.start();
                                        })
                                        .on_pointer_leave(move |_| {
                                            anim_album_play.reverse();
                                        })
                                        .position(Position::new_absolute().bottom(8.).right(8.))
                                        .background(album_play_color)
                                        .child(
                                            SvgViewer::new(if *is_playing.read() {
                                                audio_lines()
                                            } else {
                                                play()
                                            })
                                            .fill(Color::WHITE)
                                            .color(Color::WHITE)
                                            .expanded(),
                                        ),
                                ),
                            ),
                    ),
            )
            .child(
                rect()
                    .vertical()
                    .spacing(2.)
                    .width(Size::px(size))
                    .child(
                        label()
                            .max_lines(1)
                            .text_align(if self.is_artist {
                                TextAlign::Center
                            } else {
                                TextAlign::Left
                            })
                            .font_weight(FontWeight::BOLD)
                            .text_overflow(TextOverflow::Ellipsis)
                            .text(self.title.clone()),
                    )
                    .child(
                        rect()
                            .width(Size::Fill)
                            .font_weight(FontWeight::NORMAL)
                            .color(Color::from_hex("#B3B3B3").unwrap())
                            .overflow(Overflow::Clip)
                            .child(self.render_info(|id| {
                                tracing::debug!(id, "content item clicked");
                            })),
                    ),
            )
    }
}
