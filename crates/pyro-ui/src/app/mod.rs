//! Estado raíz de la interfaz y bucle principal.
//!
//! Cada submódulo añade métodos a [`PyroApp`] para un área de la interfaz.

mod canvas;
#[cfg(test)]
mod canvas_tests;
mod canvas_tiles;
#[cfg(test)]
mod commands_tests;
mod dialogs;
mod guard;
pub(crate) mod icons;
mod input;
mod palette;
mod panels;
mod prefs;
#[cfg(test)]
mod scenes;
mod shortcuts;
mod tasks;
#[cfg(test)]
mod tasks_tests;
pub(crate) mod tools;
mod updates;
mod view;

use std::path::PathBuf;

use eframe::egui::{self, TextureHandle, Vec2};

use pyro_core::document::Document;
use pyro_core::filters::Filter;
use pyro_core::history::History;
use pyro_core::paint::BrushParams;
use pyro_core::selection::{Mask, SelectMode};
use pyro_formats::fwpng::FwInfo;
use pyro_formats::io;

use canvas_tiles::Dirty;
use dialogs::{ExportDialog, FilterDialog, NewDialog};
use guard::{Pending, Status, Tone};
use tools::{Drag, Tool};
use updates::UpdateState;

use crate::commands::{
    self,
    keymap::Keymap,
    store::{Settings, Store},
};
use crate::design::style::{apply_theme, step_scale, ThemeChoice};

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Action {
    New,
    Open,
    Export,
    Quit,
    Undo,
    Redo,
    SelectAll,
    Deselect,
    InvertSelection,
    CropToSelection,
    ClearSelection,
    Filter(Filter),
    ZoomIn,
    ZoomOut,
    Fit,
    ActualSize,
    CheckUpdates,
    SetTheme(ThemeChoice),
    UiScaleUp,
    UiScaleDown,
    UiScaleReset,
    LayerNew,
    LayerDuplicate,
    LayerDelete,
    LayerUp,
    LayerDown,
    LayerMergeDown,
    SelectTool(Tool),
    BrushSmaller,
    BrushBigger,
    OpenPalette,
    OpenPreferences,
    OpenRecent(usize),
    ClearRecent,
    RevealInFolder,
}

pub struct PyroApp {
    doc: Document,
    history: History,
    tool: Tool,
    brush: BrushParams,
    tolerance: u8,
    wand_contiguous: bool,
    select_mode: SelectMode,
    clone_source: Option<(f32, f32)>,
    /// Desplazamiento origen − pincel del trazo de tampón en curso.
    clone_stroke_offset: Option<(f32, f32)>,

    zoom: f32,
    pan: Vec2,
    fit_pending: bool,
    canvas: canvas_tiles::Tiles,
    dirty: Dirty,
    sel_tiles: canvas_tiles::Tiles,
    sel_dirty: bool,
    checker: Option<TextureHandle>,
    hover_px: Option<(f32, f32)>,

    drag: Drag,
    filter_dialog: Option<FilterDialog>,
    export_dialog: Option<ExportDialog>,
    new_dialog: Option<NewDialog>,

    source_path: Option<PathBuf>,
    png_info: Option<FwInfo>,
    status: Status,

    /// Acción que espera confirmación porque descartaría cambios sin exportar.
    confirm: Option<Pending>,
    confirm_focus: bool,
    /// Acción que continúa sola cuando la exportación en curso se guarda.
    after_export: Option<Pending>,
    /// El usuario ya aceptó cerrar: no se vuelve a preguntar.
    closing: bool,
    title: String,
    /// Búsqueda o instalación de actualizaciones en curso (ventana abierta).
    update: Option<UpdateState>,
    /// Ver › Tema y Ver › Tamaño de la interfaz (se guardan en `settings`).
    theme: ThemeChoice,
    ui_scale: f32,
    /// Atajos vigentes (fábrica de Fireworks más lo que cambió el usuario).
    keymap: Keymap,
    /// Preferencias y recientes tal como se guardan en `settings.toml`.
    settings: Settings,
    store: Store,
    /// Ya se avisó de que no se pudo guardar la configuración.
    save_warned: bool,
    /// Paleta de comandos abierta (Ctrl+K).
    palette: Option<palette::Palette>,
    /// Editar › Preferencias… abierta.
    prefs: Option<prefs::PrefsDialog>,
    /// Tarea en segundo plano que va a cambiar el documento (abrir, filtro).
    task: Option<tasks::DocTask>,
    /// Sube cada vez que se abre o crea un documento: el resultado de una
    /// tarea lanzada sobre otro se descarta.
    generation: u64,
}

