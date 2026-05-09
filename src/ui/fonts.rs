use egui::{FontData, FontDefinitions, FontFamily};

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let candidates: &[&str] = &[
        r"C:\Windows\Fonts\segoeui.ttf",
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\meiryo.ttc",
        r"C:\Windows\Fonts\malgun.ttf",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let key = std::path::Path::new(path)
                .file_stem().and_then(|s| s.to_str()).unwrap_or("sys").to_string();
            fonts.font_data.insert(key.clone(), FontData::from_owned(bytes));
            fonts.families.entry(FontFamily::Proportional).or_default().push(key.clone());
            fonts.families.entry(FontFamily::Monospace).or_default().push(key);
        }
    }
    ctx.set_fonts(fonts);
}
