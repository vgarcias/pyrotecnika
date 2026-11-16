//! Deshacer/rehacer con copias completas del documento, limitado por memoria.

use crate::document::Document;

const MEMORY_BUDGET: usize = 768 * 1024 * 1024;
const MAX_STEPS: usize = 50;

pub struct History {
    undo: Vec<Document>,
    redo: Vec<Document>,
    /// Posición en la línea de cambios: sube al modificar o rehacer y baja
    /// al deshacer. No depende de cuántos pasos siguen en memoria.
    position: u64,
    /// Posición del último guardado, o `None` si ya no se puede volver a él.
    saved: Option<u64>,
}

impl Default for History {
    fn default() -> Self {
        Self { undo: Vec::new(), redo: Vec::new(), position: 0, saved: Some(0) }
    }
}

fn doc_bytes(doc: &Document) -> usize {
    let px = (doc.width * doc.height) as usize;
    px * 4 * doc.layers.len() + doc.selection.as_ref().map_or(0, |_| px)
}

impl History {
    /// Guarda el estado actual antes de modificarlo.
    pub fn push(&mut self, doc: &Document) {
        self.undo.push(doc.clone());
        self.redo.clear();
        // El guardado quedaba en la rama de rehacer, que se acaba de perder.
        if self.saved.is_some_and(|s| s > self.position) {
            self.saved = None;
        }
        self.position += 1;
        let limit = (MEMORY_BUDGET / doc_bytes(doc).max(1)).clamp(3, MAX_STEPS);
        if self.undo.len() > limit {
            let excess = self.undo.len() - limit;
            self.undo.drain(..excess);
        }
    }

    pub fn undo(&mut self, doc: &mut Document) -> bool {
        match self.undo.pop() {
            Some(prev) => {
                self.redo.push(std::mem::replace(doc, prev));
                self.position -= 1;
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self, doc: &mut Document) -> bool {
        match self.redo.pop() {
            Some(next) => {
                self.undo.push(std::mem::replace(doc, next));
                self.position += 1;
                true
            }
            None => false,
        }
    }

    /// Descarta el último estado guardado (p. ej. al cancelar un filtro).
    pub fn discard_last(&mut self) {
        if self.undo.pop().is_some() {
            self.position -= 1;
        }
    }

    /// Vacía el historial y da el documento por guardado (recién abierto o creado).
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Marca el estado actual como guardado.
    pub fn mark_saved(&mut self) {
        self.saved = Some(self.position);
    }

    /// Indica si hay cambios desde el último guardado.
    pub fn is_modified(&self) -> bool {
        self.saved != Some(self.position)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_roundtrip() {
        let mut h = History::default();
        let mut doc = Document::new(2, 2, None);
        h.push(&doc);
        doc.add_layer();
        assert!(h.undo(&mut doc));
        assert_eq!(doc.layers.len(), 1);
        assert!(h.redo(&mut doc));
        assert_eq!(doc.layers.len(), 2);
        assert!(!h.redo(&mut doc));
        h.push(&doc);
        assert!(!h.can_redo());
        h.discard_last();
        assert!(h.can_undo());
    }

    #[test]
    fn modified_follows_the_saved_position() {
        let mut h = History::default();
        let mut doc = Document::new(2, 2, None);
        assert!(!h.is_modified());
        h.push(&doc);
        doc.add_layer();
        assert!(h.is_modified());
        // Deshacer hasta el estado guardado deja el documento limpio.
        h.undo(&mut doc);
        assert!(!h.is_modified());
        h.redo(&mut doc);
        h.mark_saved();
        assert!(!h.is_modified());
        // Cancelar un filtro (push + discard) no cuenta como cambio.
        h.push(&doc);
        assert!(h.is_modified());
        h.discard_last();
        assert!(!h.is_modified());
    }

    #[test]
    fn saved_state_in_a_lost_redo_branch_stays_modified() {
        let mut h = History::default();
        let mut doc = Document::new(2, 2, None);
        h.push(&doc);
        doc.add_layer();
        h.mark_saved();
        h.undo(&mut doc);
        h.push(&doc);
        doc.add_layer();
        assert!(h.is_modified());
        h.clear();
        assert!(!h.is_modified());
    }
}
