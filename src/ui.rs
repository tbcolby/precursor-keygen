//! UI rendering for Key Ceremony.

extern crate alloc;
use alloc::format;
use alloc::string::String;

use gam::*;
use graphics_server::api::GlyphStyle;
use graphics_server::{DrawStyle, PixelColor, Point, Rectangle, TextBounds};

use crate::app::*;
use crate::generators::*;

const SCREEN_W: i16 = 336;
const HEADER_H: i16 = 30;
const FOOTER_H: i16 = 46;
const LINE_H: i16 = 22;

fn draw_header(gam: &Gam, canvas: Canvas, text: &str) {
    let r = Rectangle::new(Point::new(0, 0), Point::new(SCREEN_W - 1, HEADER_H - 1));
    gam.draw_rectangle(canvas, r.style(DrawStyle::new(PixelColor::Dark, PixelColor::Dark, 0))).ok();
    let tb = TextBounds::BoundingBox(Rectangle::new(Point::new(4, 2), Point::new(SCREEN_W - 4, HEADER_H - 2)));
    gam.draw_textview(canvas, tv::TextView::new(tb, text).style(GlyphStyle::Bold).draw_border(false).invert(true)).ok();
}

fn draw_footer(gam: &Gam, canvas: Canvas, text: &str) {
    let y = 536 - FOOTER_H;
    gam.draw_line(canvas, Point::new(0, y), Point::new(SCREEN_W - 1, y), DrawStyle::new(PixelColor::Dark, PixelColor::Dark, 1)).ok();
    let tb = TextBounds::BoundingBox(Rectangle::new(Point::new(4, y + 4), Point::new(SCREEN_W - 4, 536 - 2)));
    gam.draw_textview(canvas, tv::TextView::new(tb, text).style(GlyphStyle::Small).draw_border(false)).ok();
}

fn draw_text(gam: &Gam, canvas: Canvas, x: i16, y: i16, text: &str, style: GlyphStyle) {
    let tb = TextBounds::BoundingBox(Rectangle::new(Point::new(x, y), Point::new(SCREEN_W - 4, y + LINE_H)));
    gam.draw_textview(canvas, tv::TextView::new(tb, text).style(style).draw_border(false)).ok();
}

fn draw_text_inverted(gam: &Gam, canvas: Canvas, x: i16, y: i16, w: i16, text: &str) {
    let bg = Rectangle::new(Point::new(x, y), Point::new(x + w, y + LINE_H));
    gam.draw_rectangle(canvas, bg.style(DrawStyle::new(PixelColor::Dark, PixelColor::Dark, 0))).ok();
    let tb = TextBounds::BoundingBox(Rectangle::new(Point::new(x + 2, y), Point::new(x + w - 2, y + LINE_H)));
    gam.draw_textview(canvas, tv::TextView::new(tb, text).style(GlyphStyle::Regular).draw_border(false).invert(true)).ok();
}

fn draw_wrapped(gam: &Gam, canvas: Canvas, x: i16, y: i16, text: &str, max_chars: usize, style: GlyphStyle) -> i16 {
    let mut cy = y;
    let chars: alloc::vec::Vec<char> = text.chars().collect();
    let mut pos = 0;
    while pos < chars.len() {
        let end = (pos + max_chars).min(chars.len());
        let line: String = chars[pos..end].iter().collect();
        draw_text(gam, canvas, x, cy, &line, style);
        cy += LINE_H;
        pos = end;
        if cy > 536 - FOOTER_H - LINE_H { break; }
    }
    cy
}

pub fn draw(app: &KeygenApp, gam: &Gam, canvas: Canvas) {
    gam.draw_rectangle(canvas, Rectangle::new(Point::new(0, 0), Point::new(SCREEN_W - 1, 535)).style(DrawStyle::new(PixelColor::Light, PixelColor::Light, 0))).ok();

    match app.state {
        AppState::TypeSelect => draw_type_select(app, gam, canvas),
        AppState::Configure => draw_configure(app, gam, canvas),
        AppState::Result => draw_result(app, gam, canvas),
        AppState::Saved => draw_saved(app, gam, canvas),
    }

    gam.redraw().ok();
}

fn draw_type_select(app: &KeygenApp, gam: &Gam, canvas: Canvas) {
    draw_header(gam, canvas, "Key Ceremony");

    let mut y = HEADER_H + 8;
    draw_text(gam, canvas, 8, y, "TRNG-powered key generation", GlyphStyle::Small);
    y += 18;

    let types = GenType::all();
    for (i, gt) in types.iter().enumerate() {
        let line = format!("{}", gt.label());
        if i == app.type_cursor {
            draw_text_inverted(gam, canvas, 8, y, SCREEN_W - 16, &line);
            y += LINE_H + 2;
            draw_text(gam, canvas, 20, y, gt.description(), GlyphStyle::Small);
        } else {
            draw_text(gam, canvas, 12, y, &line, GlyphStyle::Regular);
        }
        y += LINE_H + 4;
    }

    draw_footer(gam, canvas, "Up/Down=Select  Enter=Configure  S)aved  Menu=Quit");
}

