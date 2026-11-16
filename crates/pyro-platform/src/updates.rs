//! Búsqueda e instalación de actualizaciones (docs/ARQUITECTURA.md §8.3).
//!
//! Solo se conecta cuando el usuario lo pide (Ayuda › Buscar
//! actualizaciones…). Descarga `latest.json` del último release estable de
//! GitHub, que lista para cada plataforma (`windows-x86_64`,
//! `macos-aarch64`, …) la URL del paquete y su firma minisign. La firma se
//! comprueba con la clave pública incluida en el binario **antes** de
//! escribir nada en disco.
//!
//! Solo se reemplazan solas las copias que gestiona el propio usuario: el
//! instalador NSIS de Windows, el `.app` de macOS (DMG) y la AppImage. Las
//! instaladas con MSI, PKG, DEB o RPM las gestiona IT o el gestor de
//! paquetes, así que en ellas solo se avisa.
//!
//! El formato de `latest.json` y de las firmas es el de cargo-packager
//! (`cargo packager signer`), sin depender de su crate de actualización.
//!
//! Todas las funciones bloquean: llámalas desde un hilo aparte.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine;
use serde::Deserialize;

/// Página de descargas, para las copias que no se actualizan solas.
pub const RELEASES_URL: &str = "https://github.com/vgarcias/pyrotecnika/releases/latest";

const ENDPOINT: &str = "https://github.com/vgarcias/pyrotecnika/releases/latest/download/latest.json";
const PUBKEY: &str = include_str!("../../../packaging/updater-pubkey.txt");
/// Tope de tamaño de un paquete descargado (los actuales rondan 5–15 MB).
const MAX_PACKAGE: u64 = 512 * 1024 * 1024;

#[derive(Deserialize)]
struct Manifest {
    version: String,
    notes: Option<String>,
    #[serde(default)]
    platforms: HashMap<String, Package>,
}

#[derive(Deserialize, Clone)]
struct Package {
    url: String,
    signature: String,
    format: String,
}

/// Una versión más nueva que la instalada.
pub struct Available {
    pub version: String,
    pub notes: Option<String>,
    /// `false` si esta copia la gestiona otro sistema (MSI, PKG, DEB, RPM o
    /// una compilación de desarrollo) o si no hay paquete para esta
    /// plataforma: entonces solo se puede avisar.
    pub can_install: bool,
    package: Option<Package>,
}

/// Busca una versión más nueva que `current_version`.
pub fn check(current_version: &str) -> Result<Option<Available>, String> {
    let current = semver::Version::parse(current_version).map_err(|e| format!("Versión actual no válida: {e}"))?;
    let body = agent(Duration::from_secs(30))
        .get(ENDPOINT)
        .call()
        .map_err(network_error)?
        .body_mut()
        .read_to_string()
        .map_err(network_error)?;
    let manifest: Manifest = serde_json::from_str(&body).map_err(|_| BAD_MANIFEST.to_string())?;
    let remote = semver::Version::parse(manifest.version.trim_start_matches('v')).map_err(|_| BAD_MANIFEST)?;
    if remote <= current {
        return Ok(None);
    }
    let target = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    let package = manifest.platforms.get(&target).cloned();
    let can_install = package.as_ref().is_some_and(|p| p.format == expected_format()) && self_updating();
    Ok(Some(Available {
        version: remote.to_string(),
        notes: manifest.notes.filter(|n| !n.trim().is_empty()),
        can_install,
        package,
    }))
}

impl Available {
    /// Descarga el paquete, verifica la firma y lo instala. Al volver, la
    /// nueva versión ya está en disco y hay que reiniciar ([`relaunch`]).
    ///
    /// En Windows lanza el instalador y **termina el proceso** (el instalador
    /// vuelve a abrir la app al acabar), así que no regresa si todo va bien:
    /// el trabajo sin guardar debe resolverse antes de llamarla.
    pub fn install(self) -> Result<(), String> {
        let package = match self.package {
            Some(p) if self.can_install => p,
            _ => return Err("Esta copia no se puede actualizar sola.".into()),
        };
        let bytes = agent(Duration::from_secs(600))
            .get(&package.url)
            .call()
            .map_err(network_error)?
            .body_mut()
            .with_config()
            .limit(MAX_PACKAGE)
            .read_to_vec()
            .map_err(network_error)?;
        verify(&bytes, &package.signature)?;
        apply(bytes)
    }
}

/// Abre la versión recién instalada en un proceso nuevo. Después, la
/// interfaz debe cerrar la ventana actual.
pub fn relaunch() -> Result<(), String> {
    relaunch_os().map_err(|e| format!("No se pudo volver a abrir Pyrotecnika: {e}"))
}

const BAD_MANIFEST: &str = "La información de la última versión no tiene un formato válido.";

fn agent(timeout: Duration) -> ureq::Agent {
    // rustls con ring y los certificados del sistema operativo.
    let _ = rustls::crypto::ring::default_provider().install_default();
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .user_agent(concat!("Pyrotecnika/", env!("CARGO_PKG_VERSION")))
        .tls_config(ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build())
        .build()
        .into()
}

