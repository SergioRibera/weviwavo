use freya::icons::lucide::play;
use freya::prelude::*;
use freya::radio::use_radio;
use tokio::sync::mpsc;

use crate::app::{Data, DataChannel, QueueSong};
use crate::audio::{AudioCommand, AudioQuality};
use super::use_cached_thumbnail;

// ── QueueSongRow ──────────────────────────────────────────────────────────────

struct QueueSongRow {
    song: QueueSong,
    /// Index of this entry in the original `queue` Vec.
    index: usize,
    is_current: bool,
    queue: Vec<QueueSong>,
    audio_cmd: Option<mpsc::Sender<AudioCommand>>,
    layout: LayoutData,
}

impl PartialEq for QueueSongRow {
    fn eq(&self, other: &Self) -> bool {
        self.song == other.song
            && self.index == other.index
            && self.is_current == other.is_current
    }
}

impl LayoutExt for QueueSongRow {
    fn get_layout(&mut self) -> &mut LayoutData {
        &mut self.layout
    }
}

impl ContainerExt for QueueSongRow {}

impl Component for QueueSongRow {
    fn render(&self) -> impl IntoElement {
        let thumb_src = use_cached_thumbnail(&self.song.thumbnail_url);
        let mut hover = use_state(|| false);

        let is_current = self.is_current;
        let title = self.song.title.clone();
        let artist = self.song.artist.clone();
        let queue = self.queue.clone();
        let index = self.index;
        let audio_cmd = self.audio_cmd.clone();

        let bg = if is_current {
            Color::from_hex("#2D2D2D").unwrap()
        } else if *hover.read() {
            Color::WHITE.with_a(15)
        } else {
            Color::TRANSPARENT
        };

        rect()
            .horizontal()
            .width(Size::Fill)
            .height(Size::px(56.))
            .padding(Gaps::new_symmetric(0., 8.))
            .cross_align(Alignment::Center)
            .spacing(12.)
            .corner_radius(6.)
            .background(bg)
            .on_pointer_enter(move |_| {
                Cursor::set(CursorIcon::Pointer);
                hover.set(true);
            })
            .on_pointer_leave(move |_| {
                Cursor::set(CursorIcon::Default);
                hover.set(false);
            })
            .on_press(move |_| {
                let Some(tx) = audio_cmd.clone() else { return };
                tx.try_send(AudioCommand::PlayFromQueue {
                    songs: queue.clone(),
                    index,
                    quality: AudioQuality::Medium,
                })
                .ok();
            })
            // Play icon for the current song
            .maybe_child(is_current.then(|| {
                SvgViewer::new(play())
                    .color(Color::WHITE)
                    .fill(Color::WHITE)
                    .width(Size::px(10.))
                    .height(Size::px(10.))
            }))
            // Thumbnail
            .child(
                rect()
                    .width(Size::px(40.))
                    .height(Size::px(40.))
                    .corner_radius(3.)
                    .overflow(Overflow::Clip)
                    .maybe_child(thumb_src.map(|src| {
                        ImageViewer::new(src)
                            .expanded()
                            .image_cover(ImageCover::Center)
                    })),
            )
            // Text column
            .child(
                rect()
                    .vertical()
                    .spacing(2.)
                    .width(Size::flex(1.))
                    .child(
                        label()
                            .text(title)
                            .font_size(13.)
                            .font_weight(FontWeight::BOLD)
                            .color(Color::WHITE)
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis),
                    )
                    .child(
                        label()
                            .text(artist)
                            .font_size(11.)
                            .color(Color::from_hex("#AAAAAA").unwrap())
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis),
                    ),
            )
    }
}

// ── PlayerExpanded ────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Default)]
pub struct PlayerExpanded {
    layout: LayoutData,
}

impl LayoutExt for PlayerExpanded {
    fn get_layout(&mut self) -> &mut LayoutData {
        &mut self.layout
    }
}

impl ContainerExt for PlayerExpanded {}

