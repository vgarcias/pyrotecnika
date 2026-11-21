# Fase 4: documentos, guardado y recuperación — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** varias imágenes en pestañas, Guardar con capas en `.ora`, autoguardado con recuperación tras un fallo, log de fallos y panel de historial.

**Architecture:** el estado de cada documento se agrupa en `Tab`; `PyroApp` tiene la pestaña activa en `self.tab` y las demás en `self.others` (cambiar de pestaña intercambia), así el código actual sigue trabajando sobre la activa. `.ora` se lee y escribe en `pyro-formats::ora` (zip + quick-xml); autoguardado, sesiones y log de fallos viven en `pyro-services` (sin egui); la interfaz solo lanza tareas y muestra ventanas.

**Tech Stack:** Rust 1.85, egui/eframe 0.31, egui_kittest 0.31, image 0.25, zip (solo `deflate`), quick-xml, rayon.

**Spec:** `docs/superpowers/specs/2026-11-21-fase4-documentos-recuperacion-design.md`

## Global Constraints

- Reglas del CI: sin egui/eframe/winit en `pyro-core`, `pyro-formats`, `pyro-services`; `cfg(target_os)` solo en `pyro-platform`; sin `Color32` literales fuera de `crates/pyro-ui/src/design/` (salvo WHITE y TRANSPARENT); `cargo deny` sin GPL/LGPL/AGPL/MPL; `#![forbid(unsafe_code)]` en core, formats y services.
- El clippy del CI es más nuevo que el local: nada de bucles por índice sobre vectores (`for (i, x) in v.iter().enumerate()`), literales flotantes con tipo donde dude (`2.0_f32`), y ninguna prueba que compare etiquetas de atajos literales (usar `Chord::label(is_mac())`).
- RNF-P4: guardar, autoguardar, recuperar, abrir y cambiar de pestaña no bloquean la interfaz más de 50 ms.
- Autoguardado cada 120 s; latido de sesión cada 30 s; sesión muerta si su latido tiene más de 180 s.
- Presupuesto de historial: 768 MB para todas las pestañas juntas; tope de 200 pasos por pestaña.
- `.ora`: OpenRaster 0.0.6; `mimetype` primero y sin comprimir con `image/openraster`; miniatura ≤ 256×256; límites: 32 768 px por lado y 2 GB por capa (fase 3), ≤ 8 GB descomprimido en total, ≤ 1 000 capas.
- Textos exactos: «Sin título N», «Cambios sin guardar», «Guardando…», «Este archivo usa funciones que Pyrotecnika aún no admite; se abrió aproximado.», «Pasos anteriores descartados por memoria», «nombre (recuperado)», «Cambio».
- Atajos nuevos de fábrica: Guardar `Ctrl+S`, Guardar como `Ctrl+Shift+S`, Cerrar pestaña `Ctrl+W`, Pestaña siguiente `Ctrl+Tab`, Pestaña anterior `Ctrl+Shift+Tab`.
- Rust no está en el PATH: `export PATH="/c/Users/v_gar/.cargo/bin:$PATH"`. No encadenar commit tras una comprobación con tubería: mirar antes el código de salida.

## Review Focus

1. **Tarea de una pestaña que termina con otra activa** (filtro en «A», el usuario pasa a «B») → el resultado cambia «A» y solo «A», con su paso de historial. Lo fija `tarea_de_otra_pestana_se_aplica_a_la_suya` (Tarea 2).
2. **Guardar y seguir pintando** (pintar durante el guardado en segundo plano) → el archivo tiene la imagen del momento de guardar y la pestaña sigue con «•». Lo fija `editar_mientras_se_guarda_mantiene_la_marca` (Tarea 4).
3. **`.ora` con capas desplazadas o más grandes que el lienzo, o con rutas `../`** → se recortan o rechazan con aviso; nunca se lee fuera del zip ni cae la app. Lo fija `capas_desplazadas_y_rutas_raras` (Tarea 3).
4. **Dos ventanas de Pyrotecnika a la vez** → ninguna ofrece recuperar los autoguardados vivos de la otra. Lo fija `sesion_viva_no_se_ofrece` (Tarea 6).
5. **Cerrar la app con una pestaña sin título y otra con archivo, ambas con cambios, y cancelar el diálogo de «Guardar como»** → no se cierra nada ni se pierde nada. Lo fija `cancelar_guardar_como_cancela_el_cierre` (Tarea 5).

