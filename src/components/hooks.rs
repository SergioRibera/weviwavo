use freya::animation::*;
use freya::prelude::*;

/// Returns the current responsive max-width percentage (70% ≤1024px, 100% >1024px),
/// animated with the same curve used by Section.
pub fn use_responsive_width() -> f32 {
    let platform = Platform::get();

    let mut anim = use_animation(|_| {
        AnimNum::new(70., 100.)
            .function(Function::Sine)
            .ease(Ease::InOut)
            .time(100)
    });

    let mut prev_small = use_state(|| platform.root_size.read().width <= 1024.);

    let is_small = platform.root_size.read().width <= 1024.;
    if is_small != *prev_small.read() {
        if is_small { anim.start() } else { anim.reverse() }
        prev_small.set(is_small);
    }

    anim.read().value()
}
