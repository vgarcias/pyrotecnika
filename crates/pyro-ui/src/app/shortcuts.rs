//! Preferencias › Atajos: lista de comandos con sus atajos, captura de una
//! combinación nueva, conflictos, restablecer, exportar e importar.

use std::path::Path;

use eframe::egui::{self, Key, Modifiers};

use super::PyroApp;
use crate::commands::catalog::{find, CATALOG};
use crate::commands::chord::Chord;
use crate::commands::keymap::{Keymap, KeymapFile};
use crate::commands::store::write_atomic;
use crate::commands::{self};
use crate::design::widgets;

/// Teclas que solas no forman un atajo (son modificadores o navegación del
/// propio diálogo).
fn is_modifier_only(key: Key) -> bool {
    matches!(key, Key::Tab)
}

impl PyroApp {
    /// Mientras se espera una combinación, la primera tecla pulsada es la
    /// combinación (y nadie más la ve). Esc cancela la captura o el aviso de
    /// conflicto sin cerrar Preferencias.
    pub(super) fn capture_chord(&mut self, ctx: &egui::Context) {
        let Some(prefs) = self.prefs.as_mut() else { return };
        if !prefs.waiting && prefs.pending.is_none() {
            return;
        }
        let escape = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        if escape {
            prefs.waiting = false;
            prefs.pending = None;
            return;
        }
        if !prefs.waiting {
            return;
        }
        let chord = ctx.input_mut(|i| {
            let pos = i.events.iter().position(
                |e| matches!(e, egui::Event::Key { pressed: true, repeat: false, key, .. } if !is_modifier_only(*key)),
            )?;
            let egui::Event::Key { key, modifiers, .. } = i.events.remove(pos) else { return None };
            // El texto que esa tecla habría escrito tampoco debe llegar a nadie.
            i.events.retain(|e| !matches!(e, egui::Event::Text(_)));
            Some(Chord { ctrl: modifiers.command, alt: modifiers.alt, shift: modifiers.shift, key })
        });
        let (Some(chord), Some(id)) = (chord, prefs.selected) else { return };
        prefs.waiting = false;
        prefs.message = None;
        if !Keymap::allowed(id, chord) {
            prefs.message = Some("Esta tecla sola solo vale para herramientas; añade Ctrl, Alt o Mayús.".into());
        } else if self.keymap.conflict(id, chord).is_some() {
            prefs.pending = Some((id, chord));
        } else {
            self.keymap.assign(id, chord);
            self.save_keymap();
        }
    }

    pub(super) fn save_keymap(&mut self) {
        if let Err(e) = self.store.save_keymap(&self.keymap) {
            self.warn_save_failed(e);
        }
    }

