use std::str::FromStr;

use freya::animation::*;
use freya::icons::lucide::{audio_lines, play};
use freya::prelude::*;
use ytmapi_rs::common::{Thumbnail, YoutubeID};
use ytmapi_rs::parse::HomeContent;

use super::TextInfo;

#[derive(Clone, PartialEq)]
pub struct SongInfo {
    id: String,
    title: String,
    left: TextInfo,
    details: Option<TextInfo>,
    is_album: bool,
    is_artist: bool,
    is_video: bool,
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

    fn get_best_thumbnail(thumbnails: &[Thumbnail]) -> String {
        thumbnails
            .iter()
            .max_by_key(|t| t.width * t.height)
            .map(|t| t.url.clone())
            .unwrap_or_default()
    }
}

impl<'a> From<&'a HomeContent> for SongInfo {
    fn from(value: &'a HomeContent) -> Self {
        match value {
            HomeContent::Album(v) => {
                let left = if !v.artists.is_empty() {
                    TextInfo::authors(v.artists.clone())
                } else {
                    TextInfo::plain("Album", None)
                };

                Self {
                    id: v.album_id.get_raw().to_string(),
                    title: v.title.clone(),
                    left,
                    details: v.year.as_ref().map(|y| TextInfo::plain(y.clone(), None)),
                    is_album: true,
                    is_artist: false,
                    is_video: false,
                    thumbnail: Self::get_best_thumbnail(&v.thumbnails),
                }
            }
            HomeContent::Playlist(v) => {
                let left = if !v.author.is_empty() {
                    TextInfo::authors(v.author.clone())
                } else {
                    TextInfo::none()
                };

                let details = if let Some(desc) = &v.description {
                    Some(TextInfo::plain(desc.clone(), None))
                } else {
                    v.count
                        .as_ref()
                        .map(|c| TextInfo::plain(format!("{c} songs"), None))
                };

                Self {
                    id: v.playlist_id.get_raw().to_string(),
                    title: v.title.clone(),
                    left,
                    details,
                    is_album: false,
                    is_artist: false,
                    is_video: false,
                    thumbnail: Self::get_best_thumbnail(&v.thumbnails),
                }
            }
            HomeContent::WatchPlaylist(v) => {
                let left = TextInfo::plain("Playlist", None);

                Self {
                    id: v.playlist_id.get_raw().to_string(),
                    title: v.title.clone(),
                    left,
                    details: None,
                    is_album: true,
                    is_artist: false,
                    is_video: false,
                    thumbnail: Self::get_best_thumbnail(&v.thumbnails),
                }
            }
            HomeContent::Artist(v) => Self {
                id: v.channel_id.get_raw().to_string(),
                title: v.title.clone(),
                left: TextInfo::none(),
                details: v.subscribers.as_ref().map(|s| {
                    TextInfo::plain(format!("{s} de suscriptores"), Some(TextAlign::Center))
                }),
                is_album: false,
                is_artist: true,
                is_video: false,
                thumbnail: Self::get_best_thumbnail(&v.thumbnails),
            },
            HomeContent::Song(v) => {
                let left = if !v.artists.is_empty() {
                    TextInfo::authors(v.artists.clone())
                } else {
                    TextInfo::none()
                };

                let details = v.album.as_ref().map(|album| {
                    if !album.id.get_raw().is_empty() {
                        TextInfo::clickable(album.id.get_raw().to_string(), album.name.clone())
                    } else {
                        TextInfo::plain(album.name.clone(), None)
                    }
                });

                Self {
                    id: v.video_id.get_raw().to_string(),
                    title: v.title.clone(),
                    left,
                    details,
                    is_album: false,
                    is_artist: false,
                    is_video: false,
                    thumbnail: Self::get_best_thumbnail(&v.thumbnails),
                }
            }
            HomeContent::Video(v) => Self {
                id: v.video_id.get_raw().to_string(),
                title: v.title.clone(),
                left: if !v.artists.is_empty() {
                    TextInfo::authors(v.artists.clone())
                } else {
                    TextInfo::none()
                },
                details: None,
                is_album: false,
                is_artist: false,
                is_video: true,
                thumbnail: Self::get_best_thumbnail(&v.thumbnails),
            },
        }
    }
}

impl Component for SongInfo {
    fn render(&self) -> impl IntoElement {
        let mut is_playing = use_state(|| false);
        let size = use_state(|| if self.is_video { 402. } else { 223. });
        let height = use_state(|| 223.);
        let play_btn_size = use_state(|| 48.);
        let mut hover = use_state(|| false);

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
            .max_width(Size::px(size()))
            .child(
                rect()
                    .expanded()
                    .center()
                    .width(Size::px(size()))
                    .height(Size::px(height()))
                    .corner_radius(if self.is_artist { size() } else { 8. })
                    .overflow(Overflow::Clip)
                    .on_pointer_enter(move |_| {
                        Cursor::set(CursorIcon::Pointer);
                        hover.set(true);
                    })
                    .on_pointer_leave(move |_| {
                        Cursor::set(CursorIcon::Default);
                        hover.set(false);
                    })
                    .on_secondary_press(move |_| ContextMenu::open(Menu::new()))
                    .child(
                        ImageViewer::new(Uri::from_str(self.thumbnail.as_str()).unwrap())
                            .expanded()
                            .center()
                            .image_cover(ImageCover::Center)
                            .center()
                            .child(
                                rect()
                                    .expanded()
                                    .center()
                                    .on_press({
                                        let is_album = self.is_album;
                                        let is_artist = self.is_artist;
                                        move |_| {
                                            if !is_album && !is_artist {
                                                is_playing.toggle()
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
                                        (!self.is_album && !self.is_artist).then_some(
                                            svg(if *is_playing.read() {
                                                audio_lines()
                                            } else {
                                                play()
                                            })
                                            .fill(Color::WHITE)
                                            .color(Color::WHITE)
                                            .width(Size::px(play_btn_size()))
                                            .height(Size::px(play_btn_size())),
                                        ),
                                    ),
                            )
                            .maybe_child(
                                (*hover.read() && self.is_album).then_some(
                                    rect()
                                        .width(album_play_size.clone())
                                        .height(album_play_size)
                                        .rounded_full()
                                        .padding(12.)
                                        .on_press(move |_| is_playing.toggle())
                                        .on_pointer_enter(move |_| {
                                            anim_album_play.start();
                                        })
                                        .on_pointer_leave(move |_| {
                                            anim_album_play.reverse();
                                        })
                                        .position(Position::new_absolute().bottom(8.).right(8.))
                                        .background(album_play_color)
                                        .child(
                                            svg(if *is_playing.read() {
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
                    .width(Size::px(size()))
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
                                // Handle click here
                                println!("Clicked on: {id}");
                            })),
                    ),
            )
    }
}