---

### Task 1: Estado por pestaña (sin cambiar el comportamiento)

**Files:**
- Create: `crates/pyro-ui/src/app/tab.rs`
- Modify: `crates/pyro-ui/src/app/{mod,tools,view,canvas,dialogs,tasks,panels,input,guard,scenes,palette,prefs}.rs` y las pruebas que tocan esos campos

**Interfaces:**
- Produces:
  - `pub(super) struct Tab` con los campos por documento: `doc`, `history`, `clone_source`, `clone_stroke_offset`, `zoom`, `pan`, `fit_pending`, `canvas`, `dirty`, `sel_tiles`, `sel_dirty`, `drag`, `filter_dialog`, `export_dialog`, `after_export`, `source_path`, `png_info`, `task`, `generation`, más `uid: u64` (único en la sesión, de un `AtomicU64`) y `untitled: u32` (número de «Sin título N», 0 si tiene archivo).
  - `Tab::new(doc: Document, untitled: u32) -> Tab`.
  - `PyroApp::tab: Tab` (la activa). El resto de campos de `PyroApp` siguen siendo comunes.
  - Todo `self.doc`, `self.history`, … pasa a `self.tab.doc`, `self.tab.history`, … (reemplazo mecánico campo a campo; nunca `Deref`, rompe el préstamo dividido de `self.tab.doc` con `&self.brush`).
  - `reset_document` sigue reemplazando el documento de la pestaña activa (la Tarea 2 cambia abrir y nuevo a «pestaña nueva»).

- [ ] **Step 1:** Prueba nueva en `tab.rs`: `una_pestana_al_arrancar` — la app recién creada tiene `tab.untitled == 1`, `tab.history.can_undo() == false` y `tab.uid` distinto del de otra `Tab::new`.
- [ ] **Step 2:** `cargo test -p pyro-ui una_pestana` → no compila (`tab` no existe).
- [ ] **Step 3:** Mover los campos y reemplazar los accesos.
- [ ] **Step 4:** `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p pyro-ui -- --ignored capturas` → todo verde y las capturas sin cambios (es una reorganización).
- [ ] **Step 5:** commit `refactor(ui): group per-document state in Tab`.

### Task 2: Varias pestañas

**Files:**
- Create: `crates/pyro-ui/src/app/tabs.rs` (lógica y barra), `crates/pyro-ui/src/app/tabs_tests.rs`
- Modify: `crates/pyro-ui/src/commands/catalog.rs`, `crates/pyro-ui/src/app/{mod,panels,tasks,guard,scenes}.rs`, `crates/pyro-core/src/history.rs`

**Interfaces:**
- Consumes: `Tab` (Tarea 1).
- Produces:
  - `PyroApp::others: Vec<Tab>` y `PyroApp::active: usize` — orden visible = `others[..active] ++ [tab] ++ others[active..]`.
  - `PyroApp::tab_count(&self) -> usize`, `switch_to(&mut self, i: usize)` (intercambia; marca el lienzo sucio), `add_tab(&mut self, tab: Tab)` (la inserta tras la activa y la activa), `close_active_tab(&mut self)` (si era la última, deja una «Sin título» nueva), `find_tab(&self, path: &Path) -> Option<usize>` (por `source_path` u `ora_path`, rutas normalizadas como en recientes), `tab_title(&self, i: usize) -> String` (nombre de archivo o «Sin título N»), `next_untitled(&self) -> u32`.
  - Abrir: si `find_tab` lo encuentra, la activa; si no, la tarea de abrir crea al terminar una pestaña nueva (ya no reemplaza la activa). Nuevo: pestaña nueva «Sin título N». Una pestaña «Sin título» sin cambios e intacta (sin pasos) se reemplaza en vez de quedar vacía detrás.
  - Las tareas de las pestañas inactivas se recogen cada cuadro (`poll_task` para `self.tab` y para cada `others[i]`); el resultado de un filtro se aplica a la pestaña dueña. `TaskOutput::Opened` siempre crea pestaña.
  - Catálogo: `window.next` «Pestaña siguiente» `Ctrl+Tab`, `window.prev` «Pestaña anterior» `Ctrl+Shift+Tab`, `window.close` «Cerrar pestaña» `Ctrl+W`; categoría nueva `Window` («Ventana»); menú Ventana tras Ver con esos tres comandos y, debajo, la lista de pestañas (la activa marcada). `Action::NextTab`, `Action::PrevTab`, `Action::CloseTab`, `Action::SwitchTab(usize)`.
  - Barra de pestañas: `TopBottomPanel::top` bajo el menú; cada pestaña es un `active_row`-like de ancho según el nombre (máx. 200, recorta con «…»), «•» si `history.is_modified()`, botón «×» con nombre accesible «Cerrar «nombre»»; la activa invertida; desplazamiento horizontal si no caben.
  - Cerrar pestaña con cambios: la confirmación actual (`Pending::CloseTab`), sin el botón Guardar hasta la Tarea 4.
  - `History::drop_oldest(&mut self) -> bool` y `PyroApp::enforce_history_budget(&mut self)`: mientras la suma de `bytes()` de todas las pestañas pase de `MEMORY_BUDGET`, quita el paso más viejo de la que más ocupa (si ninguna puede, para). Se llama tras cada `push`/`seal`.