fn network_error(e: ureq::Error) -> String {
    match e {
        ureq::Error::StatusCode(404) => "Todavía no hay información de actualizaciones publicada.".into(),
        ureq::Error::StatusCode(code) => format!("GitHub respondió con un error ({code}). Inténtalo más tarde."),
        ureq::Error::BodyExceedsLimit(_) => "El paquete descargado es demasiado grande. No se instaló nada.".into(),
        ureq::Error::Timeout(_) => "La conexión con GitHub tardó demasiado. Inténtalo más tarde.".into(),
        ureq::Error::Io(_) | ureq::Error::HostNotFound | ureq::Error::ConnectionFailed => {
            "No se pudo conectar con GitHub. Revisa tu conexión a internet.".into()
        }
        other => format!("No se pudo descargar la actualización: {other}"),
    }
}

/// Comprueba la firma minisign de `data`. La clave y la firma vienen en
/// base64 (formato de `cargo packager signer`).
fn verify(data: &[u8], signature_b64: &str) -> Result<(), String> {
    const INVALID: &str = "La firma del paquete descargado no es válida. No se instaló nada.";
    let decode = |s: &str| {
        base64::engine::general_purpose::STANDARD.decode(s.trim()).ok().and_then(|b| String::from_utf8(b).ok())
    };
    let key = decode(PUBKEY).and_then(|k| minisign_verify::PublicKey::decode(&k).ok()).ok_or(INVALID)?;
    let signature = decode(signature_b64).and_then(|s| minisign_verify::Signature::decode(&s).ok()).ok_or(INVALID)?;
    key.verify(data, &signature, true).map_err(|_| INVALID.to_string())
}

/// Carpeta temporal propia, creada de nuevo y solo para este proceso.
#[cfg(any(target_os = "windows", target_os = "macos"))]
fn fresh_temp_dir(name: &str) -> std::io::Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("pyrotecnika-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn io_error(e: std::io::Error) -> String {
    if e.kind() == std::io::ErrorKind::PermissionDenied {
        "No hay permiso para reemplazar la aplicación. Instala la nueva versión a mano desde la página de descargas."
            .into()
    } else {
        format!("No se pudo instalar la actualización: {e}")
    }
}

// ---------- Por sistema operativo ----------

/// ¿Esta copia se puede reemplazar a sí misma?
fn self_updating() -> bool {
    std::env::current_exe().is_ok_and(|exe| detect(&exe))
}

/// Windows: el instalador NSIS deja `uninstall.exe` junto al ejecutable; el
/// MSI no, y una compilación de `cargo build` tampoco.
#[cfg(target_os = "windows")]
fn detect(exe: &Path) -> bool {
    exe.parent().is_some_and(|dir| dir.join("uninstall.exe").is_file())
}

#[cfg(target_os = "windows")]
fn expected_format() -> &'static str {
    "nsis"
}

/// Ejecuta el instalador en modo pasivo (/P: solo barra de progreso) y con
/// /R para que vuelva a abrir la app; este proceso termina para que el
/// instalador pueda reemplazar el ejecutable.
#[cfg(target_os = "windows")]
fn apply(bytes: Vec<u8>) -> Result<(), String> {
    let installer = fresh_temp_dir("update").map_err(io_error)?.join("pyrotecnika-setup.exe");
    std::fs::write(&installer, bytes).map_err(io_error)?;
    std::process::Command::new(&installer).args(["/P", "/R"]).spawn().map_err(io_error)?;
    std::process::exit(0)
}

#[cfg(target_os = "windows")]
fn relaunch_os() -> std::io::Result<()> {
    std::process::Command::new(std::env::current_exe()?).spawn().map(|_| ())
}

/// macOS: tiene que ser un `.app` y no venir del `.pkg` (lo gestiona MDM;
/// `productbuild` deja un recibo con el identificador del bundle).
#[cfg(target_os = "macos")]
fn detect(exe: &Path) -> bool {
    app_bundle(exe).is_some() && !Path::new("/var/db/receipts/io.pyrotecnika.app.plist").exists()
}

#[cfg(target_os = "macos")]
fn expected_format() -> &'static str {
    "app"
}

#[cfg(target_os = "macos")]
fn app_bundle(exe: &Path) -> Option<PathBuf> {
    // …/Pyrotecnika.app/Contents/MacOS/pyrotecnika
    let bundle = exe.parent()?.parent()?.parent()?;
    (bundle.extension()? == "app").then(|| bundle.to_path_buf())
}

/// Descomprime el `.app.tar.gz` junto al bundle actual y lo intercambia. Si
/// algo falla a mitad, se restaura el original.
#[cfg(target_os = "macos")]
fn apply(bytes: Vec<u8>) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(io_error)?;
    let bundle = app_bundle(&exe).ok_or("No se encontró Pyrotecnika.app.")?;
    let staging = fresh_temp_dir("update").map_err(io_error)?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(std::io::Cursor::new(bytes)));
    archive.set_preserve_permissions(true);
    archive.unpack(&staging).map_err(io_error)?;
    let new_app = std::fs::read_dir(&staging)
        .map_err(io_error)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "app"))
        .ok_or("El paquete descargado no contiene una aplicación.")?;

    let backup = staging.join("anterior.app");
    std::fs::rename(&bundle, &backup).map_err(io_error)?;
    if let Err(e) = std::fs::rename(&new_app, &bundle) {
        let _ = std::fs::rename(&backup, &bundle);
        return Err(io_error(e));
    }
    let _ = std::fs::remove_dir_all(&staging);
    // Que el Finder y LaunchServices vean la versión nueva.
    let _ = std::process::Command::new("/usr/bin/touch").arg(&bundle).status();
    Ok(())
}

