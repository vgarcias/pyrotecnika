//! Preferencias, recientes y atajos en disco (`settings.toml` y
//! `keymap.toml` en la carpeta de configuración del SO).

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::keymap::{Keymap, KeymapFile};
use crate::design::style::{step_scale, ThemeChoice};

/// Versión del formato de los dos archivos.
const SCHEMA: u32 = 1;
/// Máximo de archivos recientes.
const MAX_RECENT: usize = 10;

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub(crate) struct Settings {
    pub schema: u32,
    pub appearance: Appearance,
    pub recent: Recent,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { schema: SCHEMA, appearance: Appearance::default(), recent: Recent::default() }
    }
}

/// Valor `key` de `table` con el tipo `T`, o `None` (con aviso) si no se
/// entiende.
fn field<T: DeserializeOwned>(table: &toml::Table, key: &str, what: &str, warns: &mut Vec<String>) -> Option<T> {
    let value = table.get(key)?;
    match value.clone().try_into::<T>() {
        Ok(v) => Some(v),
        Err(_) => {
            warns.push(format!("settings.toml: «{what}» = {value} no se entiende; se usa el valor de fábrica."));
            None
        }
    }
}

/// Subtabla `key`, o vacía (con aviso si existía con otro tipo).
fn section(table: &toml::Table, key: &str, warns: &mut Vec<String>) -> toml::Table {
    match table.get(key) {
        Some(toml::Value::Table(t)) => t.clone(),
        Some(_) => {
            warns.push(format!("settings.toml: [{key}] no es una sección; se usan los valores de fábrica."));
            toml::Table::new()
        }
        None => toml::Table::new(),
    }
}

impl Settings {
    /// Lee el archivo valor a valor: lo que no se entiende vuelve a fábrica
    /// y se avisa, sin perder el resto.
    pub(crate) fn from_table(table: &toml::Table, warns: &mut Vec<String>) -> Settings {
        let mut s = Settings::default();
        if let Some(v) = field(table, "schema", "schema", warns) {
            s.schema = v;
        }
        let appearance = section(table, "appearance", warns);
        if let Some(v) = field(&appearance, "theme", "appearance.theme", warns) {
            s.appearance.theme = v;
        }
        if let Some(v) = field(&appearance, "ui_scale", "appearance.ui_scale", warns) {
            s.appearance.ui_scale = v;
        }
        let recent = section(table, "recent", warns);
        if let Some(v) = field(&recent, "files", "recent.files", warns) {
            s.recent.files = v;
        }
        s
    }

    /// Acota lo que pudo llegar fuera de rango desde el archivo.
    pub(crate) fn sanitize(&mut self) {
        self.appearance.ui_scale = step_scale(self.appearance.ui_scale, 0);
        self.recent.files.truncate(MAX_RECENT);
        for f in &mut self.recent.files {
            *f = strip_verbatim(f);
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub(crate) struct Appearance {
    pub theme: ThemeChoice,
    pub ui_scale: f32,
}

impl Default for Appearance {
    fn default() -> Self {
        Appearance { theme: ThemeChoice::System, ui_scale: 1.0 }
    }
}

/// Archivos abiertos con éxito, el más reciente primero.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub(crate) struct Recent {
    pub files: Vec<PathBuf>,
}

/// Ruta absoluta y sin enlaces, en la forma que dan los diálogos del SO.
/// En Windows `canonicalize` devuelve `\\?\C:\…`, que no compara igual con
/// `C:\…` (y Explorer no la entiende): se quita ese prefijo.
fn normalized(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).map_or_else(|_| p.to_path_buf(), |c| strip_verbatim(&c))
}

fn strip_verbatim(p: &Path) -> PathBuf {
    let text = p.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    match text.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => p.to_path_buf(),
    }
}

impl Recent {
    pub(crate) fn push(&mut self, path: PathBuf) {
        let path = normalized(&path);
        self.files.retain(|p| normalized(p) != path);
        self.files.insert(0, path);
        self.files.truncate(MAX_RECENT);
    }

    pub(crate) fn remove(&mut self, path: &Path) {
        let path = normalized(path);
        self.files.retain(|p| normalized(p) != path);
    }

    pub(crate) fn clear(&mut self) {
        self.files.clear();
    }
}

/// Lectura y escritura de los dos archivos. Sin carpeta (`None`) no se lee
/// ni se escribe nada: así corren las pruebas y un SO sin carpeta de
/// configuración.
pub(crate) struct Store {
    dir: Option<PathBuf>,
    settings_newer: bool,
    keymap_newer: bool,
}

/// Resultado de leer un archivo: su contenido y si es de una versión más
/// nueva que esta app.
enum Read<T> {
    Missing,
    Ok(T, bool),
    Broken,
}

impl Store {
    pub(crate) fn new(dir: Option<PathBuf>) -> Store {
        Store { dir, settings_newer: false, keymap_newer: false }
    }

