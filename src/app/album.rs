use freya::prelude::*;
use freya::radio::use_radio;
use freya::router::RouterContext;

use crate::app::{AlbumViewData, Data, DataChannel, NavCommand, QueueSong};
use crate::audio::{AudioCommand, AudioQuality};
use crate::components::{PlaylistSongRow, use_cached_thumbnail};

const ROW_HEIGHT: f32 = 58.0;

#[derive(Clone, PartialEq)]
pub struct Album {
    pub id: String,
}

impl Component for Album {
    fn render(&self) -> impl IntoElement {
        let nav_radio = use_radio::<Data, DataChannel>(DataChannel::Navigation);
        let audio_radio = use_radio::<Data, DataChannel>(DataChannel::Player);
        let router = RouterContext::get();
        let mut last_requested = use_state(|| String::new());

        let thumb_url_str = nav_radio
            .read()
            .album_view
            .as_ref()
            .filter(|av| av.album.id == self.id)
            .and_then(|av| av.album.thumbnail.clone())
            .unwrap_or_default();
        let thumbnail_src = use_cached_thumbnail(&thumb_url_str);

        let state = nav_radio.read();
        let nav_cmd = state.nav_cmd.clone();
        let album_view = state.album_view.clone();
        drop(state);

        let audio_cmd = audio_radio.read().audio_cmd.clone();

        if *last_requested.read() != self.id {
            last_requested.set(self.id.clone());
            if let Some(tx) = nav_cmd {
                tx.try_send(NavCommand::LoadAlbum(self.id.clone())).ok();
            }
        }

        let loaded = album_view.filter(|av| av.album.id == self.id);

        if loaded.is_none() {
            return rect()
                .expanded()
                .center()
                .child(label().text("Cargando...").color(Color::WHITE.with_a(180)));
        }

        let av = loaded.unwrap();

        let ac = audio_cmd.clone();
        rect()
            .vertical()
            .expanded()
            .content(Content::Flex)
            .child(render_header(av.clone(), audio_cmd, thumbnail_src, router))
            .child(
                rect()
                    .width(Size::Fill)
                    .height(Size::flex(1.0))
                    .child(
                        VirtualScrollView::new_with_data(av.songs.clone(), move |i, songs| {
                            PlaylistSongRow::album(songs[i].clone(), i, ac.clone()).into_element()
                        })
                        .length(av.songs.len())
                        .item_size(ROW_HEIGHT),
                    ),
            )
    }
}

fn render_header(
    av: AlbumViewData,
    audio_cmd: Option<tokio::sync::mpsc::Sender<AudioCommand>>,
    thumbnail_src: Option<ImageSource>,
    router: RouterContext,
) -> impl IntoElement {
    let title = av.album.title.clone();
    let artist = av
        .album
        .artists
        .first()
        .map(|a| a.name.clone())
        .unwrap_or_default();
    let year = av.album.year.clone().unwrap_or_default();
    let song_count = av.songs.len();
    let songs = av.songs.clone();

    rect()
        .horizontal()
        .width(Size::Fill)
        .padding(Gaps::new_all(24.))
        .spacing(20.)
        .cross_align(Alignment::Center)
        // back button + thumbnail column
        .child(
            rect()
                .vertical()
                .spacing(12.)
                .cross_align(Alignment::Center)
                .child(
                    rect()
                        .padding(Gaps::new_symmetric(6., 10.))
                        .corner_radius(6.)
                        .on_press(move |_| { router.go_back(); })
                        .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                        .on_pointer_leave(|_| Cursor::set(CursorIcon::Default))
                        .child(
                            label()
                                .text("← Volver")
                                .color(Color::WHITE)
                                .font_size(13.),
                        ),
                )
                .child(
                    rect()
                        .width(Size::px(160.))
                        .height(Size::px(160.))
                        .corner_radius(8.)
                        .overflow(Overflow::Clip)
                        .background(Color::from_hex("#333333").unwrap())
                        .maybe_child(thumbnail_src.map(|src| {
                            ImageViewer::new(src)
                                .expanded()
                                .image_cover(ImageCover::Center)
                        })),
                ),
        )
        // info + play button
        .child(
            rect()
                .vertical()
                .spacing(8.)
                .width(Size::flex(1.))
                .child(
                    label()
                        .text(title)
                        .font_weight(FontWeight::BOLD)
                        .font_size(26.)
                        .max_lines(2)
                        .text_overflow(TextOverflow::Ellipsis)
                        .color(Color::WHITE),
                )
                .maybe_child((!artist.is_empty()).then_some(
                    label()
                        .text(artist)
                        .font_size(14.)
                        .color(Color::from_hex("#B3B3B3").unwrap()),
                ))
                .maybe_child((!year.is_empty()).then_some(
                    label()
                        .text(year)
                        .font_size(12.)
                        .color(Color::from_hex("#888888").unwrap()),
                ))
                .child(
                    label()
                        .text(format!("{} canciones", song_count))
                        .font_size(12.)
                        .color(Color::from_hex("#888888").unwrap()),
                )
                .child(
                    rect()
                        .rounded_full()
                        .padding(Gaps::new_all(12.))
                        .background(Color::WHITE)
                        .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                        .on_pointer_leave(|_| Cursor::set(CursorIcon::Default))
                        .on_press(move |_| {
                            let Some(tx) = audio_cmd.clone() else { return };
                            if songs.is_empty() { return; }
                            let queue: Vec<QueueSong> = songs.iter().map(|s| QueueSong {
                                video_id: s.id.clone(),
                                title: s.title.clone(),
                                artist: s.artists.first().map(|a| a.name.clone()).unwrap_or_default(),
                                album: s.album.as_ref().map(|a| a.name.clone()).unwrap_or_default(),
                                thumbnail_url: s.thumbnail.clone().unwrap_or_default(),
                                like_status: s.like_status.clone(),
                            }).collect();
                            tx.try_send(AudioCommand::PlayFromQueue {
                                songs: queue,
                                index: 0,
                                quality: AudioQuality::Medium,
                            }).ok();
                        })
                        .child(
                            SvgViewer::new(freya::icons::lucide::play())
                                .fill(Color::BLACK)
                                .color(Color::BLACK)
                                .width(Size::px(20.))
                                .height(Size::px(20.)),
                        ),
                ),
        )
}
