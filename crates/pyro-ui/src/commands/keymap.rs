//! Mapa de atajos vigente: los de fábrica más las personalizaciones del
//! usuario, con detección de conflictos y el archivo `keymap.toml`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::catalog::{find, Category, CATALOG, PLAIN_KEY_DEFAULTS};
use super::chord::Chord;
use crate::app::tools::Tool;
use crate::app::Action;

/// Atajos de cada comando, en el mismo orden que [`CATALOG`].
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct Keymap {
    bindings: Vec<Vec<Chord>>,
}

/// Contenido de `keymap.toml`: solo los comandos que difieren de fábrica.
/// Una lista vacía significa «sin atajo».
#[derive(Serialize, Deserialize, Default, PartialEq, Debug)]
pub(crate) struct KeymapFile {
    #[serde(default)]
    pub schema: u32,
    #[serde(default)]
    pub bindings: BTreeMap<String, Vec<String>>,
}

fn index(id: &str) -> Option<usize> {
    CATALOG.iter().position(|c| c.id == id)
}

fn is_tool(i: usize) -> bool {
    CATALOG[i].category == Category::Tools
}

fn defaults(i: usize) -> Vec<Chord> {
    CATALOG[i].defaults.iter().filter_map(|d| Chord::parse(d).ok()).collect()
}

/// Dos comandos pueden compartir esta combinación: herramientas con una
/// tecla sin modificador (alternan).
fn may_share(a: usize, b: usize, c: Chord) -> bool {
    is_tool(a) && is_tool(b) && !c.has_modifier()
}

impl Keymap {
    pub(crate) fn fireworks() -> Keymap {
        Keymap { bindings: (0..CATALOG.len()).map(defaults).collect() }
    }

    pub(crate) fn chords(&self, id: &str) -> &[Chord] {
        index(id).map_or(&[], |i| &self.bindings[i])
    }

    /// Primer atajo del comando para mostrar en menús, o `""`.
    pub(crate) fn label(&self, id: &str, mac: bool) -> String {
        self.chords(id).first().map(|c| c.label(mac)).unwrap_or_default()
    }

    /// Una tecla sin modificador solo vale para herramientas y para los
    /// comandos que la tienen de fábrica (Supr, Retroceso, [ y ]).
    pub(crate) fn allowed(id: &str, c: Chord) -> bool {
        if c.has_modifier() {
            return true;
        }
        let Some(def) = find(id) else { return false };
        def.category == Category::Tools
            || (PLAIN_KEY_DEFAULTS.contains(&c.key.name()) && def.defaults.contains(&c.to_config().as_str()))
    }