- [ ] **Step 1: Pruebas** (`tabs_tests.rs`, con `wait_task` de `tasks_tests`)
  - `abrir_dos_archivos_da_dos_pestanas`: abrir a.png y b.png → `tab_count() == 2`, activa b, títulos «a.png», «b.png».
  - `pintar_en_una_no_cambia_la_otra`: pintar en b; `switch_to(0)`: píxeles de a sin cambios; `can_undo` de a falso.
  - `tarea_de_otra_pestana_se_aplica_a_la_suya`: en a, `run(Filter(Invert))`; antes de que acabe `switch_to(1)`; `wait_task` sobre todas → a invertida (comparando con `Filter::Invert.run`), b intacta, `can_undo` de a verdadero.
  - `abrir_un_archivo_ya_abierto_activa_su_pestana`: abrir a, b, a → 2 pestañas, activa a.
  - `atajos_de_pestanas`: Ctrl+Tab pasa de 0 a 1 y de 1 a 0; Ctrl+Mayús+Tab al revés; Ctrl+W cierra la activa (sin cambios, sin preguntar).
  - `cerrar_con_cambios_pregunta`: pintar y Ctrl+W → `confirm` es `Pending::CloseTab`; Cancelar → siguen 2 pestañas.
  - `cerrar_la_ultima_deja_sin_titulo`: con una pestaña, Ctrl+W → 1 pestaña «Sin título 2».
  - `presupuesto_comun`: dos pestañas de 8192×8192 cambiadas enteras 3 veces cada una → suma de `bytes()` ≤ `MEMORY_BUDGET`.
- [ ] **Step 2:** `cargo test -p pyro-ui tabs_tests` → no compila.
- [ ] **Step 3:** Implementar; regenerar `docs/ATAJOS.md` (`PYRO_WRITE_ATAJOS=1 cargo test -p pyro-ui tabla_de_atajos`) y la prueba `atajos_de_fabrica_de_fireworks` con los tres atajos nuevos.
- [ ] **Step 4:** `cargo test --workspace`; capturas: la barra aparece en todas las escenas → `UPDATE_SNAPSHOTS=1 cargo test -p pyro-ui -- --ignored capturas`, revisar a la vista que solo cambia la franja de la barra (escena principal con 3 pestañas: añadir escena `Scene::Tabs` con «cenote.png», «logo.ora •», «Sin título 3»).
- [ ] **Step 5:** commit `feat(ui): documents in tabs`.

### Task 3: Leer y escribir `.ora`

**Files:**
- Create: `crates/pyro-formats/src/ora.rs`, `crates/pyro-formats/tests/ora.rs`, `fuzz/fuzz_targets/open_ora.rs`
- Modify: `Cargo.toml` (`zip = { version = "2", default-features = false, features = ["deflate"] }`, `quick-xml = "0.37"`), `crates/pyro-formats/{Cargo.toml,src/lib.rs,src/io.rs}`, `fuzz/Cargo.toml`, `.github/workflows/fuzz.yml`

