//! Estado raíz de la interfaz y bucle principal.
//!
//! Cada submódulo añade métodos a [`PyroApp`] para un área de la interfaz.

mod canvas;
mod dialogs;
mod guard;
pub(crate) mod icons;
mod input;
mod panels;
mod tools;
mod updates;
mod view;

use std::path::PathBuf;

use eframe::egui::{self, TextureHandle, TextureOptions, Vec2};

use pyro_core::document::Document;
use pyro_core::filters::{self, Filter};
use pyro_core::history::History;
use pyro_core::paint::BrushParams;
use pyro_core::selection::{Mask, SelectMode};
use pyro_formats::fwpng::FwInfo;
use pyro_formats::io;

use dialogs::{ExportDialog, FilterDialog, NewDialog};
use guard::{Pending, Status, Tone};
use tools::{Drag, Tool};
use updates::UpdateState;

#[derive(Clone, Copy)]
pub(super) enum Action {
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
    canvas_tex: Option<TextureHandle>,
    canvas_opts: TextureOptions,
    canvas_dirty: bool,
    sel_tex: Option<TextureHandle>,
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
}

impl PyroApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::design::fonts::install_fonts(&cc.egui_ctx);
        let ctx = cc.egui_ctx.clone();
        pyro_platform::open_files::set_waker(move || ctx.request_repaint());
        Self {
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
            canvas_tex: None,
            canvas_opts: TextureOptions::NEAREST,
            canvas_dirty: true,
            sel_tex: None,
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
        }
    }

    // ---------- Archivos ----------

    pub(super) fn open_path(&mut self, path: PathBuf, ctx: &egui::Context) {
        match io::open_path(&path) {
            Ok(opened) => {
                let max = ctx.input(|i| i.max_texture_side) as u32;
                let doc = opened.document;
                if doc.width > max || doc.height > max {
                    self.fail(format!(
                        "«{}» mide {}×{} px y el máximo que admite esta tarjeta gráfica es {max} px por lado. \
                         Redúcela en otro programa antes de abrirla.",
                        guard::short_name(&path),
                        doc.width,
                        doc.height
                    ));
                    return;
                }
                let message = match opened.png_info.as_ref().filter(|i| i.is_fireworks()) {
                    Some(info) => format!(
                        "Archivo de Fireworks abierto como imagen aplanada ({} bloques privados, {} KB no editables).",
                        info.private_chunks().count(),
                        info.private_bytes() / 1024
                    ),
                    None => format!("Abierto: {} ({}×{} px)", path.display(), doc.width, doc.height),
                };
                self.info(message);
                self.reset_document(doc);
                self.png_info = opened.png_info;
                self.source_path = Some(path);
            }
            Err(e) => {
                // El documento actual no se toca: no hay nada que recuperar.
                let reason = e
                    .chain()
                    .find_map(|cause| cause.downcast_ref::<std::io::Error>())
                    .map_or_else(|| e.to_string(), |io| guard::io_reason(io, false));
                self.fail(format!("No se pudo abrir «{}»: {reason}.", guard::short_name(&path)));
            }
        }
    }

    pub(super) fn reset_document(&mut self, doc: Document) {
        self.doc = doc;
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

    pub(super) fn run(&mut self, action: Action, ctx: &egui::Context) {
        if self.filter_dialog.is_some()
            && !matches!(action, Action::ZoomIn | Action::ZoomOut | Action::Fit | Action::ActualSize)
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
                    self.canvas_dirty = true;
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
                    self.filter_dialog = Some(FilterDialog { filter, backup, dirty: true });
                } else {
                    let out = filters::apply_masked(&backup, &filter.apply(&backup), self.doc.selection.as_ref());
                    self.doc.active_layer_mut().pixels = out;
                    self.canvas_dirty = true;
                    self.info(format!("{} aplicado.", filter.name()));
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
        }
    }

    pub(super) fn after_history(&mut self) {
        self.drag = Drag::None;
        self.doc.active = self.doc.active.min(self.doc.layers.len() - 1);
        self.mark_dirty();
    }

    pub(super) fn mark_dirty(&mut self) {
        self.canvas_dirty = true;
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
        self.confirm_window(ctx);
        self.sync_title(ctx);
        if self.canvas_dirty || self.sel_dirty {
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
