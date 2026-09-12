//! GPU-facing scene builders for the debug companion panel.

use super::{
    Snapshot, append_backspace_icon_quads, append_chevron_icon_quads, append_close_icon_quads,
    append_gear_icon_quads, append_keyboard_icon_quads, append_mic_icon_quads,
    append_next_icon_quads, append_pen_icon_quads, append_refresh_icon_quads,
    append_rounded_rect_quads, append_seed_icon_quads, append_soft_card_quads,
    append_suzaku_bird_icon_quads, append_trash_icon_quads, append_undo_icon_quads,
    measure_text_prefix_width, text_char_advance, text_char_width, text_glyph_advance,
    text_space_advance,
};

mod brand;
mod font_metrics;
mod glyphs;
mod panel_scene;
mod scene_basics;
mod settings_scene;
mod shapes;
mod text_helpers;
mod theme_metrics;
mod types;

pub use self::brand::{suzaku_badge_quads, suzaku_icon_argb, theme_badge_quads, theme_icon_argb};
pub(crate) use self::font_metrics::measured_char_width;
pub use self::font_metrics::{FontLayoutMetrics, with_font_metrics};
pub use self::glyphs::glyph_bitmap;
use self::text_helpers::*;
pub use self::theme_metrics::srgb_color;
use self::theme_metrics::{PanelSceneMetrics, PanelTheme};
pub use types::*;