impl PyroApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::with_config(cc, pyro_platform::paths::config_dir())
    }

    /// La app con la configuración de `dir` (`None`: no se lee ni se guarda
    /// nada, como en las pruebas).
    pub(crate) fn with_config(cc: &eframe::CreationContext<'_>, dir: Option<PathBuf>) -> Self {
        crate::design::style::install(&cc.egui_ctx);
        let ctx = cc.egui_ctx.clone();
        pyro_platform::open_files::set_waker(move || ctx.request_repaint());
        let mut store = Store::new(dir);
        let (settings, mut warns) = store.load_settings();
        let (keymap, more) = store.load_keymap();
        warns.extend(more);
        apply_theme(&cc.egui_ctx, settings.appearance.theme);
        cc.egui_ctx.set_zoom_factor(settings.appearance.ui_scale);
        let mut app = Self {
            doc: Document::new(800, 600, Some([255, 255, 255, 255])),
            history: History::default(),
            tool: Tool::Brush,
            brush: BrushParams::default(),
            tolerance: 32,
            wand_contiguous: true,
            select_mode: SelectMode::Replace,
            clone_source: None,
            clone_stroke_offset: None,
            zoom: 1.0,
            pan: Vec2::ZERO,
            fit_pending: true,
            canvas: canvas_tiles::Tiles::new("lienzo"),
            dirty: Dirty::All,
            sel_tiles: canvas_tiles::Tiles::new("seleccion"),
            sel_dirty: true,
            checker: None,
            hover_px: None,
            drag: Drag::None,
            filter_dialog: None,
            export_dialog: None,
            new_dialog: None,
            source_path: None,
            png_info: None,
            status: Status {
                tone: Tone::Info,
                text: "Listo. Abre una imagen con Archivo › Abrir o arrástrala a la ventana.".into(),
            },
            confirm: None,
            confirm_focus: false,
            after_export: None,
            closing: false,
            title: String::new(),
            update: None,
            theme: settings.appearance.theme,
            ui_scale: settings.appearance.ui_scale,
            keymap,
            settings,
            store,
            save_warned: false,
            palette: None,
            prefs: None,
            task: None,
            generation: 0,
        };
        if !warns.is_empty() {
            app.warn(warns.join(" "));
        }
        app
    }

    /// Guarda tema, escala y recientes. Si falla, avisa una sola vez.
    pub(super) fn save_settings(&mut self) {
        self.settings.appearance.theme = self.theme;
        self.settings.appearance.ui_scale = self.ui_scale;
        if let Err(e) = self.store.save_settings(&self.settings) {
            self.warn_save_failed(e);
        }
    }

    pub(super) fn warn_save_failed(&mut self, e: String) {
        if !self.save_warned {
            self.save_warned = true;
            self.warn(format!("{e}. Los cambios valen hasta cerrar Pyrotecnika."));
        }
    }

    /// Texto del atajo vigente de un comando, tal como lo muestran los menús.
    pub(super) fn menu_shortcut(&self, id: &str) -> String {
        self.keymap.label(id, commands::is_mac())
    }

    /// Si la acción tiene sentido ahora (menús y paleta la deshabilitan si no).
    pub(super) fn available(&self, action: Action) -> bool {
        if self.busy() && !action.is_view_only() {
            return false;
        }
        let layers = self.doc.layers.len();
        let active = self.doc.active;
        match action {
            Action::Undo => self.history.can_undo(),
            Action::Redo => self.history.can_redo(),
            Action::Deselect | Action::ClearSelection | Action::CropToSelection => self.doc.selection.is_some(),
            Action::UiScaleUp => self.ui_scale < 2.0,
            Action::UiScaleDown => self.ui_scale > 0.8,
            Action::UiScaleReset => self.ui_scale != 1.0,
            Action::LayerDelete => layers > 1,
            Action::LayerUp => active + 1 < layers,
            Action::LayerDown | Action::LayerMergeDown => active > 0,
            Action::ClearRecent => !self.settings.recent.files.is_empty(),
            Action::OpenRecent(i) => i < self.settings.recent.files.len(),
            Action::RevealInFolder => self.source_path.is_some(),
            _ => true,
        }
    }

    // ---------- Archivos ----------

    /// Abre `path` en segundo plano (ver [`PyroApp::start_open`]).
    pub(super) fn open_path(&mut self, path: PathBuf, ctx: &egui::Context) {
        self.start_open(path, ctx);
    }

    /// Termina de abrir: el documento ya está decodificado.
    pub(super) fn finish_open(&mut self, path: PathBuf, opened: io::Opened) {
        let doc = opened.document;
        let message = match opened.png_info.as_ref().filter(|i| i.is_fireworks()) {
            Some(info) => format!(
                "Archivo de Fireworks abierto como imagen aplanada ({} bloques privados, {} KB no editables).{}",
                info.private_chunks().count(),
                info.private_bytes() / 1024,
                if info.damaged() { " Sus datos de Fireworks están dañados." } else { "" }
            ),
            None => format!("Abierto: {} ({}×{} px)", path.display(), doc.width, doc.height),
        };
        self.info(message);
        self.reset_document(doc);
        self.png_info = opened.png_info;
        self.settings.recent.push(path.clone());
        self.save_settings();
        self.source_path = Some(path);
    }

    pub(super) fn reset_document(&mut self, doc: Document) {
        self.doc = doc;
        self.generation += 1;
        self.history.clear();
        self.drag = Drag::None;
        self.clone_source = None;
        self.filter_dialog = None;
        self.export_dialog = None;
        self.after_export = None;
        self.png_info = None;
        self.source_path = None;
        self.fit_pending = true;
        self.mark_dirty();
    }

    pub(super) fn pick_and_open(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Imágenes", &io::OPEN_EXTENSIONS)
            .add_filter("PNG de Fireworks", &["png"])
            .pick_file()
        {
            self.request(Pending::Open(path), ctx);
        }
    }

    // ---------- Acciones ----------

    /// Operación de capa con historial: la misma para botones, menú,
    /// teclado y paleta.
    pub(super) fn layer_op(&mut self, f: fn(&mut Document) -> bool) {
        let before = self.doc.clone();
        if f(&mut self.doc) {
            self.history.push(&before);
            self.dirty = Dirty::All;
        }
    }

    pub(super) fn run(&mut self, action: Action, ctx: &egui::Context) {
        if self.busy() && !action.is_view_only() {
            return;
        }
        if self.filter_dialog.is_some()
            && !matches!(
                action,
                Action::ZoomIn
                    | Action::ZoomOut
                    | Action::Fit
                    | Action::ActualSize
                    | Action::SetTheme(_)
                    | Action::UiScaleUp
                    | Action::UiScaleDown
                    | Action::UiScaleReset
            )
        {
            self.warn("Aplica o cancela el filtro abierto primero.");
            return;
        }
        match action {
            Action::New => {
                self.new_dialog = Some(NewDialog { width: 800, height: 600, transparent: false });
            }
            Action::Open => self.pick_and_open(ctx),
            Action::Export => self.export_dialog = Some(ExportDialog::new(self.doc.composite())),
            Action::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Action::Undo => {
                if self.history.undo(&mut self.doc) {
                    self.after_history();
                }
            }
            Action::Redo => {
                if self.history.redo(&mut self.doc) {
                    self.after_history();
                }
            }
            Action::SelectAll => {
                self.history.push(&self.doc);
                self.doc.selection = Some(Mask::full(self.doc.width, self.doc.height));
                self.sel_dirty = true;
            }
            Action::Deselect => {
                if self.doc.selection.is_some() {
                    self.history.push(&self.doc);
                    self.doc.selection = None;
                    self.sel_dirty = true;
                }
            }
            Action::InvertSelection => {
                self.history.push(&self.doc);
                let mut m = self.doc.selection.take().unwrap_or_else(|| Mask::empty(self.doc.width, self.doc.height));
                m.invert();
                self.doc.selection = (!m.is_empty()).then_some(m);
                self.sel_dirty = true;
            }
            Action::CropToSelection => {
                let before = self.doc.clone();
                if self.doc.crop_to_selection() {
                    self.history.push(&before);
                    self.fit_pending = true;
                    self.mark_dirty();
                } else {
                    self.warn("Recortar necesita una selección.");
                }
            }
            Action::ClearSelection => {
                if self.doc.selection.is_some() {
                    if let Some(reason) = self.layer_block() {
                        self.warn(reason);
                        return;
                    }
                    self.history.push(&self.doc);
                    self.doc.clear_selected();
                    self.dirty = Dirty::All;
                }
            }
            Action::Filter(filter) => {
                if let Some(reason) = self.layer_block() {
                    self.warn(reason);
                    return;
                }
                self.history.push(&self.doc);
                let backup = self.doc.active_layer().pixels.clone();
                if filter.has_params() {
                    self.filter_dialog = Some(FilterDialog::new(filter, backup));
                } else {
                    self.start_filter(filter, backup, ctx);
                }
            }
            Action::ZoomIn => self.zoom = (self.zoom * 1.25).min(64.0),
            Action::ZoomOut => self.zoom = (self.zoom / 1.25).max(0.02),
            Action::Fit => self.fit_pending = true,
            Action::ActualSize => {
                self.zoom = 1.0;
                self.pan = Vec2::ZERO;
            }
            Action::CheckUpdates => self.check_updates(),
            Action::SetTheme(choice) => {
                self.theme = choice;
                apply_theme(ctx, choice);
                self.save_settings();
            }
            Action::UiScaleUp | Action::UiScaleDown | Action::UiScaleReset => {
                self.ui_scale = match action {
                    Action::UiScaleUp => step_scale(self.ui_scale, 1),
                    Action::UiScaleDown => step_scale(self.ui_scale, -1),
                    _ => 1.0,
                };
                ctx.set_zoom_factor(self.ui_scale);
                self.save_settings();
            }
            Action::LayerNew => self.layer_op(|d| {
                d.add_layer();
                true
            }),
            Action::LayerDuplicate => self.layer_op(|d| {
                d.duplicate_layer();
                true
            }),
            Action::LayerDelete => self.layer_op(Document::delete_layer),
            Action::LayerUp => self.layer_op(Document::move_layer_up),
            Action::LayerDown => self.layer_op(Document::move_layer_down),
            Action::LayerMergeDown => self.layer_op(Document::merge_down),
            Action::SelectTool(tool) => self.tool = tool,
            Action::BrushSmaller => self.brush.radius = (self.brush.radius - 2.0).clamp(0.5, 250.0),
            Action::BrushBigger => self.brush.radius = (self.brush.radius + 2.0).clamp(0.5, 250.0),
            Action::OpenRecent(i) => {
                let Some(path) = self.settings.recent.files.get(i).cloned() else { return };
                if path.exists() {
                    self.request(Pending::Open(path), ctx);
                } else {
                    self.warn(format!("Ya no existe «{}»; se quitó de recientes.", guard::short_name(&path)));
                    self.settings.recent.remove(&path);
                    self.save_settings();
                }
            }
            Action::ClearRecent => {
                self.settings.recent.clear();
                self.save_settings();
            }
            Action::RevealInFolder => match &self.source_path {
                None => self.warn("El documento no viene de un archivo: no hay carpeta que mostrar."),
                Some(path) => {
                    if let Err(e) = pyro_platform::reveal::reveal(path) {
                        self.warn(format!("No se pudo abrir la carpeta: {e}."));
                    }
                }
            },
            Action::OpenPalette => self.palette = Some(palette::Palette::new()),
            Action::OpenPreferences => self.prefs = Some(prefs::PrefsDialog::new()),
        }
    }

    pub(super) fn after_history(&mut self) {
        self.drag = Drag::None;
        self.doc.active = self.doc.active.min(self.doc.layers.len() - 1);
        self.mark_dirty();
    }

    pub(super) fn mark_dirty(&mut self) {
        self.dirty = Dirty::All;
        self.sel_dirty = true;
    }
}

impl eframe::App for PyroApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Archivos de la línea de comandos o del sistema (Finder en macOS).
        // Se abren aquí, y no al crear la app, porque hasta el primer cuadro
        // no se conoce el tamaño máximo de textura de la GPU.
        for path in pyro_platform::open_files::take() {
            self.request(Pending::Open(path), ctx);
        }
        self.poll_task();
        self.guard_close(ctx);
        self.handle_input(ctx);
        self.menu_bar(ctx);
        self.status_bar(ctx);
        self.tool_panel(ctx);
        self.side_panel(ctx);
        egui::CentralPanel::default().frame(egui::Frame::default()).show(ctx, |ui| self.canvas_ui(ui));
        self.filter_window(ctx);
        self.export_window(ctx);
        self.new_window(ctx);
        self.update_window(ctx);
        self.palette_window(ctx);
        self.prefs_window(ctx);
        self.confirm_window(ctx);
        self.sync_title(ctx);
        crate::design::widgets::paint_focus_ring(ctx);
        if self.dirty != Dirty::None || self.sel_dirty {
            ctx.request_repaint();
        }
    }
}

pub(super) fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
