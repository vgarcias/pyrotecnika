//! Teclado y archivos soltados sobre la ventana.

use std::path::PathBuf;

use eframe::egui::{self, Key};

use super::{Action, Pending, PyroApp};
use crate::commands::chord::Chord;

impl PyroApp {
    pub(super) fn handle_input(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if let Some(path) = dropped.into_iter().next() {
            self.request(Pending::Open(path), ctx);
        }

        // Con un diálogo modal abierto (confirmación, preferencias, paleta),
        // o escribiendo en un campo, los atajos no actúan sobre el documento.
        if self.confirm.is_some() || self.prefs.is_some() || self.palette.is_some() || ctx.wants_keyboard_input() {
            return;
        }
        // Esc cancela la tarea en curso si no hay ninguna ventana que lo use antes.
        let windows = self.tab.filter_dialog.is_some()
            || self.tab.export_dialog.is_some()
            || self.new_dialog.is_some()
            || self.update.is_some();
        if self.busy() && !windows && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
            self.cancel_task();
        }
        let pressed: Vec<(Chord, bool)> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Key { key, pressed: true, repeat, modifiers, .. } => Some((
                        Chord { ctrl: modifiers.command, alt: modifiers.alt, shift: modifiers.shift, key: *key },
                        *repeat,
                    )),
                    _ => None,
                })
                .collect()
        });
        for (chord, repeat) in pressed {
            let action = self.keymap.resolve(chord, self.tool).or_else(|| {
                // `+` con Mayús (teclado inglés) también acerca.
                (chord.shift && chord.key == Key::Plus)
                    .then(|| self.keymap.resolve(Chord { shift: false, ..chord }, self.tool))
                    .flatten()
            });
            let Some(action) = action else { continue };
            // Mantener pulsada una tecla solo repite el tamaño del pincel.
            if repeat && !matches!(action, Action::BrushSmaller | Action::BrushBigger) {
                continue;
            }
            self.run(action, ctx);
        }
    }
}
