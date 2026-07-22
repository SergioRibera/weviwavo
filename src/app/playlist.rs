use std::str::FromStr;

use freya::animation::*;
use freya::icons::lucide::play;
use freya::prelude::*;
use freya::radio::use_radio;

use crate::app::{Data, DataChannel, NavCommand, PlaylistViewData};
use crate::audio::{AudioCommand, AudioQuality};
use crate::components::{PlaylistSongRow, SongInfo, TopBar};

#[derive(Clone, Copy, PartialEq)]
enum WindowSize {
    Small, // <= 1024
    Large, // > 1024
}

/// Page component for `/playlist/:id`. The `:id` segment is the playlist browse ID.
#[derive(Clone, PartialEq)]
pub struct Playlist {
    pub id: String,
}

impl Component for Playlist {
    fn render(&self) -> impl IntoElement {
        // ── hooks (must all be at the top, no branching) ─────────────────────
        let platform = Platform::get();

        let mut max_width_anim = use_animation(|_| {
            AnimNum::new(70., 100.)
                .function(Function::Sine)
                .ease(Ease::InOut)
                .time(100)
        });

        let mut prev_window_size = use_state(|| {
            let w = platform.root_size.read().width;
            if w <= 1024. {
                WindowSize::Small
            } else {
                WindowSize::Large
            }
        });

        let nav_radio = use_radio::<Data, DataChannel>(DataChannel::Navigation);
        let audio_radio = use_radio::<Data, DataChannel>(DataChannel::Player);
        let mut last_requested = use_state(|| String::new());

        // ── responsive width ─────────────────────────────────────────────────
        let current_width = platform.root_size.read().width;
        let current_ws = if current_width <= 1024. {
            WindowSize::Small
        } else {
            WindowSize::Large
        };

        if current_ws != *prev_window_size.read() {
            match current_ws {
                WindowSize::Small => max_width_anim.start(),
                WindowSize::Large => max_width_anim.reverse(),
            }
            prev_window_size.set(current_ws);
        }

        let max_width = max_width_anim.read().value();

        // ── data ─────────────────────────────────────────────────────────────
        let state = nav_radio.read();
        let nav_cmd = state.nav_cmd.clone();
        let playlist_view = state.playlist_view.clone();
        drop(state);

        let audio_cmd = audio_radio.read().audio_cmd.clone();

        if *last_requested.read() != self.id {
            last_requested.set(self.id.clone());
            if let Some(tx) = nav_cmd {
                tx.try_send(NavCommand::LoadPlaylist(self.id.clone())).ok();
            }
        }

        let loaded = playlist_view
            .as_ref()
            .filter(|pv| pv.playlist.id == self.id)
            .cloned();

        // ── loading state ─────────────────────────────────────────────────────
        if loaded.is_none() {
            return rect()
                .vertical()
                .expanded()
                .content(Content::Flex)
                .child(TopBar)
                .child(
                    rect()
                        .width(Size::Fill)
                        .height(Size::flex(1.0))
                        .center()
                        .child(label().text("Cargando...").color(Color::WHITE.with_a(180))),
                );
        }

        let pv = loaded.unwrap();

        // ── main layout ───────────────────────────────────────────────────────
        rect()
            .vertical()
            .expanded()
            .content(Content::Flex)
            .child(TopBar)
            .child(
                rect().width(Size::Fill).height(Size::flex(1.0)).child(
                    ScrollView::new()
                        .expanded()
                        .direction(Direction::Vertical)
                        .child(
                            rect()
                                .vertical()
                                .padding(Gaps::new_all(24.))
                                .width(Size::Fill)
                                .center()
                                .child(render_content(pv, audio_cmd, max_width as f32)),
                        ),
                ),
            )
    }
}

fn render_content(
    pv: PlaylistViewData,
    audio_cmd: Option<tokio::sync::mpsc::Sender<AudioCommand>>,
    max_width: f32,
) -> Rect {
    let thumbnail_url = pv
        .playlist
        .thumbnail
        .as_deref()
        .and_then(|u| Url::from_str(u).ok());
    let title = pv.playlist.title.clone();
    let author = pv.playlist.author.clone().unwrap_or_default();
    let song_count = pv.playlist.song_count_text.clone().unwrap_or_default();
    let songs = pv.songs.clone();
    let suggestions = pv.suggestions.clone();
    let related_playlists = pv.related_playlists.clone();

    // Wide = large screen two-column layout (header left | songs right).
    // Narrow = small screen single-column layout (header top, songs below).
    let is_wide = max_width < 90.;

    rect()
        .direction(if is_wide {
            Direction::Horizontal
        } else {
            Direction::Vertical
        })
        .width(Size::Fill)
        .content(Content::Flex)
        .max_width(Size::percent(max_width))
        .spacing(32.)
        .color(Color::WHITE)
        // ── Playlist header ───────────────────────────────────────────────────
        // Wide:   vertical stack (thumbnail → info → play), fixed width column.
        // Narrow: horizontal row (thumbnail | info+play), full width.
        .child(render_header(
            is_wide,
            thumbnail_url,
            title,
            author,
            song_count,
            songs.clone(),
            audio_cmd.clone(),
        ))
        // ── Songs + extras ────────────────────────────────────────────────────
        .child(
            rect()
                .vertical()
                .width(if is_wide { Size::flex(1.) } else { Size::Fill })
                .spacing(2.)
                .children(
                    songs
                        .iter()
                        .enumerate()
                        .map(|(i, s)| PlaylistSongRow::song(s.clone(), i))
                        .map(IntoElement::into_element),
                )
                .maybe_child((!suggestions.is_empty()).then_some(
                    rect()
                        .vertical()
                        .width(Size::Fill)
                        .spacing(8.)
                        .child(rect().height(Size::px(8.)))
                        .child(
                            label()
                                .text("Sugerencias")
                                .font_weight(FontWeight::BOLD)
                                .font_size(20.)
                                .color(Color::WHITE),
                        )
                        .children(
                            suggestions
                                .iter()
                                .map(|s| PlaylistSongRow::suggestion(s.clone()))
                                .map(IntoElement::into_element),
                        ),
                ))
                .maybe_child((!related_playlists.is_empty()).then_some(
                    rect()
                        .vertical()
                        .width(Size::Fill)
                        .spacing(16.)
                        .child(rect().height(Size::px(8.)))
                        .child(
                            label()
                                .text("Listas de reproducción relacionadas")
                                .font_weight(FontWeight::BOLD)
                                .font_size(20.)
                                .color(Color::WHITE),
                        )
                        .child(
                            rect()
                                .horizontal()
                                .width(Size::Fill)
                                .spacing(16.)
                                .children(
                                    related_playlists
                                        .iter()
                                        .map(|p| {
                                            SongInfo::from(&ytdroid::models::YTItem::Playlist(
                                                p.clone(),
                                            ))
                                        })
                                        .map(IntoElement::into_element),
                                ),
                        ),
                )),
        )
}

