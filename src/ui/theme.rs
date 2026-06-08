use egui::epaint::Shadow;
use egui::style::{Selection, WidgetVisuals, Widgets};
use egui::{Color32, FontFamily, FontId, Margin, Rounding, Stroke, TextStyle, Visuals};

pub const ACCENT: Color32 = Color32::from_rgb(0xb8, 0x5c, 0x3c);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(0xec, 0xd6, 0xc6);
pub const ACCENT_INK: Color32 = Color32::from_rgb(0xf4, 0xef, 0xe4);

pub const BG: Color32 = Color32::from_rgb(0xf4, 0xef, 0xe4);
pub const BG_ELEV: Color32 = Color32::from_rgb(0xec, 0xe5, 0xd4);
pub const SURFACE: Color32 = Color32::from_rgb(0xeb, 0xe3, 0xd0);
pub const SURFACE_2: Color32 = Color32::from_rgb(0xe3, 0xd9, 0xc1);
pub const HOVER: Color32 = Color32::from_rgb(0xe6, 0xdc, 0xc4);

pub const TEXT: Color32 = Color32::from_rgb(0x1f, 0x1a, 0x13);
pub const TEXT_2: Color32 = Color32::from_rgb(0x5a, 0x53, 0x43);
pub const TEXT_3: Color32 = Color32::from_rgb(0x8a, 0x81, 0x70);

pub const BORDER: Color32 = Color32::from_rgb(0xc9, 0xbd, 0xa2);
pub const BORDER_SOFT: Color32 = Color32::from_rgb(0xdd, 0xd2, 0xb8);

pub fn serif() -> FontFamily {
    FontFamily::Name("serif".into())
}

pub fn install(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    style.visuals = claude_visuals();

    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.interact_size = egui::vec2(40.0, 28.0);
    style.spacing.window_margin = Margin::same(16.0);
    style.spacing.menu_margin = Margin::same(8.0);
    style.spacing.indent = 18.0;
    style.spacing.slider_width = 220.0;
    style.spacing.scroll.bar_width = 8.0;

    style
        .text_styles
        .insert(TextStyle::Heading, FontId::new(28.0, serif()));
    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(14.0, FontFamily::Proportional));
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(13.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(11.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(12.0, FontFamily::Monospace),
    );

    ctx.set_style(style);
}

fn claude_visuals() -> Visuals {
    let rounding = Rounding::same(6.0);
    let hairline = Stroke::new(1.0, BORDER_SOFT);
    let stronger = Stroke::new(1.0, BORDER);
    let ink_stroke = Stroke::new(1.0, TEXT);

    let noninteractive = WidgetVisuals {
        bg_fill: BG,
        weak_bg_fill: BG,
        bg_stroke: hairline,
        fg_stroke: Stroke::new(1.0, TEXT),
        rounding,
        expansion: 0.0,
    };
    let inactive = WidgetVisuals {
        bg_fill: SURFACE,
        weak_bg_fill: SURFACE,
        bg_stroke: hairline,
        fg_stroke: Stroke::new(1.0, TEXT),
        rounding,
        expansion: 0.0,
    };
    let hovered = WidgetVisuals {
        bg_fill: HOVER,
        weak_bg_fill: HOVER,
        bg_stroke: stronger,
        fg_stroke: ink_stroke,
        rounding,
        expansion: 0.0,
    };
    let active = WidgetVisuals {
        bg_fill: SURFACE_2,
        weak_bg_fill: SURFACE_2,
        bg_stroke: stronger,
        fg_stroke: ink_stroke,
        rounding,
        expansion: 0.0,
    };
    let open = WidgetVisuals {
        bg_fill: SURFACE,
        weak_bg_fill: SURFACE,
        bg_stroke: stronger,
        fg_stroke: ink_stroke,
        rounding,
        expansion: 0.0,
    };

    Visuals {
        dark_mode: false,
        override_text_color: Some(TEXT),
        widgets: Widgets {
            noninteractive,
            inactive,
            hovered,
            active,
            open,
        },
        selection: Selection {
            bg_fill: ACCENT_SOFT,
            stroke: Stroke::new(1.0, ACCENT),
        },
        hyperlink_color: ACCENT,
        faint_bg_color: BG_ELEV,
        extreme_bg_color: SURFACE,
        code_bg_color: SURFACE,
        warn_fg_color: Color32::from_rgb(0xb8, 0x85, 0x2b),
        error_fg_color: Color32::from_rgb(0xa6, 0x41, 0x33),
        window_fill: BG,
        window_stroke: hairline,
        window_rounding: Rounding::same(10.0),
        window_shadow: Shadow::NONE,
        popup_shadow: Shadow::NONE,
        panel_fill: BG,
        menu_rounding: Rounding::same(8.0),
        button_frame: true,
        collapsing_header_frame: false,
        indent_has_left_vline: false,
        striped: false,
        slider_trailing_fill: true,
        ..Visuals::light()
    }
}
