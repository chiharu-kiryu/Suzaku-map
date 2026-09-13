use super::*;
use crate::languages::translation::{MAX_TRANSLATION_INPUT_CHARS, TranslationLanguage};

#[derive(Default)]
pub(super) struct TranslationToolScene {
    ui_language: crate::ui::UiLanguage,
    pub quads: Vec<CandidateQuad>,
    pub text_quads: Vec<CandidateQuad>,
    pub atlas_glyphs: Vec<AtlasGlyph>,
    pub text_sections: Vec<TextSection>,
    pub interactive_targets: Vec<InteractiveTarget>,
}

struct Layout {
    px: f32,
    row: f32,
    gap: f32,
    columns: usize,
    selectors_height: f32,
    label_width: f32,
}

struct ButtonState {
    selected: bool,
    enabled: bool,
}

impl Layout {
    fn new(width: f32, scale: f32, text: DisplayTextScale, ui: crate::ui::UiLanguage) -> Self {
        let px = match text {
            DisplayTextScale::Small => 1.9,
            DisplayTextScale::Medium => 2.1,
            DisplayTextScale::Large => 2.55,
        } * scale;
        let row = px * 7.0 + 10.0 * scale;
        let gap = 5.0 * scale;
        let measure =
            |label: &str| measure_text_prefix_width(label, label.chars().count(), px, -0.06);
        let label_width = ["From", "To"]
            .map(|key| measure(ui.tr(key)))
            .into_iter()
            .fold(0.0_f32, f32::max)
            .ceil()
            + 8.0 * scale;
        let chip_width = (measure(ui.tr("Auto")).ceil() + 12.0 * scale).max(48.0 * scale);
        let columns = (((width - 24.0 * scale - label_width) / (chip_width + gap)).floor()
            as usize)
            .clamp(1, 9);
        let selectors_height =
            (9usize.div_ceil(columns) + 8usize.div_ceil(columns)) as f32 * (row + gap);
        Self {
            px,
            row,
            gap,
            columns,
            selectors_height,
            label_width,
        }
    }
}

pub(super) fn preferred_translation_height(
    width: f32,
    scale: f32,
    text: DisplayTextScale,
    ui: crate::ui::UiLanguage,
) -> f32 {
    let layout = Layout::new(width, scale, text, ui);
    46.0 * scale + layout.selectors_height + 112.0 * scale + layout.row + 16.0 * scale
}

impl TranslationToolScene {
    fn text(
        &mut self,
        text: &str,
        rect: [f32; 4],
        px: f32,
        color: [f32; 4],
        role: TextRole,
        centered: bool,
    ) {
        let layout = TextBlock {
            text: if role == TextRole::TranslationText {
                text.into()
            } else {
                self.ui_language.tr(text).into()
            },
            origin: [0.0; 2],
            max_width: rect[2],
            pixel_size: px,
            letter_spacing: -0.06,
            line_gap: 0.0,
            max_lines: 1,
            color,
            align: if centered {
                TextAlign::Center
            } else {
                TextAlign::Left
            },
            role,
        }
        .layout_in_rect(rect, [2.0, 2.0]);
        self.text_quads.extend(layout.quads.iter().copied());
        self.atlas_glyphs
            .extend(layout.atlas_glyphs.iter().cloned());
        self.text_sections.push(TextSection {
            role,
            layouts: vec![layout],
        });
    }
    fn button(
        &mut self,
        chrome: &PanelChromeState,
        kind: InteractionKind,
        label: &str,
        rect: [f32; 4],
        px: f32,
        state: ButtonState,
    ) {
        let ButtonState { selected, enabled } = state;
        let theme = PanelTheme::for_preset(chrome.theme_preset);
        let label = self.ui_language.tr(label);
        let active = selected || chrome.pressed_interaction == Some(kind);
        let hovered = chrome.hovered_interaction == Some(kind);
        let radius = 7.0;
        self.quads.push(CandidateQuad::rounded(
            rect,
            if active {
                theme.accent_soft
            } else if hovered {
                theme.surface_alt
            } else {
                theme.surface
            },
            radius,
        ));
        self.quads.push(CandidateQuad::outline(
            rect,
            if active {
                theme.accent
            } else {
                theme.shell_border
            },
            radius,
            0.8,
        ));
        self.text(
            label,
            rect,
            // Localized actions are longer than the English labels; retain their
            // full wording with a small bounded size adjustment on narrow windows.
            px * ((rect[2] - 8.0).max(1.0)
                / measure_text_prefix_width(label, label.chars().count(), px, -0.06).max(1.0))
            .clamp(0.82, 1.0),
            if !enabled {
                theme.text_muted
            } else if active {
                theme.accent_text
            } else {
                theme.text_primary
            },
            TextRole::TranslationButton,
            true,
        );
        if enabled {
            self.interactive_targets
                .push(InteractiveTarget { kind, rect });
        }
    }
}

