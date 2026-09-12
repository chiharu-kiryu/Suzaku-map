//! Scoped font advances for scene layout. Each window supplies its own cached atlas metrics;
//! no global font selection, filesystem lookup, or rasterization occurs in the text measurer.
use std::{cell::RefCell, collections::HashMap, sync::Arc};

pub type FontLayoutMetrics = Arc<HashMap<char, f32>>;

thread_local! {
    static ACTIVE: RefCell<Option<FontLayoutMetrics>> = const { RefCell::new(None) };
}

pub fn with_font_metrics<T>(metrics: FontLayoutMetrics, build: impl FnOnce() -> T) -> T {
    struct Restore(Option<FontLayoutMetrics>);
    impl Drop for Restore {
        fn drop(&mut self) {
            ACTIVE.with(|active| {
                active.replace(self.0.take());
            });
        }
    }
    let _restore = Restore(ACTIVE.with(|active| active.replace(Some(metrics))));
    build()
}

pub(crate) fn measured_char_width(ch: char, pixels: f32) -> Option<f32> {
    ACTIVE
        .with(|active| active.borrow().as_ref()?.get(&ch).copied())
        .filter(|width| width.is_finite() && *width > 0.0 && *width <= 14.0)
        .map(|width| width * pixels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ime::gpu::{TextAlign, TextBlock, TextRole};
    use crate::ime::{measure_text_prefix_width, text_char_advance};

    #[test]
    fn proportional_metrics_align_wrapping_glyphs_and_caret_and_do_not_leak() {
        let metrics = Arc::new(HashMap::from([('i', 1.6), ('W', 5.8)]));
        assert_eq!(measured_char_width('i', 1.0), None);
        with_font_metrics(metrics.clone(), || {
            let block = TextBlock {
                text: "WiWi".into(),
                origin: [0.0; 2],
                max_width: 200.0,
                pixel_size: 3.0,
                letter_spacing: 0.0,
                line_gap: 3.0,
                max_lines: 1,
                color: [1.0; 4],
                align: TextAlign::Left,
                role: TextRole::InputValue,
            };
            let layout = block.layout();
            assert!(layout.atlas_glyphs[0].rect[2] > layout.atlas_glyphs[1].rect[2] * 3.0);
            let last = layout.atlas_glyphs.last().unwrap();
            let caret = measure_text_prefix_width("WiWi", 4, 3.0, 0.0);
            assert!((caret - last.rect[0] - text_char_advance('i', 3.0, 0.0)).abs() < 0.001);
            with_font_metrics(Arc::new(HashMap::from([('i', 4.0)])), || {
                assert_eq!(measured_char_width('i', 1.0), Some(4.0));
            });
            assert_eq!(measured_char_width('i', 1.0), Some(1.6));
        });
        assert_eq!(measured_char_width('i', 1.0), None);
    }
}
