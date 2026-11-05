//! Janela principal: liga mouse/teclado ao núcleo e desenha o resultado.

use crate::storage::{self, Settings};
use crate::program_window::ProgramEditor;
use crate::{fonts, icon, input, manual, registers, render};
use std::sync::Arc;
use eframe::egui::{self, Sense, Vec2, ViewportCommand, WindowLevel};
use hp12c_core::{Calculator, Key};
use hp12c_skin::Skin;
use std::time::Duration;

/// 1 mm em pontos de tela (96 pontos por polegada).
const POINTS_PER_MM: f32 = 96.0 / 25.4;
pub const DEFAULT_ZOOM: f32 = 1.5;
const ZOOMS: [f32; 7] = [0.75, 1.0, 1.25, 1.5, 2.0, 2.5, 3.0];
/// Tempo que uma tecla digitada como texto fica "afundada" na tela.
const FLASH: Duration = Duration::from_millis(110);

pub fn window_size(skin: &Skin, zoom: f32) -> Vec2 {
    Vec2::new(skin.width, skin.height) * POINTS_PER_MM * zoom
}

pub struct App {
    calc: Calculator,
    skin: Skin,
    zoom: f32,
    always_on_top: bool,
    /// Tecla segurada com o mouse.
    mouse_key: Option<Key>,
    /// Teclas seguradas no teclado do PC.
    keyboard_keys: Vec<Key>,
    /// Tecla digitada como texto, mostrada afundada até o instante indicado.
    flash: Option<(Key, f64)>,
    /// Aviso curto na parte de baixo da janela, até o instante indicado.
    notice: Option<(String, f64)>,
    settings: Settings,
    /// Último estado gravado em disco (só grava de novo quando muda).
    saved_state: String,
    saved_settings: Settings,
    last_save_check: f64,
    /// Janela "Registradores" aberta.
    show_registers: bool,
    /// Posição inicial da janela de registradores (ao lado da calculadora).
    registers_pos: Option<[f32; 2]>,
    /// Janela "Programa" aberta, sua posição inicial e o estado do editor.
    show_program: bool,
    program_pos: Option<[f32; 2]>,
    program_editor: ProgramEditor,
    window_icon: Arc<egui::IconData>,
}

/// Intervalo entre verificações de alterações para salvar, em segundos.
const SAVE_INTERVAL: f64 = 1.0;

