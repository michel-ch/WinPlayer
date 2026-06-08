use egui::{FontData, FontDefinitions, FontFamily};

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    // Keep the candidate list tight — `msyh.ttc` / `meiryo.ttc` are ~20 MB
    // each and cost startup time + atlas memory before any CJK is needed.
    let sans_candidates: &[&str] = &[r"C:\Windows\Fonts\segoeui.ttf"];
    for path in sans_candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let key = std::path::Path::new(path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("sys")
                .to_string();
            fonts
                .font_data
                .insert(key.clone(), FontData::from_owned(bytes));
            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .push(key.clone());
            fonts
                .families
                .entry(FontFamily::Monospace)
                .or_default()
                .push(key);
        }
    }

    let serif_candidates: &[&str] = &[
        r"C:\Windows\Fonts\georgia.ttf",
        r"C:\Windows\Fonts\constan.ttf",
        r"C:\Windows\Fonts\times.ttf",
    ];
    for path in serif_candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let key = std::path::Path::new(path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("serif")
                .to_string();
            fonts
                .font_data
                .insert(key.clone(), FontData::from_owned(bytes));
            fonts
                .families
                .entry(FontFamily::Name("serif".into()))
                .or_default()
                .push(key);
        }
    }

    let serif_italic_candidates: &[&str] = &[
        r"C:\Windows\Fonts\georgiai.ttf",
        r"C:\Windows\Fonts\constani.ttf",
        r"C:\Windows\Fonts\timesi.ttf",
    ];
    for path in serif_italic_candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let key = std::path::Path::new(path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("serif_italic")
                .to_string();
            fonts
                .font_data
                .insert(key.clone(), FontData::from_owned(bytes));
            fonts
                .families
                .entry(FontFamily::Name("serif_italic".into()))
                .or_default()
                .push(key);
        }
    }

    if fonts
        .families
        .get(&FontFamily::Name("serif".into()))
        .map(|v| v.is_empty())
        .unwrap_or(true)
    {
        if let Some(proportional) = fonts.families.get(&FontFamily::Proportional).cloned() {
            fonts
                .families
                .insert(FontFamily::Name("serif".into()), proportional);
        }
    }

    ctx.set_fonts(fonts);
}
