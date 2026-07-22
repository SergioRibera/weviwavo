use freya::animation::*;
use freya::prelude::*;

/// Thin 3-px track always present at the top of the layout.
/// When `active`, a white sweep bar animates across it.
#[derive(PartialEq, Default)]
pub struct LoadingBar {
    pub active: bool,
}

impl Component for LoadingBar {
    fn render(&self) -> impl IntoElement {
        let screen_w = Platform::get().root_size.read().width;
        let bar_w = 280.0f32;
        let active = self.active;

        let mut anim = use_animation(move |_| {
            AnimNum::new(-bar_w, screen_w + bar_w)
                .function(Function::Sine)
                .ease(Ease::InOut)
                .time(900)
        });

        let mut started = use_state(|| false);
        if active && !*started.read() {
            started.set(true);
            anim.start();
        }
        if !active && *started.read() {
            started.set(false);
            anim.reset();
        }

        let x = anim.read().value();

        rect()
            .width(Size::Fill)
            .height(Size::px(3.))
            .overflow(Overflow::Clip)
            .maybe_child(active.then(|| {
                rect()
                    .width(Size::px(bar_w))
                    .height(Size::Fill)
                    .background(Color::WHITE)
                    .position(Position::new_absolute().left(x))
            }))
    }
}
