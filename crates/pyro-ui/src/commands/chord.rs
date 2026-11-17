//! Una combinación de teclas: modificadores más una tecla de egui.

use eframe::egui::{InputState, Key, Modifiers};

/// Modificadores más una tecla. `ctrl` es la tecla de comando de cada SO
/// (Ctrl en Windows y Linux, ⌘ en macOS).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct Chord {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub key: Key,
}

impl Chord {
    /// Lee `Ctrl+Shift+R`: modificadores en cualquier orden y sin distinguir
    /// mayúsculas, y al final el nombre de la tecla según egui.
    pub(crate) fn parse(text: &str) -> Result<Chord, String> {
        let parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let (key_name, mods) = parts.split_last().ok_or_else(|| "combinación vacía".to_string())?;
        let mut chord = Chord { ctrl: false, alt: false, shift: false, key: Key::A };
        for m in mods {
            match m.to_ascii_lowercase().as_str() {
                "ctrl" | "cmd" => chord.ctrl = true,
                "alt" => chord.alt = true,
                "shift" => chord.shift = true,
                other => return Err(format!("«{other}» no es un modificador")),
            }
        }
        chord.key = key_from_name(key_name).ok_or_else(|| format!("«{text}»: tecla desconocida"))?;
        Ok(chord)
    }

    /// Texto para `keymap.toml`, en orden fijo: `Ctrl+Alt+Shift+Tecla`.
    pub(crate) fn to_config(&self) -> String {
        let mut out = String::new();
        for (on, name) in [(self.ctrl, "Ctrl+"), (self.alt, "Alt+"), (self.shift, "Shift+")] {
            if on {
                out.push_str(name);
            }
        }
        out.push_str(self.key.name());
        out
    }

    /// Texto para la interfaz: `Ctrl+Mayús+R` o, en macOS, `⌘⇧R`.
    pub(crate) fn label(&self, mac: bool) -> String {
        let key = key_label(self.key);
        if mac {
            let mut out = String::new();
            for (on, sym) in [(self.ctrl, "⌘"), (self.alt, "⌥"), (self.shift, "⇧")] {
                if on {
                    out.push_str(sym);
                }
            }
            out + key
        } else {
            let mut out = String::new();
            for (on, name) in [(self.ctrl, "Ctrl+"), (self.alt, "Alt+"), (self.shift, "Mayús+")] {
                if on {
                    out.push_str(name);
                }
            }
            out + key
        }
    }

    pub(crate) fn modifiers(&self) -> Modifiers {
        let mut m = Modifiers::NONE;
        if self.ctrl {
            m = m | Modifiers::COMMAND;
        }
        if self.alt {
            m = m | Modifiers::ALT;
        }
        if self.shift {
            m = m | Modifiers::SHIFT;
        }
        m
    }

    /// Tiene algún modificador; Mayús cuenta.
    pub(crate) fn has_modifier(&self) -> bool {
        self.ctrl || self.alt || self.shift
    }

    /// La tecla se pulsó en este cuadro con exactamente estos modificadores.
    pub(crate) fn pressed(&self, i: &InputState) -> bool {
        i.key_pressed(self.key) && i.modifiers.matches_exact(self.modifiers())
    }
}

/// `Key::from_name` sin distinguir mayúsculas.
fn key_from_name(name: &str) -> Option<Key> {
    Key::from_name(name).or_else(|| Key::ALL.iter().copied().find(|k| k.name().eq_ignore_ascii_case(name)))
}

fn key_label(key: Key) -> &'static str {
    match key {
        Key::Delete => "Supr",
        Key::Backspace => "Retroceso",
        Key::Plus => "+",
        Key::Equals => "=",
        Key::Minus => "-",
        Key::OpenBracket => "[",
        Key::CloseBracket => "]",
        Key::Escape => "Esc",
        Key::Enter => "Intro",
        Key::Space => "Espacio",
        other => other.symbol_or_name(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_y_vuelta() {
        for s in ["Ctrl+Shift+R", "Ctrl+Alt+0", "Shift+L", "M", "Delete", "Ctrl+Plus", "Ctrl+Equals", "OpenBracket"] {
            assert_eq!(Chord::parse(s).unwrap().to_config(), s);
        }
    }

    #[test]
    fn orden_y_mayusculas_indiferentes() {
        assert_eq!(Chord::parse("shift+ctrl+r").unwrap(), Chord::parse("Ctrl+Shift+R").unwrap());
    }

    #[test]
    fn invalidas() {
        for s in ["", "Ctrl+", "Ctrl+Nada", "Hyper+R", "Ctrl+R+T"] {
            assert!(Chord::parse(s).is_err(), "{s}");
        }
    }

    #[test]
    fn etiquetas() {
        let c = Chord::parse("Ctrl+Shift+R").unwrap();
        assert_eq!(c.label(false), "Ctrl+Mayús+R");
        assert_eq!(c.label(true), "⌘⇧R");
        assert_eq!(Chord::parse("Delete").unwrap().label(false), "Supr");
        assert_eq!(Chord::parse("Backspace").unwrap().label(false), "Retroceso");
        assert_eq!(Chord::parse("Ctrl+Plus").unwrap().label(false), "Ctrl++");
        assert_eq!(Chord::parse("Ctrl+Alt+0").unwrap().label(false), "Ctrl+Alt+0");
    }

    #[test]
    fn mayus_es_modificador() {
        assert!(Chord::parse("Shift+L").unwrap().has_modifier());
        assert!(!Chord::parse("M").unwrap().has_modifier());
    }
}