**Interfaces:**
- Consumes: `check_size`, `TooLarge`, `Monitor` (fase 3).
- Produces:
  - `pub fn encode_ora(doc: &Document, m: &dyn Monitor) -> Result<Vec<u8>>` (capas de arriba abajo en `stack.xml`, `data/layerN.png`, `mergedimage.png` = `doc.composite()`, miniatura ≤ 256 lado mayor, `pyrotecnika:selected="true"` en la activa, `xmlns:pyrotecnika="https://pyrotecnika.app/ora"` en `<image>`).
  - `pub fn write_ora(doc: &Document, path: &Path, m: &dyn Monitor) -> Result<()>`: escribe `path` + `.tmp` en la misma carpeta y renombra; si falla, `path` no cambia.
  - `pub struct OraOpened { pub document: Document, pub warnings: Vec<String> }`; `pub fn decode_ora(bytes: &[u8], m: &dyn Monitor) -> Result<OraOpened>`.
  - `io::open_bytes`: si los bytes son un zip cuyo `mimetype` es `image/openraster`, usa `decode_ora`; `Opened` gana `warnings: Vec<String>` y `is_ora: bool`. `OPEN_EXTENSIONS` incluye `"ora"`.
  - Lectura: `w`/`h` del `<image>` con `check_size`; capas en orden inverso a `stack.xml`; `x`/`y` desplazan y se recorta al lienzo (aviso «Parte de alguna capa quedaba fuera del lienzo y se recortó.»); grupos aplanados en orden; `composite-op` distinto de `svg:src-over`, o `<text>`/`<filter>`: aviso único del Global Constraints; sin `stack.xml` pero con `mergedimage.png`: aplanado con aviso; `src` con `..`, absoluto o inexistente: error «El archivo .ora está dañado: falta la capa «src».»; > 1 000 capas o > 8 GB descomprimidos (suma de `size()` del zip): `TooLarge`-like con mensaje «El archivo .ora es demasiado grande.».

- [ ] **Step 1: Pruebas** (`tests/ora.rs`)
  - `ida_y_vuelta`: documento 300×200 con 3 capas (nombres con tildes, opacidad 0.4, una oculta, activa la del medio) → `encode_ora` → `decode_ora`: mismas medidas, nombres, opacidad (±0.001), visibilidad, activa y píxeles exactos; sin avisos; `mergedimage.png` decodificado == `doc.composite()`; `mimetype` es la primera entrada y sin comprimir.
  - `estilo_gimp_y_krita`: zips fabricados en la prueba con el `stack.xml` típico de GIMP (`composite-op="svg:src-over"`, `selected="true"` de GIMP) y de Krita (grupos anidados, `x`/`y`) → abren con el número de capas esperado; el de Krita sin avisos salvo el de grupos aplanados si los lleva.
  - `capas_desplazadas_y_rutas_raras`: capa con `x="-50"` y más ancha que el lienzo → recortada + aviso; `src="../x.png"` y `src="/etc/x.png"` → error; `src` inexistente → error.
  - `danado_nunca_cae`: zip truncado en 15 puntos, `stack.xml` inválido, sin `mimetype`, PNG de capa corrupto, 1 001 capas declaradas, cabecera de capa 40 000×10 → todos `Err` o `Ok` con aviso, ninguno `panic`.
  - `escritura_atomica`: `write_ora` a una ruta cuya carpeta no existe → `Err` y nada creado; sobre un archivo existente con un documento distinto → contenido nuevo completo (se puede decodificar).
- [ ] **Step 2:** `cargo test -p pyro-formats --test ora` → no compila.
- [ ] **Step 3:** Implementar. `cargo deny check` tras añadir `zip` y `quick-xml`; si alguna licencia no pasa, Ruling y alternativa (escribir XML a mano y leer con un lector mínimo propio).
- [ ] **Step 4:** `cargo test --workspace`, `cargo deny check`, `cd fuzz && cargo check` → verde. `fuzz.yml`: añadir `open_ora` a la matriz.
- [ ] **Step 5:** commit `feat(formats): OpenRaster (.ora) read and write`.

### Task 4: Guardar y Guardar como

**Files:**
- Modify: `crates/pyro-ui/src/app/{tab,tasks,mod,guard,dialogs,panels}.rs`, `crates/pyro-ui/src/commands/catalog.rs`, `crates/pyro-core/src/history.rs`, `crates/pyro-ui/src/app/tabs_tests.rs`

