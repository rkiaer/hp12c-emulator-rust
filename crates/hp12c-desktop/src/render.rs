//! Desenho vetorial da calculadora a partir do skin (milímetros → pontos da tela).

use crate::fonts;
use eframe::egui::{
    self, epaint::Mesh, text::LayoutJob, Align, Align2, Color32, CornerRadius, FontFamily, FontId, Painter, Pos2,
    Shape, Stroke, StrokeKind, TextFormat, Vec2,
};
use hp12c_core::{DisplayState, Key, Punct};
use hp12c_skin::{seg7, KeyGeometry, KeyStyle, Label, Mark, Rect, Rgba, RunKind, Skin};

/// Conversão de milímetros do skin para pontos da tela.
#[derive(Clone, Copy)]
pub struct Transform {
    pub origin: Pos2,
    /// Pontos por milímetro.
    pub scale: f32,
}

impl Transform {
    pub fn pos(&self, x: f32, y: f32) -> Pos2 {
        Pos2::new(self.origin.x + x * self.scale, self.origin.y + y * self.scale)
    }

    pub fn rect(&self, r: Rect) -> egui::Rect {
        egui::Rect::from_min_size(self.pos(r.x, r.y), Vec2::new(r.w, r.h) * self.scale)
    }

    pub fn len(&self, mm: f32) -> f32 {
        mm * self.scale
    }

    /// Ponto da tela → milímetros do skin.
    pub fn to_mm(self, p: Pos2) -> (f32, f32) {
        ((p.x - self.origin.x) / self.scale, (p.y - self.origin.y) / self.scale)
    }
}

fn color(c: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c.0, c.1, c.2, c.3)
}

fn radius(points: f32) -> CornerRadius {
    CornerRadius::same(points.round().clamp(0.0, 255.0) as u8)
}

/// Desenha tudo. `pressed` diz se uma tecla está afundada (mouse ou teclado).
pub fn draw(
    painter: &Painter,
    tr: Transform,
    skin: &Skin,
    display: &DisplayState,
    now: f64,
    pressed: &dyn Fn(Key) -> bool,
) {
    draw_body(painter, tr, skin);
    // "running" pisca: some em metade do tempo.
    let visible = !display.blinking || (now * 2.5) as i64 % 2 == 0;
    draw_lcd(painter, tr, skin, display, visible);
    draw_badge(painter, tr, skin);
    draw_brackets(painter, tr, skin);
    for key in &skin.keys {
        draw_key(painter, tr, skin, key, pressed(key.key));
    }
}

fn draw_body(painter: &Painter, tr: Transform, skin: &Skin) {
    let c = &skin.colors;
    let body = Rect { x: 0.0, y: 0.0, w: skin.width, h: skin.height };
    painter.rect_filled(tr.rect(body), radius(tr.len(skin.corner_radius)), color(c.body));

    // Faixa metálica superior (alumínio escovado).
    let inset = 1.0;
    let strip = Rect { x: inset, y: inset, w: skin.width - 2.0 * inset, h: skin.metal_strip_height - inset };
    gradient_rounded_rect(painter, tr.rect(strip), tr.len(skin.corner_radius - inset), 0.0, color(c.metal_top), color(c.metal_bottom));
    brushed_lines(painter, tr, strip, color(c.metal_line));

    // Painel do teclado.
    let panel = Rect {
        x: inset,
        y: skin.metal_strip_height,
        w: skin.width - 2.0 * inset,
        h: skin.height - skin.metal_strip_height - inset,
    };
    let r = tr.len(skin.corner_radius - inset);
    painter.rect_filled(
        tr.rect(panel),
        CornerRadius { nw: 0, ne: 0, sw: r as u8, se: r as u8 },
        color(c.panel),
    );
    painter.line_segment(
        [tr.pos(panel.x, panel.y), tr.pos(panel.x + panel.w, panel.y)],
        Stroke::new(tr.len(0.25), color(c.panel_edge)),
    );
}

/// Linhas finas horizontais com intensidade variável imitando metal escovado.
fn brushed_lines(painter: &Painter, tr: Transform, strip: Rect, line: Color32) {
    let step = 0.22;
    let mut y = strip.y + 1.2;
    let mut seed: u32 = 12345;
    while y < strip.y + strip.h - 0.6 {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        let alpha = 18 + (seed >> 16) % 40;
        let stroke = Stroke::new(tr.len(0.06), line.gamma_multiply(alpha as f32 / 255.0 * 2.0));
        painter.line_segment([tr.pos(strip.x + 1.0, y), tr.pos(strip.x + strip.w - 1.0, y)], stroke);
        y += step;
    }
}

