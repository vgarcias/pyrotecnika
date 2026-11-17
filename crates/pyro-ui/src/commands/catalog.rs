//! Catálogo único de comandos: cada acción de la app con su identificador
//! estable, su nombre, su categoría y sus atajos de fábrica (Fireworks CS6,
//! spec de la fase 2 §6.1).

use pyro_core::filters::Filter;

use crate::app::tools::Tool;
use crate::app::Action;
use crate::design::style::ThemeChoice;
use Category::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Category {
    File,
    Edit,
    Select,
    Layers,
    Filters,
    View,
    Tools,
    Brush,
    Help,
}

impl Category {
    pub(crate) fn label(self) -> &'static str {
        match self {
            File => "Archivo",
            Edit => "Editar",
            Select => "Selección",
            Layers => "Capas",
            Filters => "Filtros",
            View => "Ver",
            Tools => "Herramientas",
            Brush => "Pincel",
            Help => "Ayuda",
        }
    }
}

/// Un comando. `id` es lo que se guarda en `keymap.toml`; `defaults`, los
/// atajos de fábrica en el formato de [`super::chord::Chord::parse`].
pub(crate) struct CommandDef {
    pub id: &'static str,
    pub name: &'static str,
    pub category: Category,
    pub action: Action,
    pub defaults: &'static [&'static str],
}

const fn def(
    id: &'static str,
    name: &'static str,
    category: Category,
    action: Action,
    defaults: &'static [&'static str],
) -> CommandDef {
    CommandDef { id, name, category, action, defaults }
}

/// Teclas sin modificador que algunos comandos que no son herramientas
/// tienen de fábrica (y por eso se les permiten).
pub(crate) const PLAIN_KEY_DEFAULTS: &[&str] = &["Delete", "Backspace", "OpenBracket", "CloseBracket"];

/// Todos los comandos, en el orden de los menús. Las herramientas siguen el
/// orden de la columna de herramientas (lo usa la alternancia de teclas).
pub(crate) static CATALOG: &[CommandDef] = &[
    def("file.new", "Nuevo…", File, Action::New, &["Ctrl+N"]),
    def("file.open", "Abrir…", File, Action::Open, &["Ctrl+O"]),
    def("file.export", "Exportar…", File, Action::Export, &["Ctrl+Shift+R"]),
    def("file.reveal", "Mostrar en carpeta", File, Action::RevealInFolder, &[]),
    def("file.clear_recent", "Limpiar lista de recientes", File, Action::ClearRecent, &[]),
    def("file.quit", "Salir", File, Action::Quit, &["Ctrl+Q"]),
    def("edit.undo", "Deshacer", Edit, Action::Undo, &["Ctrl+Z"]),
    def("edit.redo", "Rehacer", Edit, Action::Redo, &["Ctrl+Shift+Z"]),
    def("edit.clear", "Borrar selección", Edit, Action::ClearSelection, &["Delete", "Backspace"]),
    def("edit.crop", "Recortar a la selección", Edit, Action::CropToSelection, &[]),
    def("edit.palette", "Paleta de comandos", Edit, Action::OpenPalette, &["Ctrl+K"]),
    def("edit.preferences", "Preferencias…", Edit, Action::OpenPreferences, &[]),
    def("select.all", "Seleccionar todo", Select, Action::SelectAll, &["Ctrl+A"]),
    def("select.none", "Deseleccionar", Select, Action::Deselect, &["Ctrl+D"]),
    def("select.invert", "Invertir selección", Select, Action::InvertSelection, &["Ctrl+Shift+I"]),
    def("layer.new", "Nueva capa", Layers, Action::LayerNew, &["Shift+L"]),
    def("layer.duplicate", "Duplicar capa", Layers, Action::LayerDuplicate, &["Ctrl+Alt+D"]),
    def("layer.delete", "Eliminar capa", Layers, Action::LayerDelete, &[]),
    def("layer.up", "Subir capa", Layers, Action::LayerUp, &["Ctrl+Shift+F"]),
    def("layer.down", "Bajar capa", Layers, Action::LayerDown, &["Ctrl+Shift+B"]),
    def("layer.merge_down", "Combinar abajo", Layers, Action::LayerMergeDown, &["Ctrl+Alt+E"]),
    def("filter.brightness_contrast", "Brillo y contraste…", Filters, Action::Filter(Filter::BRIGHTNESS_CONTRAST), &[]),
    def("filter.hue_saturation", "Tono y saturación…", Filters, Action::Filter(Filter::HUE_SATURATION), &[]),
    def("filter.levels", "Niveles…", Filters, Action::Filter(Filter::LEVELS), &[]),
    def("filter.invert", "Invertir colores", Filters, Action::Filter(Filter::Invert), &[]),
    def("filter.grayscale", "Escala de grises", Filters, Action::Filter(Filter::Grayscale), &[]),
    def("filter.gaussian_blur", "Desenfoque gaussiano…", Filters, Action::Filter(Filter::GAUSSIAN_BLUR), &[]),
    def("filter.sharpen", "Enfocar…", Filters, Action::Filter(Filter::SHARPEN), &[]),
    def("view.zoom_in", "Acercar", View, Action::ZoomIn, &["Ctrl+Equals", "Ctrl+Plus"]),
    def("view.zoom_out", "Alejar", View, Action::ZoomOut, &["Ctrl+Minus"]),
    def("view.fit", "Ajustar a la ventana", View, Action::Fit, &["Ctrl+Alt+0"]),
    def("view.actual_size", "Tamaño real", View, Action::ActualSize, &["Ctrl+1"]),
    def("view.theme_system", "Tema: seguir al sistema", View, Action::SetTheme(ThemeChoice::System), &[]),
    def("view.theme_light", "Tema: claro", View, Action::SetTheme(ThemeChoice::Light), &[]),
    def("view.theme_dark", "Tema: oscuro", View, Action::SetTheme(ThemeChoice::Dark), &[]),
    def("view.theme_high_contrast", "Tema: alto contraste", View, Action::SetTheme(ThemeChoice::HighContrast), &[]),
    def("view.ui_scale_up", "Aumentar tamaño de la interfaz", View, Action::UiScaleUp, &[]),
    def("view.ui_scale_down", "Reducir tamaño de la interfaz", View, Action::UiScaleDown, &[]),
    def("view.ui_scale_reset", "Restablecer tamaño de la interfaz (100 %)", View, Action::UiScaleReset, &[]),
    def("tool.move", "Mover", Tools, Action::SelectTool(Tool::Move), &["V"]),
    def("tool.marquee", "Marco", Tools, Action::SelectTool(Tool::Marquee), &["M"]),
    def("tool.ellipse", "Elipse", Tools, Action::SelectTool(Tool::Ellipse), &["M"]),
    def("tool.lasso", "Lazo", Tools, Action::SelectTool(Tool::Lasso), &["L"]),
    def("tool.wand", "Varita mágica", Tools, Action::SelectTool(Tool::Wand), &["W"]),
    def("tool.brush", "Pincel", Tools, Action::SelectTool(Tool::Brush), &["B"]),
    def("tool.eraser", "Borrador", Tools, Action::SelectTool(Tool::Eraser), &["E"]),
    def("tool.clone", "Tampón de clonar", Tools, Action::SelectTool(Tool::Clone), &["S"]),
    def("tool.blur", "Desenfocar", Tools, Action::SelectTool(Tool::Blur), &["R"]),
    def("tool.bucket", "Bote de pintura", Tools, Action::SelectTool(Tool::Bucket), &["G"]),
    def("tool.eyedropper", "Cuentagotas", Tools, Action::SelectTool(Tool::Eyedropper), &["I"]),
    def("brush.smaller", "Reducir tamaño del pincel", Brush, Action::BrushSmaller, &["OpenBracket"]),
    def("brush.bigger", "Aumentar tamaño del pincel", Brush, Action::BrushBigger, &["CloseBracket"]),
    def("help.updates", "Buscar actualizaciones…", Help, Action::CheckUpdates, &[]),
];

