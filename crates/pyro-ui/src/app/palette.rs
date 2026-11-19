//! Paleta de comandos (Ctrl+K): busca cualquier comando del catálogo, y los
//! archivos recientes, por nombre.

use eframe::egui::{self, Align2, Id, Key, Modifiers, Sense, TextWrapMode, WidgetText};

use super::{guard, Action, PyroApp};
use crate::commands::catalog::{Category, CATALOG};
use crate::design::style::palette as tokens;

/// Estado de la paleta abierta.
pub(super) struct Palette {
    query: String,
    selected: usize,
    focused: bool,
}

impl Palette {
    pub(super) fn new() -> Palette {
        Palette { query: String::new(), selected: 0, focused: false }
    }

    /// Paleta abierta con una búsqueda ya escrita (escenas de prueba).
    #[cfg(test)]
    pub(super) fn with_query(query: &str) -> Palette {
        Palette { query: query.into(), ..Palette::new() }
    }
}

/// Una fila de la paleta.
pub(super) struct Entry {
    pub label: String,
    pub category: &'static str,
    pub shortcut: String,
    pub available: bool,
    pub action: Action,
}

/// Minúsculas, sin tildes ni «…», para buscar sin acertar la grafía.
pub(crate) fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| *c != '…')
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'ä' => 'a',
            'é' | 'è' | 'ë' => 'e',
            'í' | 'ì' | 'ï' => 'i',
            'ó' | 'ò' | 'ö' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            'ñ' => 'n',
            other => other,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Peor puntuación es mayor; `None` si no coincide.
/// 0: el nombre empieza por la búsqueda, o sus iniciales («ca» → Combinar
/// abajo); 1: alguna palabra empieza por ella; 2: aparece dentro;
/// 3: sus letras son iniciales de palabras, en orden.
fn score(query: &str, label: &str) -> Option<u8> {
    let words: Vec<&str> = label.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let initials: String = words.iter().filter_map(|w| w.chars().next()).collect();
    if label.starts_with(query) || (query.chars().count() >= 2 && initials.starts_with(query)) {
        Some(0)
    } else if words.iter().any(|w| w.starts_with(query)) {
        Some(1)
    } else if label.contains(query) {
        Some(2)
    } else {
        let mut it = initials.chars();
        query.chars().all(|q| it.any(|c| c == q)).then_some(3)
    }
}

/// Índices de `entries` que coinciden con `query`, mejores primero; dentro
/// de cada grupo, los disponibles antes y luego el orden del catálogo.
pub(super) fn matches(query: &str, entries: &[Entry]) -> Vec<usize> {
    let q = normalize(query);
    let mut found: Vec<(bool, u8, usize)> = entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let s = if q.is_empty() { Some(0) } else { score(&q, &normalize(&e.label)) };
            s.map(|s| (!e.available, s, i))
        })
        .collect();
    found.sort();
    found.into_iter().map(|(_, _, i)| i).collect()
}

impl PyroApp {
    fn palette_entries(&self) -> Vec<Entry> {
        let commands = CATALOG.iter().map(|c| Entry {
            label: c.name.to_string(),
            category: c.category.label(),
            shortcut: self.menu_shortcut(c.id),
            available: self.available(c.action),
            action: c.action,
        });
        let recent = self.settings.recent.files.iter().enumerate().map(|(i, p)| Entry {
            label: format!("Abrir reciente: {}", guard::short_name(p)),
            category: Category::File.label(),
            shortcut: String::new(),
            available: true,
            action: Action::OpenRecent(i),
        });
        commands.chain(recent).collect()
    }

