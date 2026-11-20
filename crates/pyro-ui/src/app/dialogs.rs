//! Ventanas de diálogo: filtros con vista previa, exportación y documento nuevo.

use std::sync::Arc;

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use image::RgbaImage;

use pyro_core::filters::Filter;
use pyro_formats::export::{self, ExportFormat};
use pyro_formats::fwpng::FwInfo;
use pyro_formats::io;
use pyro_services::jobs::{Job, JobError, JobPoll, Wake};

use super::canvas_tiles::Dirty;
use super::{format_size, guard, tasks, Pending, PyroApp, Tone};
use crate::design::widgets;

pub(super) struct FilterDialog {
    pub(super) filter: Filter,
    /// La capa activa antes del filtro (lo que restaura «Cancelar»).
    pub(super) backup: Arc<RgbaImage>,
    /// Hay que calcular una vista previa nueva con `filter`.
    pub(super) dirty: bool,
    /// Vista previa en curso y sus parámetros.
    pub(super) preview: Option<(Filter, Job<RgbaImage>)>,
    /// Parámetros de lo que muestra ahora el lienzo.
    pub(super) shown: Option<Filter>,
    /// Se pulsó «Aplicar»: se cierra en cuanto llegue la vista previa vigente.
    pub(super) apply_requested: bool,
}

impl FilterDialog {
    pub(super) fn new(filter: Filter, backup: Arc<RgbaImage>) -> Self {
        Self { filter, backup, dirty: true, preview: None, shown: None, apply_requested: false }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ExportKind {
    Png24,
    Png8,
    Jpeg,
    Gif,
    WebP,
}

pub(super) struct ExportDialog {
    pub(super) source: Arc<RgbaImage>,
    pub(super) kind: ExportKind,
    pub(super) png_colors: u16,
    pub(super) jpeg_quality: u8,
    pub(super) webp_lossless: bool,
    pub(super) webp_quality: f32,
    pub(super) dirty: bool,
    pub(super) job: Option<Job<(Vec<u8>, RgbaImage)>>,
    /// Se canceló el último cálculo: no hay resultado hasta cambiar algo.
    pub(super) cancelled: bool,
    pub(super) bytes: Option<Vec<u8>>,
    pub(super) error: Option<String>,
    /// Por qué falló el último intento de guardar, visible en el propio diálogo.
    pub(super) save_error: Option<String>,
    pub(super) preview: Option<TextureHandle>,
}

impl ExportDialog {
    pub(super) fn new(source: RgbaImage) -> Self {
        Self {
            source: Arc::new(source),
            kind: ExportKind::Png24,
            png_colors: 256,
            jpeg_quality: 80,
            webp_lossless: false,
            webp_quality: 80.0,
            dirty: true,
            job: None,
            cancelled: false,
            bytes: None,
            error: None,
            save_error: None,
            preview: None,
        }
    }

    pub(super) fn format(&self) -> ExportFormat {
        match self.kind {
            ExportKind::Png24 => ExportFormat::Png24,
            ExportKind::Png8 => ExportFormat::Png8 { colors: self.png_colors },
            ExportKind::Jpeg => ExportFormat::Jpeg { quality: self.jpeg_quality },
            ExportKind::Gif => ExportFormat::Gif,
            ExportKind::WebP => ExportFormat::WebP { lossless: self.webp_lossless, quality: self.webp_quality },
        }
    }

    /// Codifica en segundo plano (lo anterior en curso se cancela).
    pub(super) fn start_job(&mut self, wake: Wake) {
        let source = self.source.clone();
        let format = self.format();
        self.job = Some(Job::spawn(wake, move |ctx| {
            let bytes = export::encode(&source, format).map_err(|e| JobError::Failed(e.to_string()))?;
            ctx.check()?;
            let preview = image::load_from_memory(&bytes).map_err(|e| JobError::Failed(e.to_string()))?.to_rgba8();
            Ok((bytes, preview))
        }));
        self.dirty = false;
        self.cancelled = false;
    }

    /// Detiene el cálculo en curso; el diálogo queda abierto sin resultado.
    pub(super) fn cancel_job(&mut self) {
        if let Some(job) = self.job.take() {
            job.cancel();
            self.cancelled = true;
            self.bytes = None;
            self.preview = None;
        }
    }
}

pub(super) struct NewDialog {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) transparent: bool,
}

impl PyroApp {
    pub(super) fn save_export(&mut self, ctx: &egui::Context) {
        let Some(dlg) = &self.export_dialog else {
            return;
        };
        let Some(bytes) = &dlg.bytes else { return };
        let format = dlg.format();
        let ext = format.extension();
        let stem = self
            .source_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().split('.').next().unwrap_or("imagen").to_string())
            .unwrap_or_else(|| "imagen".into());
        let Some(path) = rfd::FileDialog::new()
            .add_filter(format.label(), &[ext])
            .set_file_name(format!("{stem}.{ext}"))
            .save_file()
        else {
            return;
        };
        let is_fw_source = self.png_info.as_ref().is_some_and(FwInfo::is_fireworks);
        let failure = if is_fw_source && self.source_path.as_deref() == Some(path.as_path()) {
            Some(
                "No se sobrescribe el archivo de Fireworks original: perdería sus capas. Elige otro nombre."
                    .to_string(),
            )
        } else {
            std::fs::write(&path, bytes).err().map(|e| {
                format!(
                    "No se pudo guardar «{}»: {}. Elige otra ubicación.",
                    guard::short_name(&path),
                    guard::io_reason(&e, true)
                )
            })
        };
        if let Some(message) = failure {
            if let Some(dlg) = &mut self.export_dialog {
                dlg.save_error = Some(message.clone());
            }
            self.fail(message);
            return;
        }
        self.info(format!("Exportado {} ({})", path.display(), format_size(bytes.len())));
        self.history.mark_saved();
        if let Some(dlg) = &mut self.export_dialog {
            dlg.save_error = None;
        }
        // Si la exportación venía de «Exportar primero», sigue con lo pendiente.
        if let Some(pending) = self.after_export.take() {
            self.export_dialog = None;
            self.perform(pending, ctx);
        }
    }