**Interfaces:**
- Consumes: `write_ora`, `Opened::is_ora` (Tarea 3); pestañas (Tarea 2).
- Produces:
  - `Tab::ora_path: Option<PathBuf>`; al abrir un `.ora`, `ora_path = Some(path)`.
  - `History::position(&self) -> u64` y `History::mark_saved_at(&mut self, position: u64)`.
  - Catálogo: `file.save` «Guardar» `Ctrl+S`, `file.save_as` «Guardar como…» `Ctrl+Shift+S`, antes de Exportar; `Action::Save`, `Action::SaveAs`.
  - `TaskKind::Save { path: PathBuf, position: u64, then: Option<Pending> }` — no cuenta para `busy()` (no bloquea la edición), sí para cerrar (cerrar espera: ver abajo). Al terminar bien: `mark_saved_at(position)`, `ora_path = Some(path)`, título de pestaña `nombre.ora`, recientes, borrar autoguardado (Tarea 6 lo conecta; aquí un `fn after_save(&mut self, uid)` vacío), y si `then` es `Some`, continúa con esa acción. Al fallar: aviso con la causa, marca sin cambios, `then` se descarta.
  - Guardar sin `ora_path` = Guardar como. Guardar como: `rfd` con filtro «OpenRaster» `ora`, carpeta del `source_path` u `ora_path`, nombre `stem.ora`; si la ruta elegida es el `source_path` de un PNG (no `.ora`) se rechaza con aviso (nunca sobrescribir el original).
  - Exportar ya no llama a `mark_saved`. La barra de estado dice «Cambios sin guardar».
  - Confirmación (`confirm_window`) rehecha: «Guardar», «Descartar», «Cancelar» (foco en Cancelar); «Guardar» lanza `Save` con `then = Some(pending)`.
  - Cerrar (pestaña o app) con un `Save` en curso: se espera; la barra dice «Guardando…»; el cierre sigue al terminar bien.

- [ ] **Step 1: Pruebas** (en `tabs_tests.rs`; el diálogo de archivo se sustituye en pruebas por `PyroApp::save_dialog_override: Option<Option<PathBuf>>`, `#[cfg(test)]`)
  - `guardar_un_documento_nuevo_pide_ruta`: override → `dir/x.ora`; Ctrl+S → tras `wait`, existe `x.ora`, se decodifica con las capas, la pestaña no tiene «•», título «x.ora».
  - `guardar_como_propone_ora_junto_al_origen`: abrir `dir/cenote.png`; la ruta propuesta (función pura `suggested_save_path(&Tab) -> PathBuf`) es `dir/cenote.ora`.
  - `nunca_sobrescribe_el_png`: override → `dir/cenote.png` (el origen) → aviso de error, el PNG intacto (bytes iguales).
  - `editar_mientras_se_guarda_mantiene_la_marca`: lanzar guardar y pintar antes de que acabe → archivo sin el trazo, pestaña con «•».
  - `exportar_ya_no_quita_la_marca`: pintar, exportar y guardar el export → `is_modified()` sigue verdadero.
  - `guardar_y_cerrar`: pintar, Ctrl+W, «Guardar» → tras `wait`, archivo escrito y pestaña cerrada.
  - `fallo_al_guardar_avisa_y_no_cierra`: override a una carpeta inexistente, Ctrl+W, «Guardar» → estado de error, la pestaña sigue.
- [ ] **Step 2:** fallan (no compila).
- [ ] **Step 3:** Implementar; regenerar `docs/ATAJOS.md`.
- [ ] **Step 4:** `cargo test --workspace`; capturas: solo cambia la de confirmar (botones) → regenerar esa y revisarla.
- [ ] **Step 5:** commit `feat(ui): save and save as OpenRaster`.

### Task 5: Cerrar la app con varias pestañas con cambios

**Files:**
- Modify: `crates/pyro-ui/src/app/{guard,mod,tabs}.rs`, `crates/pyro-ui/src/app/tabs_tests.rs`, `crates/pyro-ui/src/app/scenes.rs`

**Interfaces:**
- Consumes: Guardar (Tarea 4).
- Produces: `Pending::Quit` con más de una pestaña con cambios abre `quit_window`: lista de pestañas con cambios (título), «Guardar todo», «Descartar todo», «Cancelar» (foco en Cancelar). «Guardar todo»: guarda en orden; las que no tienen `ora_path` piden ruta una a una; si se cancela un diálogo de ruta o un guardado falla, se cancela el cierre (lo ya guardado queda guardado). Con una sola pestaña con cambios se usa la confirmación normal.