fn draw_configure(app: &KeygenApp, gam: &Gam, canvas: Canvas) {
    draw_header(gam, canvas, &format!("Configure: {}", app.selected_type.label()));

    let mut y = HEADER_H + 20;

    let unit = app.selected_type.length_unit();
    let display = format!("Length: < {} {} >", app.length, unit);
    draw_text_inverted(gam, canvas, 20, y, SCREEN_W - 40, &display);
    y += LINE_H + 10;

    let range = format!(
        "Range: {}-{} {}",
        app.selected_type.min_length(),
        app.selected_type.max_length(),
        unit
    );
    draw_text(gam, canvas, 20, y, &range, GlyphStyle::Small);
    y += 18;

    let entropy = entropy_display(app.selected_type, app.length);
    let ent_label = format!("~{} bits of entropy", entropy);
    draw_text(gam, canvas, 20, y, &ent_label, GlyphStyle::Small);
    y += 30;

    // Strength indicator
    let strength = if entropy >= 128 {
        "EXCELLENT (128+ bits)"
    } else if entropy >= 80 {
        "STRONG (80+ bits)"
    } else if entropy >= 60 {
        "GOOD (60+ bits)"
    } else if entropy >= 40 {
        "MODERATE (40+ bits)"
    } else {
        "WEAK (< 40 bits)"
    };
    draw_text(gam, canvas, 20, y, &format!("Strength: {}", strength), GlyphStyle::Regular);

    draw_footer(gam, canvas, "</>=Length  Enter/Space=Generate  Menu=Back");
}

fn draw_result(app: &KeygenApp, gam: &Gam, canvas: Canvas) {
    draw_header(gam, canvas, &format!("{} — Generated", app.selected_type.label()));

    let mut y = HEADER_H + 10;

    // Entropy badge
    let ent_label = format!("~{} bits entropy", app.result_entropy);
    draw_text(gam, canvas, 8, y, &ent_label, GlyphStyle::Small);
    y += 18;

    // Separator
    gam.draw_line(canvas, Point::new(8, y), Point::new(SCREEN_W - 8, y), DrawStyle::new(PixelColor::Dark, PixelColor::Dark, 1)).ok();
    y += 8;

    // The generated value — large and clear
    y = draw_wrapped(gam, canvas, 8, y, &app.result_value, 36, GlyphStyle::Regular);
    y += 10;

    // Separator
    gam.draw_line(canvas, Point::new(8, y), Point::new(SCREEN_W - 8, y), DrawStyle::new(PixelColor::Dark, PixelColor::Dark, 1)).ok();
    y += 8;

    draw_text(gam, canvas, 8, y, "R/Space = Regenerate", GlyphStyle::Small);
    y += 16;
    draw_text(gam, canvas, 8, y, "S = Save to vault", GlyphStyle::Small);

    draw_footer(gam, canvas, "R)egenerate  S)ave  Menu=Back");
}

fn draw_saved(app: &KeygenApp, gam: &Gam, canvas: Canvas) {
    let header = format!("Saved Keys ({})", app.saved.len());
    draw_header(gam, canvas, &header);

    let mut y = HEADER_H + 4;
    if app.saved.is_empty() {
        draw_text(gam, canvas, 8, y, "No saved keys", GlyphStyle::Regular);
    } else {
        for (i, key) in app.saved.iter().enumerate() {
            let preview = if key.value.len() > 20 {
                format!("{}...", &key.value[..20])
            } else {
                key.value.clone()
            };
            let line = format!("[{}] {}", key.gen_type, preview);
            if i == app.saved_cursor {
                draw_text_inverted(gam, canvas, 4, y, SCREEN_W - 8, &line);
                y += LINE_H + 2;
                let detail = format!("{} {} ~{}b", key.length, key.gen_type, key.entropy_bits);
                draw_text(gam, canvas, 12, y, &detail, GlyphStyle::Small);
            } else {
                draw_text(gam, canvas, 8, y, &line, GlyphStyle::Regular);
            }
            y += LINE_H + 4;
            if y > 536 - FOOTER_H - LINE_H { break; }
        }
    }

    draw_footer(gam, canvas, "Up/Down=Navigate  D)elete  Menu=Back");
}