    pub(super) fn shortcuts_ui(&mut self, ui: &mut egui::Ui) {
        let mac = commands::is_mac();
        let Some(prefs) = self.prefs.as_mut() else { return };
        ui.horizontal(|ui| {
            ui.label("Buscar");
            ui.add(egui::TextEdit::singleline(&mut prefs.search).hint_text("Nombre o atajo").desired_width(240.0));
        });
        ui.add_space(4.0);
        let query = super::palette::normalize(&prefs.search);
        let mut pick = None;
        egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
            let mut last_category = None;
            for def in CATALOG {
                let chords: Vec<String> = self.keymap.chords(def.id).iter().map(|c| c.label(mac)).collect();
                let shown = chords.join(", ");
                if !query.is_empty()
                    && !super::palette::normalize(def.name).contains(&query)
                    && !super::palette::normalize(&shown).contains(&query)
                {
                    continue;
                }
                if last_category != Some(def.category) {
                    last_category = Some(def.category);
                    ui.add_space(4.0);
                    ui.weak(def.category.label());
                }
                let modified = !self.keymap.is_default(def.id);
                let name = if modified { format!("{} (modificado)", def.name) } else { def.name.to_string() };
                ui.horizontal(|ui| {
                    let width = (ui.available_width() - 180.0).max(120.0);
                    let row = ui.allocate_ui(egui::vec2(width, 24.0), |ui| {
                        widgets::active_row(ui, prefs.selected == Some(def.id), &name)
                    });
                    if row.inner.clicked() {
                        pick = Some(def.id);
                    }
                    let text = egui::RichText::new(if shown.is_empty() { "—".to_string() } else { shown }).monospace();
                    ui.label(text);
                });
            }
        });
        if let Some(id) = pick {
            prefs.selected = Some(id);
            prefs.waiting = false;
            prefs.pending = None;
            prefs.message = None;
        }
        ui.separator();

        let mut changed = false;
        if let Some(id) = prefs.selected {
            let def = find(id).expect("id del catálogo");
            ui.label(egui::RichText::new(def.name).strong());
            ui.horizontal_wrapped(|ui| {
                for c in self.keymap.chords(id).to_vec() {
                    if widgets::button(ui, format!("{} ×", c.label(mac))).on_hover_text("Quitar este atajo").clicked()
                    {
                        self.keymap.remove(id, c);
                        changed = true;
                    }
                }
                if prefs.waiting {
                    ui.label("Pulsa la combinación… (Esc cancela)");
                } else if widgets::button(ui, "+ Añadir").clicked() {
                    prefs.waiting = true;
                    prefs.pending = None;
                    prefs.message = None;
                }
                if ui.add_enabled(!self.keymap.is_default(id), egui::Button::new("Restablecer")).clicked() {
                    let lost = self.keymap.reset(id);
                    prefs.message = (!lost.is_empty()).then(|| {
                        let parts: Vec<String> = lost
                            .iter()
                            .map(|(other, c)| {
                                let name = find(other).map_or(*other, |d| d.name);
                                format!("{} se quitó a «{name}»", c.label(mac))
                            })
                            .collect();
                        format!("Restablecido. {}.", parts.join("; "))
                    });
                    changed = true;
                }
            });
            if let Some((pid, chord)) = prefs.pending {
                let owner = self.keymap.conflict(pid, chord).and_then(find).map_or("otro comando", |d| d.name);
                ui.label(format!("⚠ {} ya lo usa «{owner}».", chord.label(mac)));
                ui.horizontal(|ui| {
                    if widgets::primary_button(ui, "Reasignar").clicked() {
                        self.keymap.assign(pid, chord);
                        prefs.pending = None;
                        changed = true;
                    }
                    if widgets::button(ui, "Cancelar").clicked() {
                        prefs.pending = None;
                    }
                });
            }
        } else {
            ui.weak("Elige un comando para ver y cambiar sus atajos.");
        }
        if let Some(msg) = &prefs.message {
            ui.label(msg.as_str());
        }
        ui.add_space(6.0);
        let mut import = None;
        let mut export = false;
        ui.horizontal(|ui| {
            if widgets::button(ui, "Exportar…").clicked() {
                export = true;
            }
            if widgets::button(ui, "Importar…").clicked() {
                import = rfd::FileDialog::new().add_filter("Atajos de Pyrotecnika", &["toml"]).pick_file();
            }
            if widgets::button(ui, "Restablecer todos a Fireworks").clicked() {
                self.keymap = Keymap::fireworks();
                changed = true;
            }
        });
        if export {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Atajos de Pyrotecnika", &["toml"])
                .set_file_name("atajos-pyrotecnika.toml")
                .save_file()
            {
                prefs.message = Some(match export_keymap(&self.keymap, &path) {
                    Ok(()) => format!("Atajos exportados a {}.", path.display()),
                    Err(e) => e,
                });
            }
        }
        if let Some(path) = import {
            match import_keymap(&path) {
                Ok((k, warns)) => {
                    self.keymap = k;
                    changed = true;
                    prefs.message = Some(if warns.is_empty() {
                        "Atajos importados.".into()
                    } else {
                        format!("Atajos importados; {} entradas se ignoraron: {}", warns.len(), warns.join(" "))
                    });
                }
                Err(e) => prefs.message = Some(e),
            }
        }
        if changed {
            self.save_keymap();
        }
    }
}

/// Escribe los atajos personalizados (solo las diferencias con Fireworks).
pub(super) fn export_keymap(k: &Keymap, path: &Path) -> Result<(), String> {
    let text = toml::to_string_pretty(&k.to_file()).map_err(|e| e.to_string())?;
    write_atomic(path, &text).map_err(|e| format!("No se pudieron exportar los atajos: {e}."))
}

/// Lee un archivo de atajos; sustituye las personalizaciones vigentes.
pub(super) fn import_keymap(path: &Path) -> Result<(Keymap, Vec<String>), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("No se pudo leer {}: {e}.", path.display()))?;
    let table: toml::Table =
        toml::from_str(&text).map_err(|_| format!("{} no es un archivo de atajos válido.", path.display()))?;
    let mut warns = Vec::new();
    let file = KeymapFile::from_table(&table, &mut warns);
    let (k, more) = Keymap::from_file(&file);
    warns.extend(more);
    Ok((k, warns))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exportar_e_importar() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("atajos.toml");
        let mut k = Keymap::fireworks();
        k.assign("file.export", Chord::parse("Ctrl+E").unwrap());
        export_keymap(&k, &path).unwrap();
        assert_eq!(import_keymap(&path).unwrap(), (k, vec![]));
        std::fs::write(&path, "no es toml [").unwrap();
        assert!(import_keymap(&path).is_err());
    }
}