- [ ] **Step 1: Pruebas**
  - `cerrar_con_varias_pregunta_una_vez`: dos pestañas con cambios, `Pending::Quit` → `quit_window` abierta con los dos títulos.
  - `guardar_todo_y_salir`: una con `ora_path`, otra sin; override → ruta válida → ambas guardadas, `closing == true`.
  - `cancelar_guardar_como_cancela_el_cierre`: override → `None` (cancelado) para la sin título → `closing == false`, ambas pestañas abiertas, la otra puede quedar guardada.
  - `descartar_todo`: → `closing == true` sin escribir nada.
- [ ] **Step 2:** fallan.
- [ ] **Step 3:** Implementar; escena `Scene::QuitMany` para captura en oscuro.
- [ ] **Step 4:** `cargo test --workspace`; captura nueva revisada.
- [ ] **Step 5:** commit `feat(ui): quitting with several unsaved tabs`.

### Task 6: Autoguardado y sesiones

**Files:**
- Create: `crates/pyro-services/src/recovery.rs`, `crates/pyro-services/tests/recovery.rs`
- Modify: `crates/pyro-platform/src/paths.rs` (`data_dir`), `crates/pyro-services/{Cargo.toml,src/lib.rs}` (dependencias `serde`, `toml`, `pyro-formats`), `crates/pyro-ui/src/app/{mod,tab,tasks}.rs`

**Interfaces:**
- Consumes: `encode_ora`/`write_ora` (Tarea 3), `Tab::uid` (Tarea 1).
- Produces:
  - `pyro_platform::paths::data_dir() -> Option<PathBuf>`: `%LOCALAPPDATA%\Pyrotecnika`, `$HOME/Library/Application Support/io.Pyrotecnika`, `$XDG_DATA_HOME/pyrotecnika` o `$HOME/.local/share/pyrotecnika`.
  - `pub struct RecoveryEntry { pub session: PathBuf, pub uid: u64, pub title: String, pub source_path: Option<PathBuf>, pub ora_path: Option<PathBuf>, pub saved_at: SystemTime }`.
  - `pub struct Recovery` con `Recovery::start(base: PathBuf) -> io::Result<Recovery>` (crea `base/recovery/<inicio-ms>-<pid>/` con `session.toml` = pid, inicio, latido), `heartbeat(&self)`, `autosave(&self, uid: u64, title: &str, source: Option<&Path>, ora: Option<&Path>, doc: &Document) -> Result<()>` (escribe `uid.ora` atómico + `uid.toml`), `remove(&self, uid: u64)`, `finish(self)` (borra su carpeta), `orphans(base: &Path, now: SystemTime) -> Vec<RecoveryEntry>` (de sesiones con latido > 180 s, sin contar la propia), `discard(entry: &RecoveryEntry)` (borra sus dos archivos y la carpeta si queda vacía), `adopt(&self, entry: &RecoveryEntry)` no existe: el viejo se borra solo cuando la sesión nueva autoguarda o guarda o cierra esa pestaña (`Tab::recovered_from: Option<RecoveryEntry>`).
  - En la UI: `PyroApp::recovery: Option<Recovery>` (None en pruebas salvo que se pida); cada cuadro, si pasaron 30 s, `heartbeat`; si pasaron 120 s, para cada pestaña con `history.is_modified()` y `history.position() != tab.autosaved_position`, una tarea `TaskKind::Autosave { position }` (no bloquea; si termina, `autosaved_position = Some(position)` y se borra el `recovered_from`); `request_repaint_after(30 s)` para que el reloj avance sin eventos. Guardar bien o cerrar la pestaña → `remove(uid)`; cerrar la app normalmente → `finish`.

- [ ] **Step 1: Pruebas** (`tests/recovery.rs`, con `tempdir` y tiempos inyectados vía `now`)
  - `autoguardar_y_listar_huerfanos`: sesión A autoguarda 2 docs; se simula latido viejo (reescribir `session.toml` con latido de hace 10 min); `orphans(base, now)` desde otra sesión → 2 entradas con título y rutas; los `.ora` se decodifican.
  - `sesion_viva_no_se_ofrece`: A con latido reciente → `orphans` vacío.
  - `cierre_normal_no_deja_nada`: `finish()` → carpeta de A borrada.
  - `descartar`: `discard(entry)` borra sus archivos.
  - `remove_tras_guardar`: `autosave` y `remove(uid)` → no queda `uid.ora`.
  - En `pyro-ui` (`tabs_tests.rs`): `autoguarda_cada_dos_minutos` con un reloj de prueba (`PyroApp::clock_offset: Duration`, `#[cfg(test)]`): pintar, avanzar 121 s → existe el `.ora` de la pestaña; sin cambios no se escribe nada.