pub(crate) fn find(id: &str) -> Option<&'static CommandDef> {
    CATALOG.iter().find(|c| c.id == id)
}

pub(crate) fn by_action(action: Action) -> Option<&'static CommandDef> {
    CATALOG.iter().find(|c| c.action == action)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tools::TOOLS;
    use crate::app::Action;
    use crate::commands::chord::Chord;

    #[test]
    fn atajos_de_fabrica_de_fireworks() {
        let want: &[(&str, &[&str])] = &[
            ("file.new", &["Ctrl+N"]),
            ("file.open", &["Ctrl+O"]),
            ("file.export", &["Ctrl+Shift+R"]),
            ("file.quit", &["Ctrl+Q"]),
            ("edit.undo", &["Ctrl+Z"]),
            ("edit.redo", &["Ctrl+Shift+Z"]),
            ("edit.clear", &["Delete", "Backspace"]),
            ("edit.palette", &["Ctrl+K"]),
            ("select.all", &["Ctrl+A"]),
            ("select.none", &["Ctrl+D"]),
            ("select.invert", &["Ctrl+Shift+I"]),
            ("layer.new", &["Shift+L"]),
            ("layer.duplicate", &["Ctrl+Alt+D"]),
            ("layer.up", &["Ctrl+Shift+F"]),
            ("layer.down", &["Ctrl+Shift+B"]),
            ("layer.merge_down", &["Ctrl+Alt+E"]),
            ("view.zoom_in", &["Ctrl+Equals", "Ctrl+Plus"]),
            ("view.zoom_out", &["Ctrl+Minus"]),
            ("view.fit", &["Ctrl+Alt+0"]),
            ("view.actual_size", &["Ctrl+1"]),
            ("tool.move", &["V"]),
            ("tool.marquee", &["M"]),
            ("tool.ellipse", &["M"]),
            ("tool.lasso", &["L"]),
            ("tool.wand", &["W"]),
            ("tool.brush", &["B"]),
            ("tool.eraser", &["E"]),
            ("tool.clone", &["S"]),
            ("tool.blur", &["R"]),
            ("tool.bucket", &["G"]),
            ("tool.eyedropper", &["I"]),
            ("brush.smaller", &["OpenBracket"]),
            ("brush.bigger", &["CloseBracket"]),
        ];
        for (id, chords) in want {
            assert_eq!(find(id).unwrap_or_else(|| panic!("falta {id}")).defaults, *chords, "{id}");
        }
        for c in CATALOG.iter().filter(|c| !want.iter().any(|(id, _)| id == &c.id)) {
            assert!(c.defaults.is_empty(), "{} no tiene atajo de fábrica en el spec", c.id);
        }
    }

    #[test]
    fn ids_unicos_y_atajos_validos() {
        let mut seen = std::collections::HashSet::new();
        for c in CATALOG {
            assert!(seen.insert(c.id), "id repetido {}", c.id);
            for d in c.defaults {
                Chord::parse(d).unwrap_or_else(|e| panic!("{}: {e}", c.id));
            }
        }
    }

    #[test]
    fn todas_las_herramientas_y_filtros_estan() {
        for (tool, _, _) in TOOLS {
            assert!(by_action(Action::SelectTool(tool)).is_some(), "{tool:?}");
        }
        assert_eq!(CATALOG.iter().filter(|c| c.category == Category::Filters).count(), 7);
    }
}
