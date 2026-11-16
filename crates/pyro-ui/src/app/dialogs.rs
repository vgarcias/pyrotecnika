//! Ventanas de diálogo: filtros con vista previa, exportación y documento nuevo.

use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::Duration;

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use image::RgbaImage;

use pyro_core::filters::{self, Filter};
use pyro_formats::export::{self, ExportFormat};
use pyro_formats::fwpng::FwInfo;

use super::{format_size, guard, Pending, PyroApp, Tone};
use crate::design::widgets;

pub(super) struct FilterDialog {
    pub(super) filter: Filter,
    pub(super) backup: RgbaImage,
    pub(super) dirty: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ExportKind {
    Png24,
    Png8,
    Jpeg,
    Gif,
    WebP,
}

pub(super) type ExportResult = Result<(Vec<u8>, RgbaImage), String>;

pub(super) struct ExportDialog {
    pub(super) source: Arc<RgbaImage>,
    pub(super) kind: ExportKind,
    pub(super) png_colors: u16,
    pub(super) jpeg_quality: u8,
    pub(super) webp_lossless: bool,
    pub(super) webp_quality: f32,
    pub(super) dirty: bool,
    pub(super) job: Option<Receiver<ExportResult>>,
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

    pub(super) fn start_job(&mut self) {
        let (tx, rx) = channel();
        let source = self.source.clone();
        let format = self.format();
        std::thread::spawn(move || {
            let result = export::encode(&source, format).map_err(|e| e.to_string()).and_then(|bytes| {
                let preview = image::load_from_memory(&bytes).map_err(|e| e.to_string())?.to_rgba8();
                Ok((bytes, preview))
            });
            let _ = tx.send(result);
        });
        self.job = Some(rx);
        self.dirty = false;
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
                apply = widgets::primary_button(ui, "Aplicar").clicked();
                cancel = widgets::button(ui, "Cancelar").clicked();
            });
        });
        if changed {
            dlg.dirty = true;
        }
        if dlg.dirty {
            let filtered = dlg.filter.apply(&dlg.backup);
            let out = filters::apply_masked(&dlg.backup, &filtered, self.doc.selection.as_ref());
            let active = self.doc.active;
            self.doc.layers[active].pixels = out;
            self.canvas_dirty = true;
            dlg.dirty = false;
        }
        if apply {
            let message = format!("{} aplicado.", dlg.filter.name());
            self.info(message);
            self.filter_dialog = None;
        } else if cancel {
            let active = self.doc.active;
            self.doc.layers[active].pixels = dlg.backup.clone();
            self.history.discard_last();
            self.filter_dialog = None;
            self.canvas_dirty = true;
        }
    }

    pub(super) fn export_window(&mut self, ctx: &egui::Context) {
        let after_hint = self.after_export_hint();
        let Some(dlg) = &mut self.export_dialog else {
            return;
        };
        if let Some(rx) = &dlg.job {
            match rx.try_recv() {
                Ok(result) => {
                    dlg.job = None;
                    match result {
                        Ok((bytes, preview)) => {
                            let img = ColorImage::from_rgba_unmultiplied(
                                [preview.width() as usize, preview.height() as usize],
                                preview.as_raw(),
                            );
                            dlg.preview = Some(ctx.load_texture("vista-exportacion", img, TextureOptions::NEAREST));
                            dlg.bytes = Some(bytes);
                            dlg.error = None;
                        }
                        Err(e) => {
                            dlg.bytes = None;
                            dlg.error = Some(e);
                        }
                    }
                }
                Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(50)),
                Err(TryRecvError::Disconnected) => {
                    dlg.job = None;
                    dlg.error = Some("La codificación terminó inesperadamente.".into());
                }
            }
        }
        if dlg.dirty && dlg.job.is_none() {
            dlg.start_job();
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
            if dlg.job.is_some() || dlg.dirty {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Calculando…");
                });
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
                ui.add(egui::DragValue::new(&mut dlg.width).range(1..=8192).suffix(" px"));
                ui.end_row();
                ui.label("Alto");
                ui.add(egui::DragValue::new(&mut dlg.height).range(1..=8192).suffix(" px"));
                ui.end_row();
            });
            ui.checkbox(&mut dlg.transparent, "Fondo transparente");
            create = widgets::primary_button(ui, "Crear").clicked();
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