/// Aviso curto (ex.: "Programa copiado") sobre a borda inferior da calculadora.
pub fn draw_notice(painter: &Painter, tr: Transform, skin: &Skin, text: &str) {
    let font = FontId::new(tr.len(2.2), FontFamily::Name(fonts::LABEL.into()));
    let center = tr.pos(skin.width / 2.0, skin.height - 2.0);
    let galley = painter.layout_no_wrap(text.to_string(), font, Color32::WHITE);
    let rect = egui::Rect::from_center_size(center, galley.size() + Vec2::new(tr.len(3.0), tr.len(1.0)));
    painter.rect_filled(rect, radius(tr.len(1.0)), Color32::from_black_alpha(210));
    painter.galley(rect.center() - galley.size() / 2.0, galley, Color32::WHITE);
}

fn draw_lcd(painter: &Painter, tr: Transform, skin: &Skin, display: &DisplayState, visible: bool) {
    let c = &skin.colors;
    let lcd = &skin.lcd;
    painter.rect(
        tr.rect(lcd.bezel()),
        radius(tr.len(1.2)),
        color(c.bezel),
        Stroke::new(tr.len(0.3), color(c.bezel_edge)),
        StrokeKind::Inside,
    );
    let glass = lcd.glass();
    gradient_rounded_rect(painter, tr.rect(glass), tr.len(0.6), tr.len(0.6), color(c.lcd_shadow), color(c.lcd));
    // Sombra interna no topo e à esquerda.
    let shadow = color(c.lcd_shadow);
    painter.rect_filled(tr.rect(Rect { h: 0.7, ..glass }), CornerRadius::ZERO, shadow);
    painter.rect_filled(tr.rect(Rect { w: 0.5, ..glass }), CornerRadius::ZERO, shadow.gamma_multiply(0.7));

    let ink = color(c.segment);
    let polys = seg7::segment_polygons(lcd.digit_width, lcd.digit_height, lcd.segment_thickness, lcd.slant);

    if !visible {
        return;
    }
    if display.minus {
        let minus_polys =
            seg7::segment_polygons(lcd.digit_width * 0.8, lcd.digit_height, lcd.segment_thickness, lcd.slant);
        draw_segments(painter, tr, (lcd.minus_x, lcd.digit_top), &minus_polys, seg7::G, ink);
    }
    for (i, cell) in display.cells.iter().enumerate() {
        let origin = lcd.digit_origin(i);
        draw_segments(painter, tr, origin, &polys, seg7::segments(cell.glyph), ink);
        if cell.punct != Punct::None {
            draw_punct(painter, tr, skin, origin, cell.punct == Punct::Comma, ink);
        }
    }

    let a = display.annunciators;
    let font = FontId::new(tr.len(lcd.annunciator_size), FontFamily::Name(fonts::LABEL.into()));
    for ann in &lcd.annunciators {
        let on = match ann.name.as_str() {
            "low_battery" => a.low_battery,
            "f" => a.f,
            "g" => a.g,
            "begin" => a.begin,
            "dmy" => a.dmy,
            "compound" => a.compound,
            "prgm" => a.prgm,
            _ => false,
        };
        if on {
            painter.text(tr.pos(ann.x, lcd.annunciator_y), Align2::CENTER_CENTER, &ann.text, font.clone(), ink);
        }
    }
}

fn draw_segments(
    painter: &Painter,
    tr: Transform,
    origin: (f32, f32),
    polys: &[[(f32, f32); 6]; 7],
    lit: u8,
    ink: Color32,
) {
    for (i, poly) in polys.iter().enumerate() {
        if lit & (1 << i) != 0 {
            let points = poly.iter().map(|(x, y)| tr.pos(origin.0 + x, origin.1 + y)).collect();
            painter.add(Shape::convex_polygon(points, ink, Stroke::NONE));
        }
    }
}

/// Ponto decimal; a vírgula é o ponto com uma "cauda" para baixo.
fn draw_punct(painter: &Painter, tr: Transform, skin: &Skin, origin: (f32, f32), comma: bool, ink: Color32) {
    let lcd = &skin.lcd;
    let t = lcd.segment_thickness * 1.05;
    let x = origin.0 + lcd.digit_width + (lcd.digit_pitch - lcd.digit_width) / 2.0 - t / 2.0;
    let y = origin.1 + lcd.digit_height - t;
    let dot = Rect { x, y, w: t, h: t };
    painter.rect_filled(tr.rect(dot), CornerRadius::ZERO, ink);
    if comma {
        let tail = vec![tr.pos(x + t * 0.15, y + t), tr.pos(x + t, y + t), tr.pos(x - t * 0.45, y + t * 2.3)];
        painter.add(Shape::convex_polygon(tail, ink, Stroke::NONE));
    }
}

