//! Estado de cada documento abierto (una pestaña): el documento y todo lo
//! que es solo suyo. Lo común (herramienta, pincel, tema, atajos…) vive en
//! `PyroApp`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use eframe::egui::Vec2;

use pyro_core::document::Document;
use pyro_core::history::History;
use pyro_formats::fwpng::FwInfo;

use super::autosave::AutosaveTask;
use super::canvas_tiles::{Dirty, Tiles};
use super::dialogs::{ExportDialog, FilterDialog};
use super::save::SaveTask;
use super::tasks::DocTask;
use super::tools::Drag;
use pyro_services::recovery::RecoveryEntry;

pub(super) struct Tab {
    /// Identidad única en la sesión (autoguardado, tareas).
    pub(super) uid: u64,
    /// Número de «Sin título N», o 0 si el documento viene de un archivo.
    pub(super) untitled: u32,
    /// Nombre a mostrar en vez del del archivo («x.png (recuperado)»).
    pub(super) label: Option<String>,
    pub(super) doc: Document,
    pub(super) history: History,
    pub(super) clone_source: Option<(f32, f32)>,
    /// Desplazamiento origen − pincel del trazo de tampón en curso.
    pub(super) clone_stroke_offset: Option<(f32, f32)>,
    pub(super) zoom: f32,
    pub(super) pan: Vec2,
    pub(super) fit_pending: bool,
    pub(super) canvas: Tiles,
    pub(super) dirty: Dirty,
    pub(super) sel_tiles: Tiles,
    pub(super) sel_dirty: bool,
    pub(super) drag: Drag,
    pub(super) filter_dialog: Option<FilterDialog>,
    pub(super) export_dialog: Option<ExportDialog>,
    pub(super) source_path: Option<PathBuf>,
    /// Dónde se guarda con Guardar (Ctrl+S); `None` hasta el primer guardado
    /// salvo que se abriera un `.ora`.
    pub(super) ora_path: Option<PathBuf>,
    /// Guardado en curso (no bloquea la edición).
    pub(super) save: Option<SaveTask>,
    /// Posición del historial del último autoguardado.
    pub(super) autosaved_position: Option<u64>,
    /// Autoguardado en curso.
    pub(super) autosave: Option<AutosaveTask>,
    /// Autoguardado de una sesión anterior del que se recuperó la pestaña;
    /// se borra en cuanto esta sesión la autoguarda, guarda o cierra.
    pub(super) recovered_from: Option<RecoveryEntry>,
    pub(super) png_info: Option<FwInfo>,
    /// Tarea en segundo plano que va a cambiar el documento (abrir, filtro).
    pub(super) task: Option<DocTask>,
    /// Sube cada vez que se abre o crea un documento en la pestaña: el
    /// resultado de una tarea lanzada sobre otro se descarta.
    pub(super) generation: u64,
}

impl Tab {
    pub(super) fn new(doc: Document, untitled: u32) -> Tab {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Tab {
            uid: NEXT.fetch_add(1, Ordering::Relaxed),
            untitled,
            label: None,
            doc,
            history: History::default(),
            clone_source: None,
            clone_stroke_offset: None,
            zoom: 1.0,
            pan: Vec2::ZERO,
            fit_pending: true,
            canvas: Tiles::new("lienzo"),
            dirty: Dirty::All,
            sel_tiles: Tiles::new("seleccion"),
            sel_dirty: true,
            drag: Drag::None,
            filter_dialog: None,
            export_dialog: None,
            source_path: None,
            ora_path: None,
            save: None,
            autosaved_position: None,
            autosave: None,
            recovered_from: None,
            png_info: None,
            task: None,
            generation: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::scenes::{scene, Scene};
    use crate::design::style::ThemeChoice;
    use eframe::egui;
    use pyro_core::document::Document;

    #[test]
    fn una_pestana_al_arrancar() {
        let h = egui_kittest::Harness::builder().build_eframe(|cc| crate::app::PyroApp::with_config(cc, None));
        assert_eq!(h.state().tab.untitled, 1);
        assert!(!h.state().tab.history.can_undo());
        let other = Tab::new(Document::new(4, 4, None), 2);
        assert_ne!(other.uid, h.state().tab.uid);
        let _ = scene(Scene::Main, ThemeChoice::Dark, 1.0, egui::vec2(800.0, 600.0));
    }
}