fn translation_lines(text: &str, width: f32, px: f32) -> Vec<String> {
    text.split('\n')
        .flat_map(|paragraph| {
            if paragraph.is_empty() {
                return vec![String::new()];
            }
            TextBlock {
                text: paragraph.into(),
                origin: [0.0; 2],
                max_width: width,
                pixel_size: px,
                letter_spacing: -0.06,
                line_gap: 0.0,
                max_lines: 2000,
                color: [1.0; 4],
                align: TextAlign::Left,
                role: TextRole::TranslationText,
            }
            .layout()
            .lines
        })
        .collect()
}

pub(super) fn build_translation_tool(
    chrome: &PanelChromeState,
    rect: [f32; 4],
    scale: f32,
) -> TranslationToolScene {
    let mut scene = TranslationToolScene {
        ui_language: chrome.ui_language,
        ..Default::default()
    };
    let theme = PanelTheme::for_preset(chrome.theme_preset);
    let layout = Layout::new(rect[2], scale, chrome.text_scale, chrome.ui_language);
    let x = rect[0] + 12.0 * scale;
    let width = (rect[2] - 24.0 * scale).max(1.0);
    let mut y = rect[1] + 14.0 * scale;
    scene.text(
        "Translation",
        [x, y, width * 0.55, 24.0 * scale],
        layout.px * 1.1,
        theme.text_primary,
        TextRole::TranslationLabel,
        false,
    );
    scene.text(
        if chrome.translation.phase == TranslationPhase::Ready {
            if chrome.translation.cloud {
                "Cloud · Review"
            } else {
                "Local · Review"
            }
        } else if chrome.translation.cloud {
            "Cloud model"
        } else {
            "Local model"
        },
        [x + width * 0.55, y, width * 0.45, 24.0 * scale],
        layout.px * 0.86,
        theme.text_secondary,
        TextRole::TranslationLabel,
        true,
    );
    y += 30.0 * scale;
    for source_row in [true, false] {
        let choices: Vec<_> = if source_row {
            std::iter::once(None)
                .chain(TranslationLanguage::ALL.map(Some))
                .collect()
        } else {
            TranslationLanguage::ALL.map(Some).to_vec()
        };
        scene.text(
            if source_row { "From" } else { "To" },
            [x, y, layout.label_width - 2.0 * scale, layout.row],
            layout.px,
            theme.text_secondary,
            TextRole::TranslationLabel,
            false,
        );
        let area_width = (width - layout.label_width).max(1.0);
        let columns = layout.columns.min(choices.len());
        let button_width =
            (area_width - layout.gap * (columns.saturating_sub(1)) as f32) / columns as f32;
        for (index, language) in choices.iter().copied().enumerate() {
            let button_rect = [
                x + layout.label_width + (index % columns) as f32 * (button_width + layout.gap),
                y + (index / columns) as f32 * (layout.row + layout.gap),
                button_width,
                layout.row,
            ];
            let (kind, selected) = if source_row {
                (
                    InteractionKind::SetTranslationSource(language),
                    chrome.translation.source == language,
                )
            } else {
                (
                    InteractionKind::SetTranslationTarget(language.unwrap()),
                    chrome.translation.target == language.unwrap(),
                )
            };
            scene.button(
                chrome,
                kind,
                language.map(|l| l.short_label()).unwrap_or("Auto"),
                button_rect,
                layout.px,
                ButtonState {
                    selected,
                    enabled: true,
                },
            );
        }
        y += choices.len().div_ceil(columns) as f32 * (layout.row + layout.gap);
    }
    y += 4.0 * scale;
    let footer_y = (rect[1] + rect[3] - layout.row - 12.0 * scale).max(y + 40.0 * scale);
    let result_rect = [x, y, width, (footer_y - y - 8.0 * scale).max(1.0)];
    scene.quads.push(CandidateQuad::rounded(
        result_rect,
        theme.surface,
        8.0 * scale,
    ));
    scene.quads.push(CandidateQuad::outline(
        result_rect,
        theme.shell_border,
        8.0 * scale,
        0.8,
    ));
    let input_length = chrome.seed_text.chars().count();
    let length_warning = format!(
        "Draft has {input_length} characters. Translate up to 1,000 at a time; nothing has been sent."
    );
    let copy = match chrome.translation.phase {
        TranslationPhase::Pending => {
            "Translating… You can keep editing. Cancel discards the result, but cannot recall text already sent."
        }
        TranslationPhase::Ready => &chrome.translation.text,
        TranslationPhase::Failed => &chrome.translation.message,
        TranslationPhase::Idle if input_length > MAX_TRANSLATION_INPUT_CHARS => &length_warning,
        TranslationPhase::Idle => {
            "Type a draft above, choose languages, then Translate. Up to 1,000 characters. Nothing is sent or inserted automatically."
        }
    };
    let localized_copy = if chrome.translation.phase == TranslationPhase::Ready {
        std::borrow::Cow::Borrowed(copy)
    } else {
        chrome.ui_language.message(copy)
    };
    let copy = localized_copy.as_ref();
    let text_px = layout.px
        * if chrome.translation.phase == TranslationPhase::Ready {
            1.05
        } else {
            0.92
        };
    let line_height = text_px * 7.0 + 4.0 * scale;
    let per_page = (((result_rect[3] - 16.0 * scale) / line_height).floor() as usize).max(1);
    let lines = translation_lines(copy, (width - 20.0 * scale).max(1.0), text_px);
    let pages = lines.len().div_ceil(per_page).max(1);
    let page = chrome.translation.page.min(pages - 1);
    for (index, line) in lines
        .iter()
        .skip(page * per_page)
        .take(per_page)
        .enumerate()
    {
        scene.text(
            line,
            [
                x + 8.0 * scale,
                y + 8.0 * scale + index as f32 * line_height,
                width - 16.0 * scale,
                line_height,
            ],
            text_px,
            if chrome.translation.phase == TranslationPhase::Failed {
                theme.accent_text
            } else {
                theme.text_primary
            },
            TextRole::TranslationText,
            false,
        );
    }
    let pending = chrome.translation.phase == TranslationPhase::Pending;
    let valid = !chrome.seed_text.trim().is_empty()
        && chrome.seed_text.chars().count() <= MAX_TRANSLATION_INPUT_CHARS;
    let action_width = ((width - 110.0 * scale) * 0.5).min(160.0 * scale);
    scene.button(
        chrome,
        if pending {
            InteractionKind::CancelTranslation
        } else {
            InteractionKind::TranslateText
        },
        if pending { "Cancel" } else { "Translate" },
        [x, footer_y, action_width, layout.row],
        layout.px,
        ButtonState {
            selected: true,
            enabled: pending || valid,
        },
    );
    scene.button(
        chrome,
        InteractionKind::ApplyTranslation,
        "Use draft",
        [
            x + action_width + 6.0 * scale,
            footer_y,
            action_width,
            layout.row,
        ],
        layout.px,
        ButtonState {
            selected: false,
            enabled: chrome.translation.phase == TranslationPhase::Ready
                && !chrome.translation.text.is_empty(),
        },
    );
    let nav_x = x + width - 98.0 * scale;
    scene.button(
        chrome,
        InteractionKind::TranslationPage(page.saturating_sub(1)),
        "<",
        [nav_x, footer_y, 26.0 * scale, layout.row],
        layout.px,
        ButtonState {
            selected: false,
            enabled: page > 0,
        },
    );
    scene.text(
        &format!("{}/{}", page + 1, pages),
        [nav_x + 28.0 * scale, footer_y, 42.0 * scale, layout.row],
        layout.px * 0.85,
        theme.text_secondary,
        TextRole::TranslationLabel,
        true,
    );
    scene.button(
        chrome,
        InteractionKind::TranslationPage(page + 1),
        ">",
        [nav_x + 72.0 * scale, footer_y, 26.0 * scale, layout.row],
        layout.px,
        ButtonState {
            selected: false,
            enabled: page + 1 < pages,
        },
    );
    // A compositor may constrain a small window; never draw or click outside the drawer.
    for quad in scene.quads.iter_mut().chain(scene.text_quads.iter_mut()) {
        quad.clip_rect = Some(rect);
    }
    for glyph in &mut scene.atlas_glyphs {
        glyph.clip_rect = Some(rect);
    }
    for target in &mut scene.interactive_targets {
        target.rect = intersect_rect(target.rect, rect);
    }
    scene
        .interactive_targets
        .retain(|target| target.rect[2] > 0.0 && target.rect[3] > 0.0);
    scene
}