    pub(super) fn palette_window(&mut self, ctx: &egui::Context) {
        if self.palette.is_none() {
            return;
        }
        let entries = self.palette_entries();
        let p = tokens(ctx);
        let Some(state) = self.palette.as_mut() else { return };
        let found = matches(&state.query, &entries);
        // Flechas e Intro antes que el campo de texto, que se los quedaría.
        let (down, up, enter) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::Enter),
            )
        });
        let enabled: Vec<usize> = found.iter().copied().filter(|&i| entries[i].available).collect();
        if down && !enabled.is_empty() {
            state.selected = (state.selected + 1).min(enabled.len() - 1);
        }
        if up {
            state.selected = state.selected.saturating_sub(1);
        }
        state.selected = state.selected.min(enabled.len().saturating_sub(1));
        let mut chosen = enter.then(|| enabled.get(state.selected).map(|&i| entries[i].action)).flatten();

        let id = Id::new("paleta-de-comandos");
        let top = ctx.screen_rect().height() * 0.15;
        let area = egui::Modal::default_area(id).anchor(Align2::CENTER_TOP, egui::vec2(0.0, top));
        let modal = egui::Modal::new(id).area(area).show(ctx, |ui| {
            ui.set_width(520.0);
            let edit = ui.add(
                egui::TextEdit::singleline(&mut state.query)
                    .hint_text("Buscar un comando…")
                    .desired_width(f32::INFINITY),
            );
            if !state.focused {
                edit.request_focus();
                state.focused = true;
            }
            if edit.changed() {
                state.selected = 0;
            }
            ui.add_space(4.0);
            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                if found.is_empty() {
                    ui.weak("Ningún comando coincide.");
                }
                for &i in &found {
                    let e = &entries[i];
                    let selected = e.available && enabled.get(state.selected) == Some(&i);
                    let width = ui.available_width();
                    let sense = if e.available { Sense::click() } else { Sense::hover() };
                    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 24.0), sense);
                    let name =
                        if e.shortcut.is_empty() { e.label.clone() } else { format!("{}, {}", e.label, e.shortcut) };
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, e.available, selected, &name)
                    });
                    if selected {
                        response.scroll_to_me(None);
                    }
                    if ui.is_rect_visible(rect) {
                        let (fill, ink, weak) = if selected {
                            (p.active, p.on_active, p.on_active)
                        } else if !e.available {
                            (egui::Color32::TRANSPARENT, p.text_disabled, p.text_disabled)
                        } else if response.hovered() {
                            (p.bg_hover, p.text_primary, p.text_secondary)
                        } else {
                            (egui::Color32::TRANSPARENT, p.text_primary, p.text_secondary)
                        };
                        ui.painter().rect_filled(rect, 4, fill);
                        let right = format!("{}   {}", e.category, e.shortcut);
                        let right = WidgetText::from(egui::RichText::new(right.trim_end()).monospace()).into_galley(
                            ui,
                            Some(TextWrapMode::Extend),
                            f32::INFINITY,
                            egui::TextStyle::Monospace,
                        );
                        let left = WidgetText::from(e.label.as_str()).into_galley(
                            ui,
                            Some(TextWrapMode::Truncate),
                            (width - right.size().x - 32.0).max(40.0),
                            egui::TextStyle::Body,
                        );
                        let y = rect.center().y;
                        ui.painter().galley(egui::pos2(rect.left() + 8.0, y - left.size().y / 2.0), left, ink);
                        let rx = rect.right() - 8.0 - right.size().x;
                        ui.painter().galley(egui::pos2(rx, y - right.size().y / 2.0), right, weak);
                    }
                    if response.clicked() {
                        chosen = Some(e.action);
                    }
                }
            });
        });
        if let Some(action) = chosen {
            self.palette = None;
            self.run(action, ctx);
        } else if modal.should_close() {
            self.palette = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::catalog::CATALOG;

    fn entries_for_test() -> Vec<Entry> {
        CATALOG
            .iter()
            .map(|c| Entry {
                label: c.name.to_string(),
                category: c.category.label(),
                shortcut: String::new(),
                available: true,
                action: c.action,
            })
            .collect()
    }

    #[test]
    fn sin_tildes_ni_mayusculas() {
        assert_eq!(normalize("Desenfoque Gaussiano…"), "desenfoque gaussiano");
        assert_eq!(normalize("Selección"), "seleccion");
    }

    #[test]
    fn busqueda() {
        let e = entries_for_test();
        let first = |q: &str| e[matches(q, &e)[0]].label.clone();
        assert_eq!(first("gaus"), "Desenfoque gaussiano…");
        assert_eq!(first("seleccion"), "Seleccionar todo");
        assert_eq!(first("ca"), "Combinar abajo"); // iniciales
        assert!(matches("zzz", &e).is_empty());
        assert_eq!(matches("", &e).len(), e.len());
    }

    #[test]
    fn no_disponibles_al_final() {
        let mut e = entries_for_test();
        let i = e.iter().position(|x| x.label == "Deseleccionar").unwrap();
        e[i].available = false;
        let found = matches("desel", &e);
        assert!(found.contains(&i));
        let j = e.iter().position(|x| x.label == "Seleccionar todo").unwrap();
        let found = matches("selec", &e);
        let (pi, pj) = (found.iter().position(|&x| x == i), found.iter().position(|&x| x == j));
        assert!(pi.is_none() || pi > pj, "un no disponible va detrás");
    }
}