impl Component for PlayerExpanded {
    #[allow(clippy::too_many_lines)]
    fn render(&self) -> impl IntoElement {
        let radio = use_radio::<Data, DataChannel>(DataChannel::Player);
        let state = radio.read();
        let p = &state.player;

        let album = p.album.clone();
        let queue = p.queue.clone();
        let queue_index = p.queue_index;
        let audio_cmd = state.audio_cmd.clone();
        drop(state);

        let mut left_tab = use_state(|| 0usize);
        let mut right_tab = use_state(|| 0usize);
        let mut autoplay = use_state(|| true);

        // Window size
        let root_size = Platform::get().root_size.read();
        let window_w = root_size.width;
        let window_h = root_size.height;
        drop(root_size);

        const HOMEBAR_H: f32 = 64.;
        const PLAYERBAR_H: f32 = 67.;
        let content_area_h = (window_h - HOMEBAR_H - PLAYERBAR_H).max(0.);

        // Art size: fill available space minus padding (40px each side = 80px), capped by
        // content height minus tab row (~50px) + some breathing room (110px total).
        let art_size = (window_w / 2. - 80.)
            .min(content_area_h - 160.)
            .max(100.);

        rect()
            .horizontal()
            .expanded()
            .background(Color::from_hex("#111111").unwrap())
            // ── Left panel ────────────────────────────────────────────────────
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.))
                    .height(Size::Fill)
                    .padding(Gaps::new_all(40.))
                    .spacing(16.)
                    // Tabs row
                    .child(
                        rect()
                            .horizontal()
                            .main_align(Alignment::Center)
                            .spacing(8.)
                            .child({
                                let selected = *left_tab.read() == 0;
                                rect()
                                    .padding(Gaps::new_symmetric(8., 16.))
                                    .corner_radius(20.)
                                    .background(if selected {
                                        Color::from_hex("#3D3D3D").unwrap()
                                    } else {
                                        Color::TRANSPARENT
                                    })
                                    .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                                    .on_pointer_leave(|_| Cursor::set(CursorIcon::Default))
                                    .on_press(move |_| left_tab.set(0))
                                    .child(
                                        label()
                                            .text("CANCIÓN")
                                            .font_size(12.)
                                            .font_weight(FontWeight::BOLD)
                                            .color(Color::WHITE),
                                    )
                            })
                            .child({
                                let selected = *left_tab.read() == 1;
                                rect()
                                    .padding(Gaps::new_symmetric(8., 16.))
                                    .corner_radius(20.)
                                    .background(if selected {
                                        Color::from_hex("#3D3D3D").unwrap()
                                    } else {
                                        Color::TRANSPARENT
                                    })
                                    .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                                    .on_pointer_leave(|_| Cursor::set(CursorIcon::Default))
                                    .on_press(move |_| left_tab.set(1))
                                    .child(
                                        label()
                                            .text("VÍDEO")
                                            .font_size(12.)
                                            .font_weight(FontWeight::BOLD)
                                            .color(if selected {
                                                Color::WHITE
                                            } else {
                                                Color::from_hex("#888888").unwrap()
                                            }),
                                    )
                            }),
                    )
                    // Album art
                    .child(
                        rect()
                            .expanded()
                            .center()
                            .child(
                                rect()
                                    .width(Size::px(art_size))
                                    .height(Size::px(art_size))
                                    .corner_radius(8.)
                                    .overflow(Overflow::Clip)
                                    .maybe_child({
                                        let url = radio.read().player.thumbnail_url.clone();
                                        use_cached_thumbnail(&url).map(|src| {
                                            ImageViewer::new(src)
                                                .expanded()
                                                .image_cover(ImageCover::Center)
                                        })
                                    }),
                            ),
                    ),
            )
            // ── Right panel ───────────────────────────────────────────────────
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.))
                    .height(Size::Fill)
                    // Tab bar
                    .child(
                        rect()
                            .horizontal()
                            .width(Size::Fill)
                            .height(Size::px(48.))
                            .cross_align(Alignment::Center)
                            .padding(Gaps::new_symmetric(0., 16.))
                            .spacing(16.)
                            .children(
                                ["A CONTINUACIÓN", "LETRA", "COMENTARIOS", "RELACIONADO"]
                                    .into_iter()
                                    .enumerate()
                                    .map(|(i, label_text)| {
                                        let active = *right_tab.read() == i;
                                        rect()
                                            .height(Size::Fill)
                                            .cross_align(Alignment::Center)
                                            .border(if active {
                                                Border::new()
                                                    .width(BorderWidth {
                                                        bottom: 2.,
                                                        ..Default::default()
                                                    })
                                                    .fill(Color::WHITE)
                                                    .alignment(BorderAlignment::Inner)
                                            } else {
                                                Border::new()
                                            })
                                            .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                                            .on_pointer_leave(|_| Cursor::set(CursorIcon::Default))
                                            .on_press(move |_| right_tab.set(i))
                                            .child(
                                                label()
                                                    .text(label_text)
                                                    .font_size(12.)
                                                    .font_weight(if active {
                                                        FontWeight::BOLD
                                                    } else {
                                                        FontWeight::NORMAL
                                                    })
                                                    .color(if active {
                                                        Color::WHITE
                                                    } else {
                                                        Color::from_hex("#888888").unwrap()
                                                    }),
                                            )
                                            .into_element()
                                    }),
                            ),
                    )
                    // Divider
                    .child(
                        rect()
                            .height(Size::px(1.))
                            .width(Size::Fill)
                            .background(Color::from_hex("#2D2D2D").unwrap()),
                    )
                    // Scroll content
                    .child(
                        ScrollView::new()
                            .height(Size::flex(1.))
                            .width(Size::Fill)
                            .direction(Direction::Vertical)
                            .show_scrollbar(false)
                            .child({
                                // All content wrapped in a padded vertical column
                                let current_row = queue.get(queue_index).map(|song| {
                                    QueueSongRow {
                                        song: song.clone(),
                                        index: queue_index,
                                        is_current: true,
                                        queue: queue.clone(),
                                        audio_cmd: audio_cmd.clone(),
                                        layout: LayoutData::default(),
                                    }
                                    .into_element()
                                });

                                let on = *autoplay.read();

                                let upcoming: Vec<_> = queue
                                    .iter()
                                    .enumerate()
                                    .skip(queue_index + 1)
                                    .take(50)
                                    .map(|(i, song)| {
                                        QueueSongRow {
                                            song: song.clone(),
                                            index: i,
                                            is_current: false,
                                            queue: queue.clone(),
                                            audio_cmd: audio_cmd.clone(),
                                            layout: LayoutData::default(),
                                        }
                                        .into_element()
                                    })
                                    .collect();

                                rect()
                                    .vertical()
                                    .width(Size::Fill)
                                    .padding(Gaps::new_symmetric(0., 16.))
                                    // Header row
                                    .child(
                                        rect()
                                            .horizontal()
                                            .width(Size::Fill)
                                            .cross_align(Alignment::Center)
                                            .main_align(Alignment::SpaceBetween)
                                            .padding(Gaps::new_symmetric(12., 0.))
                                            .child(
                                                rect()
                                                    .vertical()
                                                    .spacing(2.)
                                                    .child(
                                                        label()
                                                            .text("Reproduciendo de")
                                                            .font_size(12.)
                                                            .color(Color::from_hex("#888888").unwrap()),
                                                    )
                                                    .child(
                                                        label()
                                                            .text(album.clone())
                                                            .font_size(15.)
                                                            .font_weight(FontWeight::BOLD)
                                                            .color(Color::WHITE),
                                                    ),
                                            )
                                            .child(
                                                rect()
                                                    .padding(Gaps::new_symmetric(6., 14.))
                                                    .corner_radius(20.)
                                                    .border(
                                                        Border::new()
                                                            .width(1.)
                                                            .fill(Color::WHITE)
                                                            .alignment(BorderAlignment::Inner),
                                                    )
                                                    .child(
                                                        label()
                                                            .text("Guardar")
                                                            .font_size(12.)
                                                            .font_weight(FontWeight::BOLD)
                                                            .color(Color::WHITE),
                                                    ),
                                            ),
                                    )
                                    // Autoplay row
                                    .child(
                                        rect()
                                            .horizontal()
                                            .width(Size::Fill)
                                            .cross_align(Alignment::Center)
                                            .main_align(Alignment::SpaceBetween)
                                            .padding(Gaps::new_symmetric(12., 0.))
                                            .child(
                                                label()
                                                    .text("Reproducción automática")
                                                    .font_size(14.)
                                                    .color(Color::WHITE),
                                            )
                                            .child(
                                                rect()
                                                    .width(Size::px(40.))
                                                    .height(Size::px(22.))
                                                    .corner_radius(11.)
                                                    .background(if on {
                                                        Color::from_hex("#1A73E8").unwrap()
                                                    } else {
                                                        Color::from_hex("#555555").unwrap()
                                                    })
                                                    .cross_align(Alignment::Center)
                                                    .padding(Gaps::new_all(2.))
                                                    .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                                                    .on_pointer_leave(|_| Cursor::set(CursorIcon::Default))
                                                    .on_press(move |_| autoplay.toggle())
                                                    .child(
                                                        rect()
                                                            .width(Size::px(18.))
                                                            .height(Size::px(18.))
                                                            .rounded_full()
                                                            .background(Color::WHITE)
                                                            .offset_x(if on { 18. } else { 0. }),
                                                    ),
                                            ),
                                    )
                                    // Current song
                                    .maybe_child(current_row)
                                    // Autoplay label
                                    .child(
                                        label()
                                            .text("Reproducción automática activada")
                                            .font_size(11.)
                                            .color(Color::from_hex("#888888").unwrap()),
                                    )
                                    // Filter chips
                                    .child(
                                        rect()
                                            .horizontal()
                                            .spacing(6.)
                                            .padding(Gaps::new_symmetric(8., 0.))
                                            .child(
                                                rect()
                                                    .padding(Gaps::new_symmetric(4., 12.))
                                                    .corner_radius(20.)
                                                    .background(Color::WHITE)
                                                    .child(
                                                        label()
                                                            .text("Todo")
                                                            .font_size(12.)
                                                            .font_weight(FontWeight::BOLD)
                                                            .color(Color::BLACK),
                                                    ),
                                            ),
                                    )
                                    // Upcoming songs
                                    .children(upcoming)
                            }),
                    ),
            )
    }
}