    fn path(&self, name: &str) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join(name))
    }

    /// Lee `name` como tabla TOML. Solo un texto que no es TOML se aparta
    /// como `.bak`; los valores sueltos que no se entienden los resuelve
    /// quien interpreta la tabla.
    fn read(&self, name: &str, warns: &mut Vec<String>) -> Read<toml::Table> {
        let Some(path) = self.path(name) else { return Read::Missing };
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Read::Missing,
            Err(e) => {
                warns.push(format!("No se pudo leer {name} ({e}); se usan los valores de fábrica."));
                return Read::Missing;
            }
        };
        match toml::from_str::<toml::Table>(&text) {
            Ok(t) => {
                let newer = t.get("schema").and_then(toml::Value::as_integer).is_some_and(|v| v > i64::from(SCHEMA));
                if newer {
                    warns.push(format!(
                        "{name} es de una versión más nueva de Pyrotecnika; se usa lo que se entiende y se \
                         guardará una copia en {name}.bak antes de cambiarlo."
                    ));
                }
                Read::Ok(t, newer)
            }
            Err(_) => {
                let _ = std::fs::rename(&path, path.with_extension("toml.bak"));
                warns
                    .push(format!("{name} estaba dañado: se apartó como {name}.bak y se usan los valores de fábrica."));
                Read::Broken
            }
        }
    }

    pub(crate) fn load_settings(&mut self) -> (Settings, Vec<String>) {
        let mut warns = Vec::new();
        let mut settings = match self.read("settings.toml", &mut warns) {
            Read::Ok(t, newer) => {
                self.settings_newer = newer;
                Settings::from_table(&t, &mut warns)
            }
            Read::Missing | Read::Broken => Settings::default(),
        };
        settings.sanitize();
        (settings, warns)
    }

    pub(crate) fn load_keymap(&mut self) -> (Keymap, Vec<String>) {
        let mut warns = Vec::new();
        match self.read("keymap.toml", &mut warns) {
            Read::Ok(t, newer) => {
                self.keymap_newer = newer;
                let file = KeymapFile::from_table(&t, &mut warns);
                let (k, more) = Keymap::from_file(&file);
                warns.extend(more);
                (k, warns)
            }
            Read::Missing | Read::Broken => (Keymap::fireworks(), warns),
        }
    }

    fn write(&self, name: &str, text: &str, newer: bool) -> Result<(), String> {
        let (Some(dir), Some(path)) = (self.dir.as_ref(), self.path(name)) else { return Ok(()) };
        std::fs::create_dir_all(dir).map_err(|e| format!("No se pudo crear {}: {e}", dir.display()))?;
        if newer && path.exists() {
            std::fs::copy(&path, path.with_extension("toml.bak"))
                .map_err(|e| format!("No se pudo respaldar {name}: {e}"))?;
        }
        write_atomic(&path, text).map_err(|e| format!("No se pudo guardar {name}: {e}"))
    }

    pub(crate) fn save_settings(&mut self, s: &Settings) -> Result<(), String> {
        let text = toml::to_string_pretty(s).map_err(|e| e.to_string())?;
        self.write("settings.toml", &text, self.settings_newer)?;
        self.settings_newer = false;
        Ok(())
    }

    pub(crate) fn save_keymap(&mut self, k: &Keymap) -> Result<(), String> {
        let text = toml::to_string_pretty(&k.to_file()).map_err(|e| e.to_string())?;
        self.write("keymap.toml", &text, self.keymap_newer)?;
        self.keymap_newer = false;
        Ok(())
    }
}

