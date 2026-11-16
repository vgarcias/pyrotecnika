//! Escenas deterministas de la app para pruebas: un documento sintético con
//! dos capas y una selección, cada diálogo abierto en un estado fijo, en
//! cualquier tema y escala. Las capturas de referencia (`#[ignore]`) se
//! generan en local con `UPDATE_SNAPSHOTS=1 cargo test -p pyro-ui -- --ignored`
//! y se guardan en `crates/pyro-ui/tests/snapshots/`; el CI solo comprueba
//! que las escenas se dibujan, sin comparar píxeles (sin GPU no coinciden).

use eframe::egui;
use egui_kittest::Harness;
use image::{Rgba, RgbaImage};

use pyro_core::document::{Document, Layer};
use pyro_core::filters::Filter;
use pyro_core::selection::Mask;

use super::dialogs::ExportDialog;
use super::guard::Pending;
use super::updates::UpdateState;
use super::{Action, PyroApp};
use crate::design::style::{apply_theme, ThemeChoice};

#[derive(Clone, Copy, Debug)]
pub(super) enum Scene {
    Main,
    Export,
    Filter,
    Confirm,
    Updates,
    LongNames,
}

impl Scene {
    pub(super) const ALL: [Scene; 6] =
        [Scene::Main, Scene::Export, Scene::Filter, Scene::Confirm, Scene::Updates, Scene::LongNames];

    pub(super) fn name(self) -> &'static str {
        match self {
            Scene::Main => "principal",
            Scene::Export => "exportar",
            Scene::Filter => "filtro",
            Scene::Confirm => "confirmar",
            Scene::Updates => "actualizaciones",
            Scene::LongNames => "nombres-largos",
        }
    }
}

/// Documento sintético de 800×533: «Fondo» (cielo y tierra) y «Retoque
/// cielo» activa al 85 %, con una selección de 230×150 en (120, 90).
fn sample_document() -> Document {
    let (w, h) = (800, 533);
    let sky = |y: u32| {
        let t = y as f32 / (h as f32 * 0.4);
        [(109.0 + 60.0 * t) as u8, (143.0 + 50.0 * t) as u8, (176.0 + 38.0 * t) as u8, 255]
    };
    let ground = |y: u32| {
        let t = (y as f32 - h as f32 * 0.4) / (h as f32 * 0.6);
        [(217.0 - 138.0 * t) as u8, (201.0 - 111.0 * t) as u8, (163.0 - 105.0 * t) as u8, 255]
    };
    let background = RgbaImage::from_fn(w, h, |_, y| Rgba(if y < h * 2 / 5 { sky(y) } else { ground(y) }));
    let mut doc = Document::from_image(background, "Fondo");
    let touch = RgbaImage::from_fn(w, h, |x, y| {
        let inside = (150..560).contains(&x) && (40..170).contains(&y);
        Rgba(if inside { [235, 240, 245, 90] } else { [0, 0, 0, 0] })
    });
    let mut layer = Layer::from_image("Retoque cielo", touch);
    layer.opacity = 0.85;
    doc.layers.push(layer);
    doc.active = 1;
    doc.selection = Some(Mask::rect(w, h, (120.0, 90.0), (350.0, 240.0)));
    doc
}

/// La app en una escena fija, con el tema y la escala de interfaz dados.
pub(super) fn scene(which: Scene, theme: ThemeChoice, scale: f32, size: egui::Vec2) -> Harness<'static, PyroApp> {
    let mut harness = Harness::builder().with_size(size).build_eframe(|cc| PyroApp::new(cc));
    let ctx = harness.ctx.clone();
    {
        let app = harness.state_mut();
        app.reset_document(sample_document());
        app.history.push(&app.doc.clone());
        app.history.mark_saved();
        app.theme = theme;
        app.ui_scale = scale;
    }
    apply_theme(&ctx, theme);
    ctx.set_zoom_factor(scale);
    // Primer cuadro: fuentes y tamaño de textura listos antes de abrir diálogos.
    harness.run_steps(2);
    let app = harness.state_mut();
    match which {
        Scene::Main => {}
        Scene::Export => {
            app.export_dialog = Some(ExportDialog::new(app.doc.composite()));
        }
        Scene::Filter => app.run(Action::Filter(Filter::BRIGHTNESS_CONTRAST), &ctx),
        Scene::Confirm => {
            app.history.push(&app.doc.clone());
            app.request(Pending::Quit, &ctx);
        }
        Scene::Updates => app.update = Some(UpdateState::UpToDate),
        Scene::LongNames => {
            app.doc.layers[1].name = "Retoque del cielo con nubes de tormenta sobre el cenote, versión final 3".into();
            app.warn(
                "La capa «Fondo con la textura de piedra caliza del cenote, copia 2» está oculta. Muéstrala para editarla.",
            );
        }
    }
    harness.run_steps(4);
    // La exportación codifica en otro hilo: se espera a que termine para
    // que la captura muestre el tamaño y la vista previa.
    for _ in 0..200 {
        if harness.state().export_dialog.as_ref().is_none_or(|d| d.bytes.is_some()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        harness.run_steps(1);
    }
    harness.run_steps(2);
    harness
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::kittest::Queryable;

    #[test]
    fn todas_las_escenas_se_dibujan() {
        for theme in [ThemeChoice::Dark, ThemeChoice::Light, ThemeChoice::HighContrast] {
            for s in Scene::ALL {
                let mut h = scene(s, theme, 1.0, egui::vec2(1280.0, 800.0));
                h.run_steps(4);
            }
        }
    }

    /// Capturas de referencia de cada escena en los tres temas, más la
    /// principal al 150 %. Se comparan en local (Windows, GPU real).
    #[test]
    #[ignore = "capturas de referencia: cargo test -p pyro-ui -- --ignored"]
    fn capturas_de_referencia() {
        let options = egui_kittest::SnapshotOptions::new().threshold(1.0);
        let mut failures = Vec::new();
        let mut cases: Vec<(Scene, ThemeChoice, &str, f32)> = Vec::new();
        for (theme, tag) in [
            (ThemeChoice::Dark, "oscuro"),
            (ThemeChoice::Light, "claro"),
            (ThemeChoice::HighContrast, "alto-contraste"),
        ] {
            for s in Scene::ALL {
                cases.push((s, theme, tag, 1.0));
            }
        }
        cases.push((Scene::Main, ThemeChoice::Dark, "oscuro", 1.5));
        for (s, theme, tag, scale) in cases {
            let mut h = scene(s, theme, scale, egui::vec2(1280.0, 800.0));
            let suffix = if scale == 1.0 { String::new() } else { format!("-{:.0}", scale * 100.0) };
            let name = format!("{}-{tag}{suffix}", s.name());
            if let Err(e) = h.try_snapshot_options(&name, &options) {
                failures.push(format!("{name}: {e}"));
            }
        }
        assert!(
            failures.is_empty(),
            "{}",
            failures.join(
                "
"
            )
        );
    }

    #[test]
    fn ventana_minima_al_200_por_ciento() {
        let mut h = scene(Scene::Main, ThemeChoice::Dark, 2.0, egui::vec2(900.0, 560.0));
        h.run_steps(4);
        let canvas = h.get_by_label("Lienzo");
        assert!(canvas.raw_bounds().expect("el lienzo tiene área").width() > 0.0);
    }
}
