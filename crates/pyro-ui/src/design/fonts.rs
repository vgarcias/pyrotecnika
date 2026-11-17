//! Tipografía: Inter para la interfaz, JetBrains Mono para valores numéricos
//! y la fuente de iconos Lucide, registradas juntas (§2 del spec).

use std::collections::BTreeMap;
use std::sync::Arc;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily, FontId, TextStyle};

use crate::app::icons;

/// Familia de Inter SemiBold: etiquetas, botones, títulos y avisos.
pub(crate) const SEMIBOLD: &str = "semibold";

const INTER: &[u8] = include_bytes!("../../assets/fonts/Inter-Regular.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../../assets/fonts/Inter-SemiBold.ttf");
const JETBRAINS_MONO: &[u8] = include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf");

/// Estilo de texto de los títulos de sección y de ventana (15 px semibold).
pub(crate) fn title_style() -> TextStyle {
    TextStyle::Name("title".into())
}

/// Inter, Inter SemiBold, JetBrains Mono y los iconos, con las fuentes de
/// egui detrás como respaldo para los caracteres que falten.
pub(crate) fn fonts() -> FontDefinitions {
    let mut f = FontDefinitions::default();
    let fallback_prop = f.families[&FontFamily::Proportional].clone();
    let fallback_mono = f.families[&FontFamily::Monospace].clone();
    for (key, data) in [
        ("inter", INTER),
        ("inter-semibold", INTER_SEMIBOLD),
        ("jetbrains-mono", JETBRAINS_MONO),
        (icons::FAMILY, icons::FONT),
    ] {
        f.font_data.insert(key.into(), Arc::new(FontData::from_static(data)));
    }
    let with = |first: &str, rest: &[String]| {
        std::iter::once(first.to_string()).chain(rest.iter().cloned()).collect::<Vec<_>>()
    };
    f.families.insert(FontFamily::Proportional, with("inter", &fallback_prop));
    f.families.insert(FontFamily::Name(SEMIBOLD.into()), with("inter-semibold", &fallback_prop));
    f.families.insert(FontFamily::Monospace, with("jetbrains-mono", &fallback_mono));
    f.families.insert(FontFamily::Name(icons::FAMILY.into()), vec![icons::FAMILY.into()]);
    f
}

pub(crate) fn install_fonts(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
}

/// Escala tipográfica a 100 %: caption 11 · body 13 · label/botón 13
/// semibold · title 15 semibold · heading 18 semibold · números 12 mono.
pub(crate) fn text_styles() -> BTreeMap<TextStyle, FontId> {
    let semibold = || FontFamily::Name(SEMIBOLD.into());
    BTreeMap::from([
        (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(13.0, semibold())),
        (title_style(), FontId::new(15.0, semibold())),
        (TextStyle::Heading, FontId::new(18.0, semibold())),
        (TextStyle::Monospace, FontId::new(12.0, FontFamily::Monospace)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn familias_registradas() {
        let f = fonts();
        for fam in [
            FontFamily::Proportional,
            FontFamily::Monospace,
            FontFamily::Name(SEMIBOLD.into()),
            FontFamily::Name(icons::FAMILY.into()),
        ] {
            assert!(f.families.get(&fam).is_some_and(|v| !v.is_empty()), "{fam:?}");
        }
        assert_eq!(f.families[&FontFamily::Proportional][0], "inter");
    }

    #[test]
    fn escala_tipografica_del_spec() {
        let s = text_styles();
        assert_eq!(s[&TextStyle::Small].size, 11.0);
        assert_eq!(s[&TextStyle::Body].size, 13.0);
        assert_eq!(s[&TextStyle::Button].size, 13.0);
        assert_eq!(s[&title_style()].size, 15.0);
        assert_eq!(s[&TextStyle::Heading].size, 18.0);
        assert_eq!(s[&TextStyle::Monospace].size, 12.0);
        assert_eq!(s[&TextStyle::Button].family, FontFamily::Name(SEMIBOLD.into()));
    }
}