    pub(super) fn filter_window(&mut self, ctx: &egui::Context) {
        let Some(dlg) = &mut self.filter_dialog else {
            return;
        };
        if let Some((filter, job)) = &mut dlg.preview {
            match job.poll() {
                JobPoll::Running => {}
                JobPoll::Done(img) => {
                    let active = self.doc.active;
                    self.doc.layers[active].pixels = Arc::new(img);
                    self.dirty = Dirty::All;
                    dlg.shown = Some(*filter);
                    dlg.preview = None;
                }
                JobPoll::Failed(_) | JobPoll::Cancelled => dlg.preview = None,
            }
        }
        let mut apply = false;
        let mut cancel = false;
        let mut changed = false;
        let scope = format!(
            "Capa «{}» · {}",
            self.doc.active_layer().name,
            if self.doc.selection.is_some() { "solo dentro de la selección" } else { "toda la capa" }
        );
        egui::Window::new(dlg.filter.name()).collapsible(false).resizable(false).show(ctx, |ui| {
            ui.label(scope);
            ui.add_space(4.0);
            let mut slider = |ui: &mut egui::Ui, v: &mut f32, range: std::ops::RangeInclusive<f32>, label: &str| {
                changed |= ui.add(egui::Slider::new(v, range).text(label)).changed();
            };
            match &mut dlg.filter {
                Filter::BrightnessContrast { brightness, contrast } => {
                    slider(ui, brightness, -100.0..=100.0, "Brillo");
                    slider(ui, contrast, -100.0..=100.0, "Contraste");
                }
                Filter::HueSaturation { hue, saturation, lightness } => {
                    slider(ui, hue, -180.0..=180.0, "Tono");
                    slider(ui, saturation, -100.0..=100.0, "Saturación");
                    slider(ui, lightness, -100.0..=100.0, "Luminosidad");
                }
                Filter::Levels { in_black, in_white, gamma, out_black, out_white } => {
                    ui.label("Entrada");
                    slider(ui, in_black, 0.0..=254.0, "Negro");
                    slider(ui, gamma, 0.1..=9.99, "Gamma");
                    slider(ui, in_white, 1.0..=255.0, "Blanco");
                    *in_white = in_white.max(*in_black + 1.0);
                    ui.label("Salida");
                    slider(ui, out_black, 0.0..=255.0, "Negro");
                    slider(ui, out_white, 0.0..=255.0, "Blanco");
                }
                Filter::GaussianBlur { sigma } => slider(ui, sigma, 0.1..=50.0, "Radio"),
                Filter::Sharpen { sigma, amount } => {
                    slider(ui, sigma, 0.1..=10.0, "Radio");
                    slider(ui, amount, 0.0..=5.0, "Cantidad");
                }
                Filter::Invert | Filter::Grayscale => {}
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if dlg.apply_requested {
                    ui.add_enabled_ui(false, |ui| widgets::primary_button(ui, "Aplicando…"));
                } else {
                    apply = widgets::primary_button(ui, "Aplicar").clicked();
                }
                cancel = widgets::button(ui, "Cancelar").clicked();
                if dlg.preview.is_some() {
                    ui.spinner();
                }
            });
        });
        if changed {
            dlg.dirty = true;
        }
        // Esc es «Cancelar» mientras el diálogo está abierto.
        cancel |= ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if apply {
            dlg.apply_requested = true;
        }
        if dlg.dirty {
            // Lo que se calculaba ya no vale: se suelta (y se cancela) y se lanza otra.
            let (filter, source, sel) = (dlg.filter, Arc::clone(&dlg.backup), self.doc.selection.clone());
            let job = Job::spawn(tasks::wake(ctx), move |monitor| Ok(filter.run(&source, sel.as_ref(), monitor)?));
            dlg.preview = Some((filter, job));
            dlg.dirty = false;
        }
        let current = dlg.filter;
        let ready = dlg.preview.is_none() && dlg.shown == Some(current);
        if cancel {
            let active = self.doc.active;
            self.doc.layers[active].pixels = Arc::clone(&dlg.backup);
            self.history.discard_last();
            self.filter_dialog = None;
            self.dirty = Dirty::All;
        } else if dlg.apply_requested && ready {
            self.history.seal(&self.doc);
            self.filter_dialog = None;
            self.info(format!("{} aplicado.", current.name()));
        }
    }

    pub(super) fn export_window(&mut self, ctx: &egui::Context) {
        let after_hint = self.after_export_hint();
        let Some(dlg) = &mut self.export_dialog else {
            return;
        };
        if let Some(job) = &mut dlg.job {
            match job.poll() {
                JobPoll::Running => {}
                JobPoll::Done((bytes, preview)) => {
                    dlg.job = None;
                    let img = ColorImage::from_rgba_unmultiplied(
                        [preview.width() as usize, preview.height() as usize],
                        preview.as_raw(),
                    );
                    dlg.preview = Some(ctx.load_texture("vista-exportacion", img, TextureOptions::NEAREST));
                    dlg.bytes = Some(bytes);
                    dlg.error = None;
                }
                JobPoll::Failed(e) => {
                    dlg.job = None;
                    dlg.bytes = None;
                    dlg.error = Some(e);
                }
                JobPoll::Cancelled => dlg.job = None,
            }
        }
        if dlg.dirty {
            // Un cambio de formato o calidad deja viejo lo que se esté calculando.
            dlg.start_job(tasks::wake(ctx));
        }

        let mut open = true;
        let mut save = false;
        egui::Window::new("Exportar").open(&mut open).default_width(460.0).show(ctx, |ui| {
            let before = (dlg.kind, dlg.png_colors, dlg.jpeg_quality, dlg.webp_lossless, dlg.webp_quality);
            ui.horizontal(|ui| {
                ui.label("Formato");
                egui::ComboBox::from_id_salt("formato").selected_text(dlg.format().label()).show_ui(ui, |ui| {
                    ui.selectable_value(&mut dlg.kind, ExportKind::Png24, "PNG-24 (sin pérdida)");
                    ui.selectable_value(&mut dlg.kind, ExportKind::Png8, "PNG-8 (paleta)");
                    ui.selectable_value(&mut dlg.kind, ExportKind::Jpeg, "JPEG");
                    ui.selectable_value(&mut dlg.kind, ExportKind::Gif, "GIF (256 colores)");
                    ui.selectable_value(&mut dlg.kind, ExportKind::WebP, "WebP");
                });
            });
            match dlg.kind {
                ExportKind::Png8 => {
                    ui.add(egui::Slider::new(&mut dlg.png_colors, 16..=256).text("Colores"));
                }
                ExportKind::Jpeg => {
                    ui.add(egui::Slider::new(&mut dlg.jpeg_quality, 1..=100).text("Calidad"));
                    ui.small("JPEG no admite transparencia: se rellena de blanco.");
                }
                ExportKind::WebP => {
                    ui.checkbox(&mut dlg.webp_lossless, "Sin pérdida");
                    ui.add_enabled(
                        !dlg.webp_lossless,
                        egui::Slider::new(&mut dlg.webp_quality, 0.0..=100.0).text("Calidad"),
                    );
                }
                _ => {}
            }
            let after = (dlg.kind, dlg.png_colors, dlg.jpeg_quality, dlg.webp_lossless, dlg.webp_quality);
            if before != after {
                dlg.dirty = true;
            }

            ui.separator();
            let raw = dlg.source.width() as usize * dlg.source.height() as usize * 4;
            let mut cancel = false;
            if dlg.job.is_some() || dlg.dirty {
                ui.horizontal(|ui| {
                    ui.add(egui::ProgressBar::new(0.0).desired_width(160.0).animate(true));
                    ui.label("Calculando…");
                    cancel = widgets::button(ui, "Cancelar").clicked();
                });
            } else if dlg.cancelled {
                ui.label("Cálculo cancelado. Cambia el formato o la calidad para calcularlo de nuevo.");
            } else if let Some(e) = &dlg.error {
                guard::notice(ui, Tone::Error, &format!("No se pudo codificar la imagen: {e}."));
            } else if let Some(bytes) = &dlg.bytes {
                ui.horizontal(|ui| {
                    ui.label("Tamaño:");
                    ui.monospace(format_size(bytes.len()));
                    ui.label(format!("({:.1} % del original sin comprimir)", bytes.len() as f64 / raw as f64 * 100.0));
                });
            }
            if let Some(tex) = &dlg.preview {
                ui.label("Vista previa al 100 % (desplázate para revisar la calidad):");
                egui::ScrollArea::both().max_height(340.0).max_width(440.0).show(ui, |ui| {
                    ui.add(egui::Image::from_texture(egui::load::SizedTexture::from_handle(tex)));
                });
            }
            ui.separator();
            if cancel {
                dlg.cancel_job();
            }
            let ready = dlg.bytes.is_some() && dlg.job.is_none() && !dlg.dirty;
            if let Some(e) = &dlg.save_error {
                guard::notice(ui, Tone::Error, e);
            }
            if let Some(hint) = &after_hint {
                ui.label(hint);
            }
            if ui.add_enabled_ui(ready, |ui| widgets::primary_button(ui, "Guardar…")).inner.clicked() {
                save = true;
            }
        });
        if !open {
            self.export_dialog = None;
            self.after_export = None;
        } else if save {
            self.save_export(ctx);
        }
    }

    pub(super) fn new_window(&mut self, ctx: &egui::Context) {
        let Some(dlg) = &mut self.new_dialog else {
            return;
        };
        let mut create = false;
        let mut open = true;
        egui::Window::new("Nuevo documento").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            egui::Grid::new("nuevo").show(ui, |ui| {
                ui.label("Ancho");
                ui.add(egui::DragValue::new(&mut dlg.width).range(1..=io::MAX_SIDE).suffix(" px"));
                ui.end_row();
                ui.label("Alto");
                ui.add(egui::DragValue::new(&mut dlg.height).range(1..=io::MAX_SIDE).suffix(" px"));
                ui.end_row();
            });
            ui.checkbox(&mut dlg.transparent, "Fondo transparente");
            let too_large = io::check_size(dlg.width, dlg.height).err();
            if let Some(e) = &too_large {
                guard::notice(ui, Tone::Warn, &e.to_string());
            }
            create = ui.add_enabled_ui(too_large.is_none(), |ui| widgets::primary_button(ui, "Crear")).inner.clicked();
        });
        if create {
            let pending = Pending::New { width: dlg.width, height: dlg.height, transparent: dlg.transparent };
            self.new_dialog = None;
            self.request(pending, ctx);
        } else if !open {
            self.new_dialog = None;
        }
    }
}