fn render_header(
    is_wide: bool,
    thumbnail_url: Option<Url>,
    title: String,
    author: String,
    song_count: String,
    songs: Vec<ytdroid::models::SongItem>,
    audio_cmd: Option<tokio::sync::mpsc::Sender<AudioCommand>>,
) -> Rect {
    // Wide: column with thumbnail on top, info + play centred below.
    // Narrow: row with thumbnail on the left, info + play on the right.
    let thumbnail_size = if is_wide { 160. } else { 200. };

    let thumbnail = rect()
        .width(Size::px(thumbnail_size))
        .height(Size::px(thumbnail_size))
        .corner_radius(8.)
        .overflow(Overflow::Clip)
        .background(Color::from_hex("#333333").unwrap())
        .maybe_child(thumbnail_url.map(|url| {
            ImageViewer::new(url)
                .expanded()
                .image_cover(ImageCover::Center)
        }));

    let info = rect()
        .vertical()
        .width(if is_wide { Size::Fill } else { Size::flex(1.) })
        .cross_align(if is_wide {
            Alignment::Center
        } else {
            Alignment::Start
        })
        .spacing(8.)
        .child(
            label()
                .text(title)
                .font_weight(FontWeight::BOLD)
                .font_size(if is_wide { 18. } else { 28. })
                .max_lines(2)
                .text_overflow(TextOverflow::Ellipsis)
                .text_align(if is_wide {
                    TextAlign::Center
                } else {
                    TextAlign::Left
                })
                .color(Color::WHITE),
        )
        .maybe_child((!author.is_empty()).then_some(
            label()
                .text(author)
                .font_size(13.)
                .color(Color::from_hex("#B3B3B3").unwrap()),
        ))
        .maybe_child((!song_count.is_empty()).then_some(
            label()
                .text(song_count)
                .font_size(12.)
                .color(Color::from_hex("#888888").unwrap()),
        ))
        .child(
            rect()
                .rounded_full()
                .padding(Gaps::new_all(12.))
                .background(Color::WHITE)
                .on_press({
                    let audio_cmd = audio_cmd.clone();
                    move |_| {
                        if let Some(first) = songs.first() {
                            send_play(&audio_cmd, first);
                        }
                    }
                })
                .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                .on_pointer_leave(|_| Cursor::set(CursorIcon::Default))
                .child(
                    SvgViewer::new(play())
                        .fill(Color::BLACK)
                        .color(Color::BLACK)
                        .width(Size::px(20.))
                        .height(Size::px(20.)),
                ),
        );

    if is_wide {
        // Vertical column: thumbnail centred, then info below
        rect()
            .vertical()
            .width(Size::px(220.))
            .cross_align(Alignment::Center)
            .spacing(16.)
            .padding(Gaps::new(0., 0., 16., 0.))
            .child(thumbnail)
            .child(info)
    } else {
        // Horizontal row: thumbnail | info
        rect()
            .horizontal()
            .width(Size::Fill)
            .content(Content::Flex)
            .spacing(24.)
            .cross_align(Alignment::Center)
            .child(thumbnail)
            .child(info)
    }
}

fn send_play(
    audio_cmd: &Option<tokio::sync::mpsc::Sender<AudioCommand>>,
    song: &ytdroid::models::SongItem,
) {
    if let Some(tx) = audio_cmd {
        tx.try_send(AudioCommand::Play {
            video_id: song.id.clone(),
            quality: AudioQuality::Medium,
            title: song.title.clone(),
            artist: song
                .artists
                .first()
                .map(|a| a.name.clone())
                .unwrap_or_default(),
            album: song
                .album
                .as_ref()
                .map(|a| a.name.clone())
                .unwrap_or_default(),
            thumbnail_url: song.thumbnail.clone().unwrap_or_default(),
        })
        .ok();
    }
}