- [ ] **Step 2:** fallan.
- [ ] **Step 3:** Implementar. `cargo deny check`.
- [ ] **Step 4:** `cargo test --workspace` y regla «sin egui» del CI para `pyro-services`.
- [ ] **Step 5:** commit `feat(services): autosave with per-session recovery folders`.

### Task 7: Recuperar al arrancar

**Files:**
- Create: `crates/pyro-ui/src/app/recover.rs`
- Modify: `crates/pyro-ui/src/app/{mod,tasks,scenes}.rs`, `crates/pyro-ui/src/app/tabs_tests.rs`

**Interfaces:**
- Consumes: `Recovery::orphans`, `discard`, `RecoveryEntry` (Tarea 6); `decode_ora` (Tarea 3).
- Produces: al crear la app con `data_dir`, si `orphans` no está vacío, `PyroApp::recover_dialog: Option<Vec<RecoveryEntry>>` y una ventana modal «Recuperar trabajo» con una fila por entrada («título — hace N min») y «Recuperar», «Descartar», «Decidir después». Recuperar: una tarea `TaskKind::Recover(entry)` por entrada que abre el `.ora` en una pestaña nueva con título, `source_path`, `ora_path` de la entrada, marcada con cambios (`history` con un paso «Recuperado» que no se puede deshacer más allá) y `recovered_from = Some(entry)`; si su archivo ya está abierto en otra pestaña, título «nombre (recuperado)». Inmediatamente después se autoguarda en la sesión nueva (no espera 120 s). Descartar: `discard` de todas. Decidir después: cierra la ventana sin tocar nada.

- [ ] **Step 1: Pruebas**
  - `ofrece_recuperar_tras_un_fallo`: preparar en `tempdir` una sesión vieja con un autoguardado de 40×30 de «cenote.png» → crear la app con esa carpeta → `recover_dialog` con 1 entrada; Recuperar → tras `wait`, una pestaña «cenote.png» de 40×30 con «•».
  - `decidir_despues_conserva`: → ventana cerrada y archivos en su sitio; otra app nueva vuelve a ofrecerlo.
  - `descartar_borra`: → sin archivos y sin ventana en la siguiente.
  - `recuperado_de_un_archivo_abierto`: con «cenote.png» ya abierto, recuperar → título «cenote.png (recuperado)».
- [ ] **Step 2:** fallan.
- [ ] **Step 3:** Implementar; escena `Scene::Recover` (2 entradas) para captura en oscuro.
- [ ] **Step 4:** `cargo test --workspace`; captura revisada.
- [ ] **Step 5:** commit `feat(ui): offer to recover autosaved work after a crash`.

### Task 8: Log de fallos

**Files:**
- Create: `crates/pyro-services/src/crashlog.rs`, `crates/pyro-services/tests/crashlog.rs`
- Modify: `crates/pyro-services/src/lib.rs`, `app/pyrotecnika/src/main.rs`, `app/pyrotecnika/Cargo.toml`

**Interfaces:**
- Produces: `pub fn install_panic_hook(dir: PathBuf)` (guarda el hook anterior y lo llama después de escribir), `pub fn panic_entry(message: &str, location: &str, backtrace: &str, home: Option<&Path>) -> String` (hora, mensaje, ubicación y traza; cada aparición de `home` se sustituye por `~`), `pub fn prune(dir: &Path, now: SystemTime, keep_days: u64)` (borra `pyrotecnika-AAAA-MM-DD.log` de más de 7 días). Un archivo por día: `logs/pyrotecnika-AAAA-MM-DD.log`. `main.rs` lo instala con `data_dir()/logs` antes de `run_native`.

