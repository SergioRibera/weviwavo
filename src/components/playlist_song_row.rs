use std::str::FromStr;

use freya::icons::lucide::{ellipsis_vertical, play, thumbs_down, thumbs_up};
use freya::prelude::*;
use freya::radio::use_radio;
use freya::router::RouterContext;
use ytdroid::models::SongItem;

use crate::app::{Data, DataChannel, QueueSong, Route};
use crate::audio::{AudioCommand, AudioQuality};
use crate::components::song_context_menu::song_context_menu;

fn fmt_duration(secs: u32) -> String {
    let m = secs / 60;
    let s = secs % 60;
    format!("{m}:{s:02}")
}

#[derive(Clone, PartialEq)]
pub struct PlaylistSongRow {
    pub song: SongItem,
    /// When true, the row belongs to the Suggestions section. On press,
    /// a single-song `Play` is sent instead of `PlayFromQueue`.
    pub is_suggestion: bool,
    /// Index of this song in the current playlist queue (ignored for suggestions).
    pub queue_index: usize,
}

impl PlaylistSongRow {
    #[must_use]
    pub fn song(song: SongItem, queue_index: usize) -> Self {
        Self { song, is_suggestion: false, queue_index }
    }

    #[must_use]
    pub fn suggestion(song: SongItem) -> Self {
        Self { song, is_suggestion: true, queue_index: 0 }
    }
}