fn draw_badge(painter: &Painter, tr: Transform, skin: &Skin) {
    let b = &skin.badge;
    let c = &skin.colors;
    painter.text(
        tr.pos(b.x, b.y),
        Align2::CENTER_CENTER,
        &b.text,
        FontId::new(tr.len(b.size), FontFamily::Name(fonts::BADGE.into())),
        color(c.badge_text),
    );
    let sub_font = FontId::new(tr.len(b.sub_size), FontFamily::Name(fonts::LABEL.into()));
    let spaced: String = b.sub.chars().flat_map(|ch| [ch, '\u{2009}']).collect();
    painter.text(tr.pos(b.x, b.sub_y), Align2::CENTER_CENTER, spaced.trim_end(), sub_font, color(c.badge_sub));
}

fn draw_brackets(painter: &Painter, tr: Transform, skin: &Skin) {
    let gold = color(skin.colors.f_label);
    let font = FontId::new(tr.len(skin.keyboard.f_size), FontFamily::Name(fonts::LABEL.into()));
    let stroke = Stroke::new(tr.len(0.22), gold);
    let tick = 0.85;
    for b in &skin.brackets {
        let center = tr.pos((b.x0 + b.x1) / 2.0, b.y);
        let text = painter.text(center, Align2::CENTER_CENTER, &b.text, font.clone(), gold);
        let gap = tr.len(0.6);
        let (left, right) = (tr.pos(b.x0, b.y), tr.pos(b.x1, b.y));
        painter.line_segment([left, Pos2::new(text.left() - gap, left.y)], stroke);
        painter.line_segment([Pos2::new(text.right() + gap, right.y), right], stroke);
        painter.line_segment([left, tr.pos(b.x0, b.y + tick)], stroke);
        painter.line_segment([right, tr.pos(b.x1, b.y + tick)], stroke);
    }
}

fn draw_key(painter: &Painter, tr: Transform, skin: &Skin, key: &KeyGeometry, pressed: bool) {
    let c = &skin.colors;
    let kb = &skin.keyboard;
    let label_family = FontFamily::Name(fonts::LABEL.into());

    // Legenda dourada (f), impressa no painel acima da tecla: não se move.
    if let Some(f) = &key.f {
        let (x, y) = key.f_label_center;
        draw_label(painter, f, tr.pos(x, y), tr.len(kb.f_size), color(c.f_label), &label_family);
    }

    let (top, top_light, face, text) = match key.style {
        KeyStyle::Normal => (c.key_top, c.key_top_light, c.key_face, c.key_text),
        KeyStyle::Gold => (c.f_key_top, c.f_key_top, c.f_key_face, c.prefix_key_text),
        KeyStyle::Blue => (c.g_key_top, c.g_key_top, c.g_key_face, c.prefix_key_text),
    };
    let r = tr.len(kb.key_radius);
    let sink = if pressed { tr.len(0.35) } else { 0.0 };
    let shift = Vec2::new(0.0, sink);

    // Contorno escuro (a "sombra" da tecla no painel).
    painter.rect_filled(tr.rect(key.rect.expand(0.3)), radius(r + tr.len(0.3)), color(c.key_edge));

    let body = tr.rect(key.rect).translate(shift);
    painter.rect_filled(body, radius(r), color(face));

    let top_rect = tr.rect(key.top_face()).translate(shift);
    let (light, dark) = if pressed && key.style == KeyStyle::Normal {
        (color(c.key_pressed), color(c.key_pressed))
    } else {
        (color(top_light), color(top))
    };
    gradient_rounded_rect(painter, top_rect, r, r * 0.5, light, dark);

    // Legenda principal.
    let center = top_rect.center();
    // Dígitos e operadores de um símbolo (÷ × − + ·) usam a fonte maior.
    let plain = key.main.plain();
    let single = plain.chars().count() == 1;
    let main_size = if single && !plain.chars().all(char::is_alphabetic) {
        kb.digit_size
    } else if single {
        kb.main_size * 1.15
    } else {
        kb.main_size
    };
    if key.vertical {
        let letters: Vec<char> = key.main.plain().chars().collect();
        let step = tr.len(main_size * 0.98);
        let start = center.y - step * (letters.len() as f32 - 1.0) / 2.0;
        let font = FontId::new(tr.len(main_size), label_family.clone());
        for (i, ch) in letters.iter().enumerate() {
            let p = Pos2::new(center.x, start + step * i as f32);
            painter.text(p, Align2::CENTER_CENTER, ch, font.clone(), color(text));
        }
    } else {
        draw_label(painter, &key.main, center, tr.len(main_size), color(text), &label_family);
    }

    // Legenda azul (g) na face frontal.
    if let Some(g) = &key.g {
        let front = tr.rect(key.front_face()).translate(shift);
        draw_label(painter, g, front.center(), tr.len(kb.g_size), color(c.g_label), &label_family);
    }
}

