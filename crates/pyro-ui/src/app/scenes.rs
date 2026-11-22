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
    /// Paleta de comandos con «gaus» escrito.
    Palette,
    /// Preferencias › Atajos con «Exportar…» elegido y el aviso de conflicto.
    Prefs,
    /// Preferencias › Apariencia (solo se captura en oscuro).
    PrefsAppearance,
    /// Abriendo un archivo al 40 %: progreso y «Cancelar» en la barra de estado.
    Task,
    /// Tres pestañas: «cenote.png», «logo.ora» con cambios (activa) y «Sin título 3».
    Tabs,
    /// Salir con dos pestañas con cambios (solo se captura en oscuro).
    QuitMany,
    /// Recuperar trabajo con dos documentos (solo se captura en oscuro).
    Recover,
}

impl Scene {
    pub(super) const ALL: [Scene; 10] = [
        Scene::Main,
        Scene::Export,
        Scene::Filter,
        Scene::Confirm,
        Scene::Updates,
        Scene::LongNames,
        Scene::Palette,
        Scene::Prefs,
        Scene::Task,
        Scene::Tabs,
    ];

    pub(super) fn name(self) -> &'static str {
        match self {
            Scene::Main => "principal",
            Scene::Export => "exportar",
            Scene::Filter => "filtro",
            Scene::Confirm => "confirmar",
            Scene::Updates => "actualizaciones",
            Scene::LongNames => "nombres-largos",
            Scene::Palette => "paleta",
            Scene::Prefs => "preferencias-atajos",
            Scene::PrefsAppearance => "preferencias-apariencia",
            Scene::Task => "tarea-en-curso",
            Scene::Tabs => "pestanas",
            Scene::QuitMany => "salir-varias",
            Scene::Recover => "recuperar",
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
    let mut harness = Harness::builder().with_size(size).build_eframe(|cc| PyroApp::with_config(cc, None));
    let ctx = harness.ctx.clone();
    {
        let app = harness.state_mut();
        app.reset_document(sample_document());
        app.tab.history.push(&app.tab.doc.clone());
        app.tab.history.mark_saved();
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
            app.tab.export_dialog = Some(ExportDialog::new(app.tab.doc.composite()));
        }
        Scene::Filter => app.run(Action::Filter(Filter::BRIGHTNESS_CONTRAST), &ctx),
        Scene::Confirm => {
            app.tab.history.push(&app.tab.doc.clone());
            app.request(Pending::Quit, &ctx);
        }
        Scene::Updates => app.update = Some(UpdateState::UpToDate),
        Scene::LongNames => {
            app.tab.doc.layers[1].name =
                "Retoque del cielo con nubes de tormenta sobre el cenote, versión final 3".into();
            app.warn(
                "La capa «Fondo con la textura de piedra caliza del cenote, copia 2» está oculta. Muéstrala para editarla.",
            );
        }
        Scene::Palette => app.palette = Some(super::palette::Palette::with_query("gaus")),
        Scene::Prefs => {
            let mut prefs = super::prefs::PrefsDialog::new();
            prefs.section = super::prefs::Section::Shortcuts;
            prefs.selected = Some("file.export");
            let chord = crate::commands::chord::Chord::parse("Ctrl+K").expect("combinación válida");
            prefs.pending = Some(("file.export", chord));
            app.prefs = Some(prefs);
        }
        Scene::PrefsAppearance => app.prefs = Some(super::prefs::PrefsDialog::new()),
        Scene::Task => {
            let job = pyro_services::jobs::Job::spawn(super::tasks::wake(&ctx), |ctx| {
                use pyro_core::task::Monitor;
                ctx.progress(0.4);
                loop {
                    ctx.check()?;
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            });
            app.start_task(super::tasks::TaskKind::Open("cenote-final.png".into()), job);
        }
        Scene::Recover => app.recover_dialog = Some(super::recover::sample_entries()),
        Scene::QuitMany => {
            app.tab.history.push(&app.tab.doc.clone());
            let mut logo = super::tab::Tab::new(sample_document(), 0);
            logo.source_path = Some("logo.ora".into());
            logo.history.push(&logo.doc.clone());
            app.add_tab(logo);
            app.quit_window = true;
            app.confirm_focus = true;
        }
        Scene::Tabs => {
            app.tab.source_path = Some("cenote.png".into());
            app.tab.untitled = 0;
            let mut logo = super::tab::Tab::new(sample_document(), 0);
            logo.source_path = Some("logo.ora".into());
            logo.history.push(&logo.doc.clone());
            app.add_tab(logo);
            app.untitled_next = 3;
            let n = app.next_untitled();
            app.others.push(super::tab::Tab::new(Document::new(800, 600, Some([255, 255, 255, 255])), n));
        }
    }
    harness.run_steps(4);
    // La exportación codifica en otro hilo: se espera a que termine para
    // que la captura muestre el tamaño y la vista previa.
    for _ in 0..200 {
        let app = harness.state();
        let export_ready = app.tab.export_dialog.as_ref().is_none_or(|d| d.bytes.is_some());
        let task_ready = app.tab.task.as_ref().is_none_or(|t| t.job.progress().is_some());
        let filter_ready = app.tab.filter_dialog.as_ref().is_none_or(|d| d.preview.is_none() && d.shown.is_some());
        if export_ready && task_ready && filter_ready {
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
            for s in Scene::ALL.into_iter().chain([Scene::PrefsAppearance, Scene::QuitMany, Scene::Recover]) {
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
        cases.push((Scene::PrefsAppearance, ThemeChoice::Dark, "oscuro", 1.0));
        cases.push((Scene::QuitMany, ThemeChoice::Dark, "oscuro", 1.0));
        cases.push((Scene::Recover, ThemeChoice::Dark, "oscuro", 1.0));
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

    /// Al 200 % en la ventana mínima la columna de herramientas no cabe: debe
    /// poder desplazarse hasta la última herramienta y la ayuda, por encima
    /// de la barra de estado.
    #[test]
    fn herramientas_alcanzables_al_200_por_ciento() {
        let mut h = scene(Scene::Main, ThemeChoice::Dark, 2.0, egui::vec2(900.0, 560.0));
        h.run_steps(4);
        h.input_mut().events.push(egui::Event::PointerMoved(egui::pos2(20.0, 120.0)));
        h.input_mut().events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -2000.0),
            modifiers: egui::Modifiers::NONE,
        });
        h.run_steps(30);
        let status_top = h.get_by_label("800×533 px").raw_bounds().expect("barra de estado").y0;
        for name in ["Cuentagotas", "Ayuda de navegación"] {
            let b = h.get_by_label(name).raw_bounds().expect("tiene área");
            assert!(b.y1 <= status_top, "{name} queda bajo la barra de estado: {b:?} vs {status_top}");
        }
    }
}