    /// Otro comando que ya usa `c` (salvo herramientas que comparten tecla).
    #[allow(dead_code)] // Temporal: lo usa el editor de atajos (tarea 8).
    pub(crate) fn conflict(&self, id: &str, c: Chord) -> Option<&'static str> {
        let me = index(id)?;
        (0..CATALOG.len())
            .find(|&j| j != me && self.bindings[j].contains(&c) && !may_share(me, j, c))
            .map(|j| CATALOG[j].id)
    }

    /// Añade `c` a `id` y se lo quita a quien lo tuviera.
    pub(crate) fn assign(&mut self, id: &str, c: Chord) {
        let Some(me) = index(id) else { return };
        for j in 0..CATALOG.len() {
            if j != me && !may_share(me, j, c) {
                self.bindings[j].retain(|x| *x != c);
            }
        }
        if !self.bindings[me].contains(&c) {
            self.bindings[me].push(c);
        }
    }

    #[allow(dead_code)] // Temporal: lo usa el editor de atajos (tarea 8).
    pub(crate) fn remove(&mut self, id: &str, c: Chord) {
        if let Some(i) = index(id) {
            self.bindings[i].retain(|x| *x != c);
        }
    }

    /// Vuelve a los atajos de fábrica del comando, quitándoselos a otros.
    #[allow(dead_code)] // Temporal: lo usa el editor de atajos (tarea 8).
    pub(crate) fn reset(&mut self, id: &str) {
        let Some(i) = index(id) else { return };
        self.bindings[i].clear();
        for c in defaults(i) {
            self.assign(id, c);
        }
    }

    #[allow(dead_code)] // Temporal: lo usa el editor de atajos (tarea 8).
    pub(crate) fn is_default(&self, id: &str) -> bool {
        index(id).is_some_and(|i| self.bindings[i] == defaults(i))
    }

    /// Acción de una pulsación. Las herramientas que comparten una tecla sin
    /// modificador alternan: tras la activa, la siguiente del grupo; si la
    /// activa no está en el grupo, la primera.
    pub(crate) fn resolve(&self, c: Chord, active: Tool) -> Option<Action> {
        let hits: Vec<usize> = (0..CATALOG.len()).filter(|&i| self.bindings[i].contains(&c)).collect();
        let first = *hits.first()?;
        if hits.len() > 1 && hits.iter().all(|&i| is_tool(i)) {
            let current = hits.iter().position(|&i| CATALOG[i].action == Action::SelectTool(active));
            let next = current.map_or(0, |p| (p + 1) % hits.len());
            return Some(CATALOG[hits[next]].action);
        }
        Some(CATALOG[first].action)
    }

    #[allow(dead_code)] // Temporal: lo usa el editor de atajos (tarea 8).
    pub(crate) fn to_file(&self) -> KeymapFile {
        let bindings = (0..CATALOG.len())
            .filter(|&i| self.bindings[i] != defaults(i))
            .map(|i| (CATALOG[i].id.to_string(), self.bindings[i].iter().map(|c| c.to_config()).collect()))
            .collect();
        KeymapFile { schema: 1, bindings }
    }

    /// Reemplaza las personalizaciones por las del archivo (lo que no
    /// aparece vuelve a fábrica). Ignora y avisa de lo inválido; si dos
    /// comandos del archivo piden la misma combinación, gana el primero en
    /// orden del catálogo.
    pub(crate) fn from_file(file: &KeymapFile) -> (Keymap, Vec<String>) {
        let mut warns = Vec::new();
        for id in file.bindings.keys().filter(|id| index(id).is_none()) {
            warns.push(format!("Atajos: «{id}» no es un comando conocido; se ignora."));
        }
        let mut map = Keymap::fireworks();
        let mut claimed: Vec<(Chord, usize)> = Vec::new();
        for (i, def) in CATALOG.iter().enumerate() {
            let Some(list) = file.bindings.get(def.id) else { continue };
            map.bindings[i].clear();
            for text in list {
                let c = match Chord::parse(text) {
                    Ok(c) => c,
                    Err(e) => {
                        warns.push(format!("Atajos de «{}»: {e}; se ignora.", def.name));
                        continue;
                    }
                };
                if !Keymap::allowed(def.id, c) {
                    warns.push(format!("Atajos de «{}»: «{text}» necesita Ctrl, Alt o Mayús; se ignora.", def.name));
                    continue;
                }
                if let Some(&(_, owner)) = claimed.iter().find(|(x, j)| *x == c && !may_share(i, *j, c)) {
                    warns.push(format!(
                        "Atajos: «{text}» ya es de «{}»; se ignora en «{}».",
                        CATALOG[owner].name, def.name
                    ));
                    continue;
                }
                claimed.push((c, i));
                map.assign(def.id, c);
            }
        }
        (map, warns)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tools::Tool;
    use crate::app::Action;

    fn ch(s: &str) -> Chord {
        Chord::parse(s).unwrap()
    }

    #[test]
    fn alternancia_de_herramientas() {
        let k = Keymap::fireworks();
        assert_eq!(k.resolve(ch("M"), Tool::Brush), Some(Action::SelectTool(Tool::Marquee)));
        assert_eq!(k.resolve(ch("M"), Tool::Marquee), Some(Action::SelectTool(Tool::Ellipse)));
        assert_eq!(k.resolve(ch("M"), Tool::Ellipse), Some(Action::SelectTool(Tool::Marquee)));
        let mut k2 = k.clone();
        k2.remove("tool.marquee", ch("M"));
        k2.assign("tool.marquee", ch("Q"));
        assert_eq!(k2.resolve(ch("M"), Tool::Ellipse), Some(Action::SelectTool(Tool::Ellipse)));
        assert_eq!(k2.resolve(ch("Q"), Tool::Brush), Some(Action::SelectTool(Tool::Marquee)));
    }

    #[test]
    fn modificadores_exactos() {
        let k = Keymap::fireworks();
        assert_eq!(k.resolve(ch("Ctrl+Z"), Tool::Brush), Some(Action::Undo));
        assert_eq!(k.resolve(ch("Ctrl+Shift+Z"), Tool::Brush), Some(Action::Redo));
        assert_eq!(k.resolve(ch("Ctrl+Y"), Tool::Brush), None);
        assert_eq!(k.resolve(ch("Ctrl+E"), Tool::Brush), None);
    }

    #[test]
    fn conflictos_y_reasignar() {
        let mut k = Keymap::fireworks();
        assert_eq!(k.conflict("file.export", ch("Ctrl+K")), Some("edit.palette"));
        assert_eq!(k.conflict("tool.lasso", ch("M")), None);
        assert_eq!(k.conflict("file.export", ch("M")), Some("tool.marquee"));
        k.assign("file.export", ch("Ctrl+K"));
        assert!(k.chords("edit.palette").is_empty());
        assert_eq!(k.resolve(ch("Ctrl+K"), Tool::Brush), Some(Action::Export));
    }

    #[test]
    fn tecla_sola_solo_para_herramientas() {
        assert!(Keymap::allowed("tool.lasso", ch("Q")));
        assert!(Keymap::allowed("edit.clear", ch("Delete")));
        assert!(!Keymap::allowed("file.export", ch("X")));
        assert!(Keymap::allowed("file.export", ch("Shift+X")));
    }

    #[test]
    fn archivo_solo_con_diferencias_y_reemplazo() {
        let mut k = Keymap::fireworks();
        k.assign("file.export", ch("Ctrl+E"));
        let f = k.to_file();
        assert_eq!(f.bindings.len(), 1);
        assert_eq!(f.bindings["file.export"], vec!["Ctrl+Shift+R".to_string(), "Ctrl+E".into()]);
        let (back, warns) = Keymap::from_file(&f);
        assert_eq!(back, k);
        assert!(warns.is_empty());
        // Importar reemplaza: un archivo vacío devuelve todo a fábrica.
        assert_eq!(Keymap::from_file(&KeymapFile { schema: 1, ..Default::default() }).0, Keymap::fireworks());
    }

    #[test]
    fn archivo_editado_a_mano_con_errores() {
        let f: KeymapFile = toml::from_str(
            r#"
            schema = 1
            [bindings]
            "nada.de.nada" = ["Ctrl+J"]
            "file.export" = ["Ctrl+Nada", "X", "Ctrl+J"]
            "file.open" = ["Ctrl+J"]
        "#,
        )
        .unwrap();
        let (k, warns) = Keymap::from_file(&f);
        assert_eq!(warns.len(), 4, "{warns:?}"); // id desconocido, tecla inválida, tecla sola, conflicto
        assert_eq!(k.chords("file.open"), &[ch("Ctrl+J")]); // file.open va antes en el catálogo
        assert!(k.chords("file.export").is_empty());
    }
}