/// Texto centralizado com sobrescrito/subscrito e barra/chapéu sobre o primeiro caractere.
fn draw_label(painter: &Painter, label: &Label, center: Pos2, size: f32, ink: Color32, family: &FontFamily) {
    let mut job = LayoutJob::default();
    for run in &label.runs {
        let (scale, valign) = match run.kind {
            RunKind::Normal => (1.0, Align::Center),
            RunKind::Superscript => (0.68, Align::TOP),
            RunKind::Subscript => (0.68, Align::BOTTOM),
        };
        job.append(
            &run.text,
            0.0,
            TextFormat { font_id: FontId::new(size * scale, family.clone()), color: ink, valign, ..Default::default() },
        );
    }
    let galley = painter.layout_job(job);
    let pos = center - galley.size() / 2.0;
    let height = galley.size().y;
    painter.galley(pos, galley, ink);

    if let (Some(mark), Some(first)) = (label.first_mark, label.first_char()) {
        let w = painter.layout_no_wrap(first.to_string(), FontId::new(size, family.clone()), ink).size().x;
        // Topo das minúsculas fica por volta de 38% da altura da linha.
        let y = pos.y + height * 0.26;
        let stroke = Stroke::new(size * 0.085, ink);
        match mark {
            Mark::Bar => {
                painter.line_segment([Pos2::new(pos.x + w * 0.1, y), Pos2::new(pos.x + w * 0.9, y)], stroke);
            }
            Mark::Hat => {
                let peak = Pos2::new(pos.x + w * 0.5, y - size * 0.1);
                painter.line_segment([Pos2::new(pos.x + w * 0.15, y + size * 0.05), peak], stroke);
                painter.line_segment([peak, Pos2::new(pos.x + w * 0.85, y + size * 0.05)], stroke);
            }
        };
    }
}

/// Retângulo arredondado com degradê vertical (de `top` para `bottom`).
fn gradient_rounded_rect(
    painter: &Painter,
    rect: egui::Rect,
    top_radius: f32,
    bottom_radius: f32,
    top: Color32,
    bottom: Color32,
) {
    let outline = rounded_outline(rect, top_radius, bottom_radius);
    let mut mesh = Mesh::default();
    let lerp = |y: f32| {
        let t = ((y - rect.top()) / rect.height().max(1.0)).clamp(0.0, 1.0);
        top.lerp_to_gamma(bottom, t)
    };
    mesh.colored_vertex(rect.center(), lerp(rect.center().y));
    for p in &outline {
        mesh.colored_vertex(*p, lerp(p.y));
    }
    let n = outline.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    painter.add(Shape::mesh(mesh));
}

/// Contorno convexo de um retângulo com cantos arredondados (sentido horário).
fn rounded_outline(rect: egui::Rect, top_radius: f32, bottom_radius: f32) -> Vec<Pos2> {
    const STEPS: usize = 6;
    let max_r = rect.width().min(rect.height()) / 2.0;
    let (rt, rb) = (top_radius.min(max_r), bottom_radius.min(max_r));
    let corners = [
        (Pos2::new(rect.right() - rt, rect.top() + rt), rt, -90.0_f32),
        (Pos2::new(rect.right() - rb, rect.bottom() - rb), rb, 0.0),
        (Pos2::new(rect.left() + rb, rect.bottom() - rb), rb, 90.0),
        (Pos2::new(rect.left() + rt, rect.top() + rt), rt, 180.0),
    ];
    let mut points = Vec::with_capacity(4 * (STEPS + 1));
    for (center, r, start) in corners {
        for s in 0..=STEPS {
            let angle = (start + 90.0 * s as f32 / STEPS as f32).to_radians();
            points.push(Pos2::new(center.x + r * angle.cos(), center.y + r * angle.sin()));
        }
    }
    points
}