- [ ] **Step 1: Pruebas** (`tests/crashlog.rs`, binario propio porque el hook es global)
  - `un_panic_escribe_en_el_log`: `install_panic_hook(tmp)`; `std::thread::spawn(|| panic!("prueba de fallo")).join()` → existe un `.log` de hoy que contiene «prueba de fallo» y `crashlog.rs`.
  - `sin_rutas_del_usuario`: `panic_entry("no se pudo abrir C:\\Users\\ana\\fotos\\x.png", …, Some(Path::new("C:\\Users\\ana")))` contiene `~\fotos\x.png` y no `C:\Users\ana`.
  - `poda_de_siete_dias`: archivos de hace 3 y 10 días → `prune` deja el de 3.
- [ ] **Step 2:** fallan.
- [ ] **Step 3:** Implementar.
- [ ] **Step 4:** `cargo test --workspace`.
- [ ] **Step 5:** commit `feat(services): crash log via panic hook`.

### Task 9: Panel de historial

**Files:**
- Create: `crates/pyro-ui/src/app/history_panel.rs`
- Modify: `crates/pyro-core/src/history.rs`, todos los llamadores de `history.push` (unos 20 en `crates/pyro-ui/src/app/`), `crates/pyro-ui/src/app/{panels,scenes}.rs`

**Interfaces:**
- Consumes: `History` (fase 3).
- Produces:
  - `History::push(&mut self, doc: &Document, label: &'static str)`; los deltas guardan su `label`.
  - `History::new(origin: &'static str) -> History` (sustituye a `default()` donde hay documento: «Abierto», «Nuevo documento», «Recuperado») y `clear(origin)`.
  - `History::steps(&self) -> HistorySteps { pub labels: Vec<&'static str>, pub current: usize, pub trimmed: bool }` — `labels[0]` es el origen; `current` el índice del estado actual.
  - `History::go_to(&mut self, doc: &mut Document, index: usize) -> bool` (deshace o rehace hasta ese índice).
  - Nombres exactos por llamador según spec §6.6 («Pincel», «Borrador», «Tampón», «Desenfocar», «Mover», «Bote de pintura», «Selección», «Deseleccionar», «Borrar selección», «Recortar», «Nueva capa», «Duplicar capa», «Eliminar capa», «Subir capa», «Bajar capa», «Combinar abajo», «Propiedades de capa», `filter.name()`); cualquier otro, «Cambio».
  - Panel: `CollapsingHeader` «Historial» bajo Capas, filas `active_row` (la actual invertida, las rehacibles en `text_disabled`), primera fila «Pasos anteriores descartados por memoria» si `trimmed`; clic → `go_to` + `after_history`; teclas ↑/↓/Intro con foco en la lista.

- [ ] **Step 1: Pruebas** (en `history.rs` y kittest)
  - `nombres_y_posicion`: push «Pincel», push «Nueva capa», undo → `steps()` = `["Nuevo documento", "Pincel", "Nueva capa"]`, `current == 1`.
  - `ir_a_un_paso`: tres pasos, `go_to(0)` → documento inicial; `go_to(3)` → final.
  - `descartados`: 205 pasos → `trimmed == true`, `labels.len() == 201`.
  - kittest `clic_en_el_historial_deshace`: pintar dos trazos y clic en la fila «Pincel» de arriba → `steps().current == 1`.
- [ ] **Step 2:** fallan.
- [ ] **Step 3:** Implementar; actualizar las pruebas existentes que llaman a `push(&doc)`.
- [ ] **Step 4:** `cargo test --workspace`; capturas: el panel de historial aparece en las escenas → regenerar, revisar.
- [ ] **Step 5:** commit `feat(ui): history panel with named steps`.

### Task 10: Documentación y cierre

**Files:**
- Modify: `docs/ARQUITECTURA.md`, `DESIGN.md`, `docs/ATAJOS.md`

- [ ] **Step 1:** ARQUITECTURA: RF-08, RF-09, RF-10, RF-20 ✔ (fase 4); §1.4 (autoguardado cada 2 min por sesión, log diario con 7 días, el hook no guarda); §1.5 (carpeta de datos y `recovery/`, `logs/`); tabla de dependencias (`zip`, `quick-xml`); RNF-E1 completo; §9.1 fase 4 ✔. DESIGN.md: barra de pestañas, ventana de recuperación, ventana de salir con varias pestañas, panel de historial.
- [ ] **Step 2:** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, reglas del CI (OS, colores, dependencias de interfaz), `cargo test -p pyro-ui -- --ignored capturas` → verde.
- [ ] **Step 3:** commit `docs: phase 4 architecture, design and shortcuts`.