/// Escribe en un temporal de la misma carpeta y lo renombra: nunca queda un
/// archivo a medias.
pub(crate) fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::style::ThemeChoice;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn ida_y_vuelta() {
        let d = tempdir().unwrap();
        let mut st = Store::new(Some(d.path().into()));
        let mut s = Settings::default();
        s.appearance.theme = ThemeChoice::HighContrast;
        s.appearance.ui_scale = 1.5;
        s.recent.push(d.path().join("a.png"));
        st.save_settings(&s).unwrap();
        assert_eq!(st.load_settings(), (s, vec![]));
    }

    #[test]
    fn sin_archivos_valores_de_fabrica() {
        let d = tempdir().unwrap();
        let (s, w) = Store::new(Some(d.path().into())).load_settings();
        assert_eq!(s, Settings::default());
        assert!(w.is_empty());
        assert_eq!(s.appearance.theme, ThemeChoice::System);
        assert_eq!(s.appearance.ui_scale, 1.0);
    }

    #[test]
    fn archivo_danado_se_aparta() {
        let d = tempdir().unwrap();
        std::fs::write(d.path().join("settings.toml"), "esto = [no es toml").unwrap();
        let (s, w) = Store::new(Some(d.path().into())).load_settings();
        assert_eq!(s, Settings::default());
        assert_eq!(w.len(), 1);
        assert!(d.path().join("settings.toml.bak").exists());
    }

    #[test]
    fn claves_desconocidas_y_escala_fuera_de_rango() {
        let d = tempdir().unwrap();
        std::fs::write(d.path().join("settings.toml"), "schema = 1\nfuturo = 3\n[appearance]\nui_scale = 7.0\n")
            .unwrap();
        let (s, _) = Store::new(Some(d.path().into())).load_settings();
        assert_eq!(s.appearance.ui_scale, 2.0);
    }

    #[test]
    fn version_mas_nueva_se_respalda_al_escribir() {
        let d = tempdir().unwrap();
        std::fs::write(d.path().join("settings.toml"), "schema = 9\n").unwrap();
        let mut st = Store::new(Some(d.path().into()));
        let (s, w) = st.load_settings();
        assert_eq!(w.len(), 1);
        assert!(!d.path().join("settings.toml.bak").exists());
        st.save_settings(&s).unwrap();
        assert_eq!(std::fs::read_to_string(d.path().join("settings.toml.bak")).unwrap(), "schema = 9\n");
    }

    #[test]
    fn recientes_tope_orden_y_duplicados() {
        let mut r = Recent::default();
        for i in 0..12 {
            r.push(format!("f{i}.png").into());
        }
        r.push("f5.png".into());
        assert_eq!(r.files.len(), 10);
        assert_eq!(r.files[0], PathBuf::from("f5.png"));
        assert_eq!(r.files.iter().filter(|p| p.as_path() == std::path::Path::new("f5.png")).count(), 1);
    }

    #[test]
    fn sin_carpeta_no_guarda_ni_falla() {
        let mut st = Store::new(None);
        assert_eq!(st.load_settings().0, Settings::default());
        assert!(st.save_settings(&Settings::default()).is_ok());
    }

    #[test]
    fn carpeta_sin_permiso_avisa() {
        let d = tempdir().unwrap();
        let blocked = d.path().join("archivo");
        std::fs::write(&blocked, "").unwrap();
        // La «carpeta» es un archivo: crear dentro falla en los tres SO.
        let mut st = Store::new(Some(blocked.join("sub")));
        assert!(st.save_settings(&Settings::default()).is_err());
    }

    #[test]
    fn atajos_ida_y_vuelta_y_danado() {
        use crate::commands::chord::Chord;
        let d = tempdir().unwrap();
        let mut st = Store::new(Some(d.path().into()));
        let mut k = Keymap::fireworks();
        k.assign("file.export", Chord::parse("Ctrl+E").unwrap());
        st.save_keymap(&k).unwrap();
        assert_eq!(st.load_keymap(), (k, vec![]));
        std::fs::write(d.path().join("keymap.toml"), "[[[").unwrap();
        let (k2, w) = st.load_keymap();
        assert_eq!(k2, Keymap::fireworks());
        assert_eq!(w.len(), 1);
        assert!(d.path().join("keymap.toml.bak").exists());
    }
    #[test]
    fn reciente_sin_prefijo_verbatim() {
        let d = tempdir().unwrap();
        let f = d.path().join("a.png");
        std::fs::write(&f, b"x").unwrap();
        let mut r = Recent::default();
        r.push(f.clone());
        let stored = r.files[0].to_string_lossy().into_owned();
        assert!(!stored.starts_with(r"\\?\"), "{stored}");
        assert!(r.files[0].exists());
        r.remove(&f);
        assert!(r.files.is_empty());
    }

    #[test]
    fn valor_de_tipo_equivocado_conserva_lo_demas() {
        let d = tempdir().unwrap();
        std::fs::write(
            d.path().join("settings.toml"),
            "schema = 1\n[appearance]\ntheme = \"dark\"\nui_scale = \"1.2\"\n[recent]\nfiles = [\"a.png\"]\n",
        )
        .unwrap();
        let (s, w) = Store::new(Some(d.path().into())).load_settings();
        assert_eq!(s.appearance.theme, ThemeChoice::Dark);
        assert_eq!(s.appearance.ui_scale, 1.0);
        assert_eq!(s.recent.files, vec![PathBuf::from("a.png")]);
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(!d.path().join("settings.toml.bak").exists());
    }

    #[test]
    fn tema_desconocido_vuelve_al_de_fabrica() {
        let d = tempdir().unwrap();
        std::fs::write(d.path().join("settings.toml"), "[appearance]\ntheme = \"neon\"\nui_scale = 1.5\n").unwrap();
        let (s, w) = Store::new(Some(d.path().into())).load_settings();
        assert_eq!(s.appearance.theme, ThemeChoice::System);
        assert_eq!(s.appearance.ui_scale, 1.5);
        assert_eq!(w.len(), 1, "{w:?}");
    }

    #[test]
    fn atajo_escrito_como_texto_y_valor_raro() {
        use crate::commands::chord::Chord;
        let d = tempdir().unwrap();
        std::fs::write(
            d.path().join("keymap.toml"),
            "schema = 1\n[bindings]\n\"file.export\" = \"Ctrl+E\"\n\"file.open\" = 5\n\"edit.palette\" = [\"Ctrl+J\"]\n",
        )
        .unwrap();
        let (k, w) = Store::new(Some(d.path().into())).load_keymap();
        assert_eq!(k.chords("file.export"), &[Chord::parse("Ctrl+E").unwrap()]);
        assert_eq!(k.chords("file.open"), Keymap::fireworks().chords("file.open"));
        assert_eq!(k.chords("edit.palette"), &[Chord::parse("Ctrl+J").unwrap()]);
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(!d.path().join("keymap.toml.bak").exists());
    }
}
