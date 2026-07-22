use bytes::Bytes;
use freya::icons::lucide::{
    disc_album, download, library, list_minus, list_music, list_plus, mic_vocal, pin, radio,
    share_2, skip_forward,
};
use freya::prelude::*;
use freya::router::RouterContext;

use crate::app::Route;

/// A single row inside a song context menu: icon on the left, label on the right.
#[derive(Clone, PartialEq)]
pub struct SongMenuEntry {
    icon: Bytes,
    text: &'static str,
    on_press: Option<EventHandler<Event<PressEventData>>>,
}

impl SongMenuEntry {
    pub fn new(icon: Bytes, text: &'static str) -> Self {
        Self { icon, text, on_press: None }
    }

    pub fn on_press(mut self, f: impl Into<EventHandler<Event<PressEventData>>>) -> Self {
        self.on_press = Some(f.into());
        self
    }
}

impl Component for SongMenuEntry {
    fn render(&self) -> impl IntoElement {
        let icon = self.icon.clone();
        let text = self.text;
        let on_press = self.on_press.clone();

        MenuButton::new()
            .map(on_press, |el, handler| {
                el.on_press(move |e: Event<PressEventData>| {
                    handler.call(e);
                    ContextMenu::close();
                })
            })
            .child(
                rect()
                    .horizontal()
                    .spacing(10.)
                    .cross_align(Alignment::Center)
                    .child(
                        SvgViewer::new(icon)
                            .color(Color::from_hex("#B3B3B3").unwrap())
                            .fill(Color::TRANSPARENT)
                            .width(Size::px(16.))
                            .height(Size::px(16.)),
                    )
                    .child(label().text(text).font_size(13.)),
            )
    }
}

/// Builds the context `Menu` for a song row.
///
/// `router` must be captured at component render time (not inside event handlers).
pub fn song_context_menu(
    artist_id: Option<String>,
    album_id: Option<String>,
    router: RouterContext,
) -> Menu {
    Menu::new()
        .child(SongMenuEntry::new(radio(), "Iniciar mix"))
        .child(SongMenuEntry::new(skip_forward(), "Reproducir a continuación"))
        .child(SongMenuEntry::new(list_plus(), "Añadir a la cola"))
        .child(SongMenuEntry::new(library(), "Guardar en la biblioteca"))
        .child(SongMenuEntry::new(download(), "Descargar"))
        .child(SongMenuEntry::new(list_music(), "Añadir a lista de reproducción"))
        .child(SongMenuEntry::new(list_minus(), "Quitar de la lista de reproducción"))
        .child({
            let router = router.clone();
            let id = album_id.clone();
            SongMenuEntry::new(disc_album(), "Ir al álbum").on_press(move |_| {
                if let Some(ref album_id) = id {
                    router.push(Route::Album { id: album_id.clone() }).ok();
                }
            })
        })
        .child({
            let id = artist_id.clone();
            SongMenuEntry::new(mic_vocal(), "Ir a artista").on_press(move |_| {
                if let Some(ref artist_id) = id {
                    router.push(Route::Artist { id: artist_id.clone() }).ok();
                }
            })
        })
        .child(SongMenuEntry::new(share_2(), "Compartir"))
        .child(SongMenuEntry::new(pin(), "Fijar en Vuelve a escucharlo"))
}