/// Cabeçalho que identifica uma listagem de programa colada.
const PROGRAM_HEADER: &str = "# HP-12C";
const NOTICE_SECONDS: f64 = 3.0;

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, skin: Skin, calc: Calculator, settings: Settings) -> Self {
        fonts::install(&cc.egui_ctx);
        Self {
            saved_state: calc.save_state(),
            calc,
            skin,
            zoom: settings.zoom,
            always_on_top: settings.always_on_top,
            mouse_key: None,
            keyboard_keys: Vec::new(),
            flash: None,
            notice: None,
            show_registers: settings.show_registers,
            registers_pos: None,
            show_program: settings.show_program,
            program_pos: None,
            program_editor: ProgramEditor::new(),
            window_icon: Arc::new(icon::calculator_icon()),
            saved_settings: settings.clone(),
            settings,
            last_save_check: 0.0,
        }
    }

    fn toggle_registers(&mut self) {
        self.show_registers = !self.show_registers;
        if self.show_registers {
            self.registers_pos = None; // reposiciona ao lado da calculadora
        }
    }

    /// Janela nativa separada, ao lado da calculadora, com o estado interno.
    fn registers_window(&mut self, ctx: &egui::Context) {
        if self.registers_pos.is_none() {
            let main = ctx.input(|i| i.viewport().outer_rect);
            self.registers_pos = main.map(|r| [r.max.x + 12.0, r.min.y]);
        }
        let mut builder = egui::ViewportBuilder::default()
            .with_title("HP-12C — Registradores")
            .with_inner_size([380.0, 660.0])
            .with_min_inner_size([300.0, 320.0])
            .with_icon(self.window_icon.clone());
        if let Some(pos) = self.registers_pos {
            builder = builder.with_position(pos);
        }
        let calc = &self.calc;
        let closed = ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("hp12c-registradores"), builder, |ui, _| {
            egui::CentralPanel::default_margins().show(ui, |ui| registers::show(ui, calc));
            ui.ctx().input(|i| i.viewport().close_requested())
        });
        if closed {
            self.show_registers = false;
        }
    }

    fn toggle_program(&mut self) {
        self.show_program = !self.show_program;
        if self.show_program {
            self.program_pos = None;
        }
    }

    /// Janela do editor de programas, à direita da calculadora (e dos registradores, se abertos).
    fn program_window(&mut self, ctx: &egui::Context) {
        if self.program_pos.is_none() {
            let offset = if self.show_registers { 404.0 } else { 0.0 };
            let main = ctx.input(|i| i.viewport().outer_rect);
            self.program_pos = main.map(|r| [r.max.x + 12.0 + offset, r.min.y]);
        }
        let mut builder = egui::ViewportBuilder::default()
            .with_title("HP-12C — Programa")
            .with_inner_size([520.0, 660.0])
            .with_min_inner_size([400.0, 320.0])
            .with_icon(self.window_icon.clone());
        if let Some(pos) = self.program_pos {
            builder = builder.with_position(pos);
        }
        let (calc, editor, skin) = (&mut self.calc, &mut self.program_editor, &self.skin);
        let closed = ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("hp12c-programa"), builder, |ui, _| {
            egui::CentralPanel::default_margins().show(ui, |ui| editor.show(ui, calc, skin));
            ui.ctx().input(|i| i.viewport().close_requested())
        });
        if closed {
            self.show_program = false;
        }
    }

    /// Grava estado e preferências se mudaram (memória contínua).
    fn persist(&mut self, ctx: &egui::Context) {
        self.settings.zoom = self.zoom;
        self.settings.always_on_top = self.always_on_top;
        self.settings.show_registers = self.show_registers;
        self.settings.show_program = self.show_program;
        if let Some(rect) = ctx.input(|i| i.viewport().outer_rect) {
            self.settings.position = Some([rect.min.x, rect.min.y]);
        }
        if self.settings != self.saved_settings {
            storage::save_settings(&self.settings);
            self.saved_settings = self.settings.clone();
        }
        let state = self.calc.save_state();
        if state != self.saved_state {
            storage::save_state(&state);
            self.saved_state = state;
        }
    }

    /// Liga a calculadora com uma tecla pressionada (testes de inicialização).
    fn power_on_holding(&mut self, key: Key) {
        if self.calc.is_powered() {
            self.calc.tap(Key::On);
        }
        self.calc.press(key);
        self.calc.tap(Key::On);
        self.calc.release(key);
    }

    fn notify(&mut self, ctx: &egui::Context, text: impl Into<String>) {
        let now = ctx.input(|i| i.time);
        self.notice = Some((text.into(), now + NOTICE_SECONDS));
        ctx.request_repaint_after(Duration::from_secs_f64(NOTICE_SECONDS));
    }

    fn open_manual(&mut self, ctx: &egui::Context) {
        match manual::open() {
            Ok(()) => self.notify(ctx, "Abrindo o manual"),
            Err(_) => self.notify(ctx, "Manual não abriu"),
        }
    }

    fn paste(&mut self, ctx: &egui::Context, text: &str) {
        if text.trim_start().starts_with(PROGRAM_HEADER) {
            match self.calc.load_program(text) {
                Ok(()) => self.notify(ctx, "Programa carregado"),
                Err(e) => self.notify(ctx, format!("Programa inválido ({e})")),
            }
        } else if let Some(value) = input::parse_pasted(text) {
            self.calc.input_value(value);
        }
    }

    fn is_pressed(&self, key: Key, now: f64) -> bool {
        self.mouse_key == Some(key)
            || self.keyboard_keys.contains(&key)
            || self.flash.is_some_and(|(k, until)| k == key && now < until)
    }

    fn handle_mouse(&mut self, ctx: &egui::Context, response: &egui::Response, tr: render::Transform) {
        let (pressed, released, pos) =
            ctx.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_released(), i.pointer.interact_pos()));
        if pressed && response.hovered() {
            if let Some(pos) = pos {
                let (x, y) = tr.to_mm(pos);
                match self.skin.hit_test(x, y) {
                    Some(key) => {
                        self.calc.press(key);
                        self.mouse_key = Some(key);
                    }
                    // Fora das teclas: arrasta a janela.
                    None => ctx.send_viewport_cmd(ViewportCommand::StartDrag),
                }
            }
        }
        if released {
            if let Some(key) = self.mouse_key.take() {
                self.calc.release(key);
            }
        }
    }

    fn handle_keyboard(&mut self, ctx: &egui::Context) {
        let (events, now) = ctx.input(|i| (i.events.clone(), i.time));
        for event in events {
            match event {
                egui::Event::Key { key, pressed, repeat, modifiers, .. } => {
                    if repeat || modifiers.ctrl || modifiers.alt || modifiers.command {
                        continue;
                    }
                    if key == egui::Key::Escape {
                        if pressed {
                            self.calc.cancel_prefix();
                        }
                        continue;
                    }
                    if key == egui::Key::F1 {
                        if pressed {
                            self.open_manual(ctx);
                        }
                        continue;
                    }
                    if key == egui::Key::F2 {
                        if pressed {
                            self.toggle_registers();
                        }
                        continue;
                    }
                    if key == egui::Key::F3 {
                        if pressed {
                            self.toggle_program();
                        }
                        continue;
                    }
                    let Some(hp) = input::map_key(key) else { continue };
                    if pressed {
                        if !self.keyboard_keys.contains(&hp) {
                            self.keyboard_keys.push(hp);
                            self.calc.press(hp);
                        }
                    } else if let Some(i) = self.keyboard_keys.iter().position(|k| *k == hp) {
                        self.keyboard_keys.remove(i);
                        self.calc.release(hp);
                    }
                }
                egui::Event::Text(text) => {
                    for ch in text.chars() {
                        if let Some(hp) = input::map_char(ch) {
                            self.calc.tap(hp);
                            self.flash = Some((hp, now + FLASH.as_secs_f64()));
                            ctx.request_repaint_after(FLASH);
                        }
                    }
                }
                egui::Event::Copy => self.copy_x(ctx),
                egui::Event::Paste(text) => self.paste(ctx, &text),
                _ => {}
            }
        }
    }

    fn copy_x(&self, ctx: &egui::Context) {
        ctx.copy_text(input::format_for_copy(self.calc.x(), self.calc.comma_decimal()));
    }

    fn context_menu(&mut self, ui: &mut egui::Ui) {
        if ui.button("Copiar X  (Ctrl+C)").clicked() {
            self.copy_x(ui.ctx());
            ui.close();
        }
        ui.label("Colar em X: Ctrl+V");
        if ui.button("Copiar programa").clicked() {
            ui.ctx().copy_text(self.calc.program_listing());
            self.notify(ui.ctx(), "Programa copiado");
            ui.close();
        }
        ui.label("Colar programa: Ctrl+V com a listagem");
        ui.separator();
        let mut show = self.show_registers;
        if ui.checkbox(&mut show, "Registradores  (F2)").clicked() {
            self.toggle_registers();
            ui.close();
        }
        let mut show_program = self.show_program;
        if ui.checkbox(&mut show_program, "Programa: ver e editar  (F3)").clicked() {
            self.toggle_program();
            ui.close();
        }
        ui.menu_button("Zoom", |ui| {
            for zoom in ZOOMS {
                let label = format!("{:.0}%", zoom * 100.0);
                if ui.radio(self.zoom == zoom, label).clicked() {
                    self.zoom = zoom;
                    ui.ctx().send_viewport_cmd(ViewportCommand::InnerSize(window_size(&self.skin, zoom)));
                    ui.close();
                }
            }
        });
        if ui.checkbox(&mut self.always_on_top, "Sempre no topo").clicked() {
            let level = if self.always_on_top { WindowLevel::AlwaysOnTop } else { WindowLevel::Normal };
            ui.ctx().send_viewport_cmd(ViewportCommand::WindowLevel(level));
            ui.close();
        }
        ui.separator();
        let power = if self.calc.is_powered() { "Desligar (ON)" } else { "Ligar (ON)" };
        if ui.button(power).clicked() {
            self.calc.tap(Key::On);
            ui.close();
        }
        ui.menu_button("Ligar segurando…", |ui| {
            let tests = [
                ("Autoteste do display (ON + ×)", Key::Mul),
                ("Teste de teclado (ON + ÷)", Key::Div),
                ("Trocar ponto/vírgula (ON + .)", Key::Dot),
                ("Apagar toda a memória (ON + −)", Key::Sub),
            ];
            for (label, key) in tests {
                if ui.button(label).clicked() {
                    self.power_on_holding(key);
                    ui.close();
                }
            }
        });
        if ui.button("Minimizar").clicked() {
            ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
            ui.close();
        }
        ui.separator();
        if ui.button("Manual ilustrado  (F1)").clicked() {
            self.open_manual(ui.ctx());
            ui.close();
        }
        if ui.button("Sair").clicked() {
            ui.ctx().send_viewport_cmd(ViewportCommand::Close);
        }
        ui.separator();
        ui.weak(concat!("HP-12C emulador v", env!("CARGO_PKG_VERSION")));
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let rect = ui.max_rect();
        let response = ui.allocate_rect(rect, Sense::click());
        let tr = render::Transform { origin: rect.min, scale: rect.width() / self.skin.width };

        self.handle_keyboard(&ctx);
        self.handle_mouse(&ctx, &response, tr);
        response.context_menu(|ui| self.context_menu(ui));

        // Programa em execução: avança e continua redesenhando ("running" pisca).
        let elapsed_ms = (ctx.input(|i| i.stable_dt) * 1000.0).clamp(1.0, 100.0) as u32;
        self.calc.tick(elapsed_ms);
        if self.calc.is_running() {
            ctx.request_repaint();
        }

        let now = ctx.input(|i| i.time);
        let display = self.calc.display();
        render::draw(ui.painter(), tr, &self.skin, &display, now, &|k| self.is_pressed(k, now));
        if let Some((text, until)) = &self.notice {
            if now < *until {
                render::draw_notice(ui.painter(), tr, &self.skin, text);
            }
        }

        if self.show_registers {
            self.registers_window(&ctx);
        }
        if self.show_program {
            self.program_window(&ctx);
        }

        if now - self.last_save_check >= SAVE_INTERVAL {
            self.last_save_check = now;
            self.persist(&ctx);
        }
        // Garante uma verificação mesmo sem eventos (ex.: depois de mover a janela).
        ctx.request_repaint_after(Duration::from_secs_f64(SAVE_INTERVAL));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.settings.show_registers = self.show_registers;
        self.settings.show_program = self.show_program;
        self.settings.zoom = self.zoom;
        self.settings.always_on_top = self.always_on_top;
        storage::save_settings(&self.settings);
        storage::save_state(&self.calc.save_state());
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4] // fundo transparente: só a calculadora aparece
    }
}