impl Component for PlaylistSongRow {
    #[allow(clippy::too_many_lines)] // row layout is inherently wide
    fn render(&self) -> impl IntoElement {
        // ── hooks — all at the top ────────────────────────────────────────────
        let audio_radio = use_radio::<Data, DataChannel>(DataChannel::Player);
        let nav_radio = use_radio::<Data, DataChannel>(DataChannel::Navigation);
        let router = RouterContext::get();
        let mut hover = use_state(|| false);
        let mut thumb_hover = use_state(|| false);
        let mut artist_hover = use_state(|| false);
        let mut album_hover = use_state(|| false);

        // ── data extraction ───────────────────────────────────────────────────
        let audio_cmd = audio_radio.read().audio_cmd.clone();

        let playlist_songs: Vec<QueueSong> = if !self.is_suggestion {
            nav_radio
                .read()
                .playlist_view
                .as_ref()
                .map(|pv| {
                    pv.songs
                        .iter()
                        .map(|s| QueueSong {
                            video_id: s.id.clone(),
                            title: s.title.clone(),
                            artist: s.artists.first().map(|a| a.name.clone()).unwrap_or_default(),
                            album: s.album.as_ref().map(|a| a.name.clone()).unwrap_or_default(),
                            thumbnail_url: s.thumbnail.clone().unwrap_or_default(),
                        })
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let video_id = self.song.id.clone();
        let title = self.song.title.clone();
        let artist_name =
            self.song.artists.first().map(|a| a.name.clone()).unwrap_or_default();
        let artist_id = self.song.artists.first().and_then(|a| a.id.clone());
        let album_name = self.song.album.as_ref().map(|a| a.name.clone()).unwrap_or_default();
        let album_id = self.song.album.as_ref().map(|a| a.id.clone());
        let thumb_url = self
            .song
            .thumbnail
            .as_deref()
            .and_then(|u| Url::from_str(u).ok());
        let duration = self.song.duration.map(fmt_duration).unwrap_or_default();

        let is_suggestion = self.is_suggestion;
        let queue_index = self.queue_index;
        let thumbnail_url_str =
            self.song.thumbnail.clone().unwrap_or_default();

        // Captured for context-menu builder (called at event time, not render time).
        let ctx_artist_id = artist_id.clone();
        let ctx_album_id = album_id.clone();
        let ctx_router = router.clone();

        // ── layout ────────────────────────────────────────────────────────────
        rect()
            .horizontal()
            .spacing(12.)
            .padding(Gaps::new_symmetric(4., 8.))
            .width(Size::Fill)
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .corner_radius(6.)
            .background(if *hover.read() {
                Color::WHITE.with_a(15)
            } else {
                Color::TRANSPARENT
            })
            .on_pointer_enter(move |_| hover.set(true))
            .on_pointer_leave(move |_| hover.set(false))
            .on_secondary_down(move |e: Event<PressEventData>| {
                ContextMenu::open_from_event(
                    &e,
                    song_context_menu(
                        ctx_artist_id.clone(),
                        ctx_album_id.clone(),
                        ctx_router.clone(),
                    ),
                );
            })
            // ── Thumbnail zone (also triggers play) ───────────────────────────
            .child({
                let audio_cmd = audio_cmd.clone();
                let playlist_songs = playlist_songs.clone();
                let video_id = video_id.clone();
                let title = title.clone();
                let artist_name = artist_name.clone();
                let album_name = album_name.clone();
                let thumbnail_url_str = thumbnail_url_str.clone();
                let thumb_url_display = thumb_url.clone();

                rect()
                    .width(Size::px(50.))
                    .height(Size::px(50.))
                    .corner_radius(4.)
                    .overflow(Overflow::Clip)
                    .on_pointer_enter(move |_| {
                        Cursor::set(CursorIcon::Pointer);
                        thumb_hover.set(true);
                    })
                    .on_pointer_leave(move |_| {
                        Cursor::set(CursorIcon::Default);
                        thumb_hover.set(false);
                    })
                    .on_press(move |_| {
                        let Some(tx) = audio_cmd.clone() else { return };
                        if is_suggestion || playlist_songs.is_empty() {
                            tx.try_send(AudioCommand::Play {
                                video_id: video_id.clone(),
                                quality: AudioQuality::Medium,
                                title: title.clone(),
                                artist: artist_name.clone(),
                                album: album_name.clone(),
                                thumbnail_url: thumbnail_url_str.clone(),
                            })
                            .ok();
                        } else {
                            tx.try_send(AudioCommand::PlayFromQueue {
                                songs: playlist_songs.clone(),
                                index: queue_index,
                                quality: AudioQuality::Medium,
                            })
                            .ok();
                        }
                    })
                    .maybe_child(thumb_url_display.map(|url| {
                        let hovered = *thumb_hover.read();
                        ImageViewer::new(url)
                            .expanded()
                            .image_cover(ImageCover::Center)
                            .child(
                                rect()
                                    .expanded()
                                    .center()
                                    .background(Color::BLACK.with_a(if hovered { 120 } else { 0 }))
                                    .maybe_child(hovered.then(|| {
                                        SvgViewer::new(play())
                                            .color(Color::WHITE)
                                            .fill(Color::WHITE)
                                            .width(Size::px(16.))
                                            .height(Size::px(16.))
                                    })),
                            )
                    }))
            })
            // ── Info zone: title + artist/album (artist/album navigate independently) ──
            .child({
                let router_title = router.clone();

                rect()
                    .vertical()
                    .spacing(3.)
                    .width(Size::flex(1.))
                    // Title — display only; play is triggered from the thumbnail
                    .child(
                        label()
                            .text(title.clone())
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .font_weight(FontWeight::MEDIUM)
                            .color(Color::WHITE),
                    )
                    // Subtitle row: [artist] • [album]  — each navigates independently
                    .child(
                        rect()
                            .horizontal()
                            .spacing(5.)
                            .width(Size::Fill)
                            // Artist label(s)
                            .maybe_child((!artist_name.is_empty()).then(|| {
                                let router = router_title.clone();
                                let id = artist_id.clone();
                                let is_hovered = *artist_hover.read();
                                rect()
                                    .on_pointer_enter(move |_| {
                                        Cursor::set(CursorIcon::Pointer);
                                        hover.set(true);
                                        artist_hover.set(true);
                                    })
                                    .on_pointer_leave(move |_| {
                                        Cursor::set(CursorIcon::Default);
                                        artist_hover.set(false);
                                    })
                                    .on_press(move |_| {
                                        if let Some(ref browse_id) = id {
                                            router
                                                .push(Route::Artist {
                                                    id: browse_id.clone(),
                                                })
                                                .ok();
                                        }
                                    })
                                    .child(
                                        paragraph()
                                            .max_lines(1)
                                            .text_overflow(TextOverflow::Ellipsis)
                                            .span(
                                                Span::new(artist_name.clone())
                                                    .font_size(12.)
                                                    .color(Color::from_hex("#B3B3B3").unwrap())
                                                    .text_decoration(if is_hovered {
                                                        TextDecoration::Underline
                                                    } else {
                                                        TextDecoration::None
                                                    }),
                                            ),
                                    )
                            }))
                            // Separator " • " (only if both artist and album present)
                            .maybe_child(
                                (!artist_name.is_empty() && !album_name.is_empty()).then(|| {
                                    label()
                                        .text(" • ")
                                        .font_size(12.)
                                        .color(Color::from_hex("#B3B3B3").unwrap())
                                }),
                            )
                            // Album label
                            .maybe_child((!album_name.is_empty()).then(|| {
                                let router = router_title.clone();
                                let id = album_id.clone();
                                let is_hovered = *album_hover.read();
                                rect()
                                    .on_pointer_enter(move |_| {
                                        Cursor::set(CursorIcon::Pointer);
                                        hover.set(true);
                                        album_hover.set(true);
                                    })
                                    .on_pointer_leave(move |_| {
                                        Cursor::set(CursorIcon::Default);
                                        album_hover.set(false);
                                    })
                                    .on_press(move |_| {
                                        if let Some(ref browse_id) = id {
                                            router
                                                .push(Route::Album {
                                                    id: browse_id.clone(),
                                                })
                                                .ok();
                                        }
                                    })
                                    .child(
                                        paragraph()
                                            .max_lines(1)
                                            .text_overflow(TextOverflow::Ellipsis)
                                            .span(
                                                Span::new(album_name.clone())
                                                    .font_size(12.)
                                                    .color(Color::from_hex("#B3B3B3").unwrap())
                                                    .text_decoration(if is_hovered {
                                                        TextDecoration::Underline
                                                    } else {
                                                        TextDecoration::None
                                                    }),
                                            ),
                                    )
                            })),
                    )
            })
            // ── Duration ──────────────────────────────────────────────────────
            .child(
                label()
                    .text(duration)
                    .font_size(13.)
                    .color(Color::from_hex("#B3B3B3").unwrap()),
            )
            // ── Action buttons (visible on hover) ─────────────────────────────
            .maybe_child((*hover.read()).then(|| {
                rect()
                    .horizontal()
                    .spacing(4.)
                    .cross_align(Alignment::Center)
                    .child(
                        rect()
                            .center()
                            .padding(Gaps::new_all(4.))
                            .rounded_full()
                            .on_pointer_enter(move |_| {
                                Cursor::set(CursorIcon::Pointer);
                                hover.set(true);
                            })
                            .on_pointer_leave(|_| Cursor::set(CursorIcon::Default))
                            .child(
                                SvgViewer::new(thumbs_up())
                                    .color(Color::from_hex("#B3B3B3").unwrap())
                                    .fill(Color::TRANSPARENT)
                                    .width(Size::px(15.))
                                    .height(Size::px(15.)),
                            ),
                    )
                    .child(
                        rect()
                            .center()
                            .padding(Gaps::new_all(4.))
                            .rounded_full()
                            .on_pointer_enter(move |_| {
                                Cursor::set(CursorIcon::Pointer);
                                hover.set(true);
                            })
                            .on_pointer_leave(|_| Cursor::set(CursorIcon::Default))
                            .child(
                                SvgViewer::new(thumbs_down())
                                    .color(Color::from_hex("#B3B3B3").unwrap())
                                    .fill(Color::TRANSPARENT)
                                    .width(Size::px(15.))
                                    .height(Size::px(15.)),
                            ),
                    )
            }))
    }
}
