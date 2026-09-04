//! GPU-facing scene builders for the debug companion panel.

use super::{
    Snapshot, append_backspace_icon_quads, append_chevron_icon_quads, append_gear_icon_quads,
    append_keyboard_icon_quads, append_mic_icon_quads, append_next_icon_quads,
    append_pen_icon_quads, append_refresh_icon_quads, append_rounded_rect_quads,
    append_seed_icon_quads, append_soft_card_quads, append_suzaku_bird_icon_quads,
    append_trash_icon_quads, append_undo_icon_quads, measure_text_prefix_width, text_glyph_advance,
    text_space_advance,
};

mod glyphs;
mod panel_scene;
mod scene_basics;
mod settings_scene;
mod text_helpers;
mod theme_metrics;
mod types;

pub use self::glyphs::glyph_bitmap;
use self::text_helpers::*;
use self::theme_metrics::*;
pub use types::*;