#[cfg(target_os = "macos")]
fn relaunch_os() -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    let bundle = app_bundle(&exe).ok_or_else(|| std::io::Error::other("sin bundle .app"))?;
    // `open -n` abre una instancia nueva aunque esta siga viva unos instantes.
    std::process::Command::new("/usr/bin/open").arg("-n").arg(bundle).spawn().map(|_| ())
}

/// Linux: solo la AppImage (`$APPIMAGE` lo define su cargador). Los `.deb` y
/// `.rpm` se actualizan con el gestor de paquetes.
#[cfg(target_os = "linux")]
fn detect(_exe: &Path) -> bool {
    std::env::var_os("APPIMAGE").is_some()
}

#[cfg(target_os = "linux")]
fn expected_format() -> &'static str {
    "appimage"
}

/// Escribe la AppImage nueva al lado y la intercambia de forma atómica.
#[cfg(target_os = "linux")]
fn apply(bytes: Vec<u8>) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let current = PathBuf::from(std::env::var_os("APPIMAGE").ok_or("No se encontró la AppImage.")?);
    let mut tmp = current.clone().into_os_string();
    tmp.push(".nueva");
    let tmp = PathBuf::from(tmp);
    let mode = std::fs::metadata(&current).map(|m| m.permissions().mode()).unwrap_or(0o755) | 0o111;
    std::fs::write(&tmp, bytes).map_err(io_error)?;
    let renamed = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode))
        .and_then(|()| std::fs::rename(&tmp, &current));
    if let Err(e) = renamed {
        let _ = std::fs::remove_file(&tmp);
        return Err(io_error(e));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn relaunch_os() -> std::io::Result<()> {
    let appimage = std::env::var_os("APPIMAGE").ok_or_else(|| std::io::Error::other("sin $APPIMAGE"))?;
    std::process::Command::new(appimage).spawn().map(|_| ())
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn detect(_exe: &Path) -> bool {
    false
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn expected_format() -> &'static str {
    ""
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn apply(_bytes: Vec<u8>) -> Result<(), String> {
    Err("Este sistema no admite actualizaciones automáticas.".into())
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn relaunch_os() -> std::io::Result<()> {
    Err(std::io::Error::other("sistema no admitido"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_clave_publica_incluida_es_valida() {
        let raw = base64::engine::general_purpose::STANDARD.decode(PUBKEY.trim()).unwrap();
        minisign_verify::PublicKey::decode(std::str::from_utf8(&raw).unwrap()).unwrap();
    }

    /// Firmado con la llave real (`cargo packager signer sign`), el mismo
    /// formato que produce release.yml.
    #[test]
    fn acepta_la_firma_del_pipeline_y_rechaza_datos_alterados() {
        let data = include_bytes!("../tests/fixtures/paquete.txt");
        let signature = include_str!("../tests/fixtures/paquete.txt.sig");
        verify(data, signature).expect("la firma real debe ser válida");
        let mut altered = data.to_vec();
        altered[0] ^= 1;
        assert!(verify(&altered, signature).is_err());
    }

    #[test]
    fn rechaza_firmas_que_no_son_validas() {
        assert!(verify(b"datos", "no-es-base64!").is_err());
        let fake = base64::engine::general_purpose::STANDARD.encode("untrusted comment: x\nabc\n");
        assert!(verify(b"datos", &fake).is_err());
    }

    #[test]
    fn lee_el_manifiesto_de_cargo_packager() {
        let json = r#"{"version":"0.2.0","notes":"Notas","pub_date":"2026-11-16T00:00:00Z",
            "platforms":{"windows-x86_64":{"url":"https://example.com/a.exe","signature":"c2ln","format":"nsis"}}}"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        assert_eq!(m.version, "0.2.0");
        assert_eq!(m.platforms["windows-x86_64"].format, "nsis");
    }

    /// Consulta real a GitHub. Necesita red: `cargo test -p pyro-platform -- --ignored`.
    #[test]
    #[ignore]
    fn consulta_real_a_github() {
        match check("0.0.1") {
            Ok(Some(a)) => println!("disponible {} (instalable: {})", a.version, a.can_install),
            Ok(None) => println!("al día"),
            Err(e) => println!("error: {e}"),
        }
    }

    #[test]
    fn ninguna_copia_de_desarrollo_se_actualiza_sola() {
        // `cargo test` corre desde target/: no hay uninstall.exe, .app ni $APPIMAGE.
        if std::env::var_os("APPIMAGE").is_none() {
            assert!(!self_updating());
        }
    }
}
