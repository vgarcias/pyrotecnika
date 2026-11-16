//! Teclado y archivos soltados sobre la ventana.

use std::path::PathBuf;

use eframe::egui::{self, Key, Modifiers};

use super::tools::TOOLS;
use super::{Action, Pending, PyroApp};

impl PyroApp {
    pub(super) fn handle_input(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if let Some(path) = dropped.into_iter().next() {
            self.request(Pending::Open(path), ctx);
        }

        // Con la confirmación abierta, los atajos no actúan sobre el documento.
        if self.confirm.is_some() || ctx.wants_keyboard_input() {
            return;
        }
        let mut actions = Vec::new();
        let mut new_tool = None;
        let mut size_delta = 0.0;
        ctx.input_mut(|i| {
            let cmd_shift = Modifiers::COMMAND | Modifiers::SHIFT;
            if i.consume_key(cmd_shift, Key::Z) || i.consume_key(Modifiers::COMMAND, Key::Y) {
                actions.push(Action::Redo);
            }
            if i.consume_key(cmd_shift, Key::I) {
                actions.push(Action::InvertSelection);
            }
            let simple = [
                (Key::Z, Action::Undo),
                (Key::N, Action::New),
                (Key::O, Action::Open),
                (Key::E, Action::Export),
                (Key::A, Action::SelectAll),
                (Key::D, Action::Deselect),
                (Key::Num0, Action::Fit),
                (Key::Num1, Action::ActualSize),
                (Key::Plus, Action::ZoomIn),
                (Key::Equals, Action::ZoomIn),
                (Key::Minus, Action::ZoomOut),
            ];
            for (key, action) in simple {
                if i.consume_key(Modifiers::COMMAND, key) {
                    actions.push(action);
                }
            }
            if i.modifiers.is_none() {
                if i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace) {
                    actions.push(Action::ClearSelection);
                }
                for (tool, _, key) in TOOLS {
                    if i.key_pressed(key) {
                        new_tool = Some(tool);
                    }
                }
                if i.key_pressed(Key::OpenBracket) {
                    size_delta -= 2.0;
                }
                if i.key_pressed(Key::CloseBracket) {
                    size_delta += 2.0;
                }
            }
        });
        if let Some(t) = new_tool {
            self.tool = t;
        }
        self.brush.radius = (self.brush.radius + size_delta).clamp(0.5, 250.0);
        for a in actions {
            self.run(a, ctx);
        }
    }
}
