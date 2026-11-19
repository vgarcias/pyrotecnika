# Fase 3: no bloqueo y memoria — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** que nada congele la interfaz más de 50 ms, que el pincel vaya a 60 fps en 4096×4096, que deshacer guarde solo lo que cambió y que ningún archivo dañado cierre la app — todo medido.

**Architecture:** las capas pasan a `Arc<RgbaImage>` con copia al escribir y un id estable; el historial guarda deltas inversos por bloques de 256×256; un crate nuevo `pyro-services::jobs` corre abrir, filtros y exportar en segundo plano con progreso y cancelación (el núcleo solo ve un trait `Monitor`); el lienzo se sube a la GPU en texturas de ≤ 1024×1024 y solo el rectángulo sucio.

**Tech Stack:** Rust 1.85, egui/eframe 0.31 (wgpu), egui_kittest 0.31, image 0.25, rayon, cargo-fuzz (nightly, solo CI).

**Spec:** `docs/superpowers/specs/2026-11-19-fase3-no-bloqueo-memoria-design.md`

## Global Constraints

- Ninguna operación ocupa el hilo de la interfaz más de 50 ms; cancelar responde en < 100 ms.
- Trazo en 4096×4096 con 3 capas: trabajo de CPU por fotograma < 16 ms (p95).
- Bloques del historial: 256×256. Presupuesto 768 MB. Tope 200 pasos.
- Límites al abrir y en «Nuevo»: ≤ 32 768 px por lado y ancho × alto × 4 ≤ 2 GB (2 147 483 648 bytes); `max_alloc` del decodificador 2 GB.
- Mensaje exacto: «La imagen es demasiado grande (A×B px). Pyrotecnika abre imágenes de hasta 32 768 px por lado y 2 GB por capa.»
- `pyro-core`, `pyro-formats` y `pyro-services` no dependen de egui/eframe/winit (regla del CI).
- `cfg(target_os)` solo en `pyro-platform`; sin `Color32` literales fuera de `crates/pyro-ui/src/design/` (salvo WHITE y TRANSPARENT).
- `#![forbid(unsafe_code)]` en `pyro-core`, `pyro-formats` y `pyro-services`.
- `cargo deny`: sin GPL/LGPL/AGPL/MPL. `rayon` (MIT/Apache) está permitido.
- Textos en español; colores neutros; las capturas de referencia existentes no cambian.
- El CI usa un rustc más nuevo: literales flotantes con tipo explícito donde la inferencia dude (`2.0_f32`).
- Rust no está en el PATH: `export PATH="/c/Users/v_gar/.cargo/bin:$PATH"`.

## Review Focus

1. **Deshacer después de cambiar la estructura** (borrar una capa pintada, recortar, combinar, duplicar y luego pintar la copia) → cada deshacer devuelve exactamente el estado anterior. Lo fija `deshacer_y_rehacer_secuencias_al_azar` (Tarea 2).
2. **Cancelar justo cuando la tarea ya terminó** (el resultado llega en el mismo cuadro en que se pulsa Cancelar) → no se aplica. Lo fija `cancelar_gana_al_resultado` (Tarea 5) y `abrir_y_cancelar_deja_el_documento` (Tarea 7).
3. **Mover el control del filtro muy deprisa** (decenas de vistas previas encadenadas) → solo se aplica la última, el lienzo nunca muestra una vieja tras una nueva, y «Cancelar» restaura los píxeles exactos. Lo fija `vista_previa_solo_la_ultima` (Tarea 8).
4. **Imagen más grande que la textura máxima de la GPU pero dentro de los límites** (p. ej. 9000×300) → se abre y se ve; antes se rechazaba. Lo fija `lienzo_mayor_que_un_bloque` (Tarea 6).
5. **Pintar con una tarea de exportación en curso** → el pincel funciona y la exportación produce la imagen del momento en que empezó. Lo fija `exportar_mientras_se_pinta` (Tarea 8).

---

### Task 1: Capas compartidas y composición rápida

**Files:**
- Modify: `crates/pyro-core/Cargo.toml` (añadir `rayon = "1"`; también en `[workspace.dependencies]` de `Cargo.toml`)
- Modify: `crates/pyro-core/src/document.rs`, `crates/pyro-core/src/paint.rs`, `crates/pyro-core/src/filters.rs`, `crates/pyro-core/src/selection.rs`
- Modify (adaptar llamadas, 16 sitios con `.pixels`): `crates/pyro-formats/src/{export,io}.rs`, `crates/pyro-ui/src/app/{dialogs,mod,tools,view}.rs`

**Interfaces:**
- Produces:
  - `pub struct LayerId(pub u64)` (`Copy, Eq, Hash, Debug`); `Layer { pub id: LayerId, pub name, pub visible, pub opacity, pub pixels: Arc<RgbaImage> }`. Ids nuevos de un `AtomicU64` global en `Layer::new`/`Layer::from_image`.
  - `Layer::image_mut(&mut self) -> &mut RgbaImage` (`Arc::make_mut`).
  - `Document::duplicate_layer` da a la copia un id nuevo.
  - `Document::composite_rect(&self, x0: u32, y0: u32, x1: u32, y1: u32) -> RgbaImage` (x1/y1 exclusivos, recortados al lienzo) — paralela por filas con rayon; atajos: opacidad efectiva 1 y alfa 255 → copia; alfa 0 → nada.
  - `Document::composite(&self) -> RgbaImage` = `composite_rect` del lienzo entero.
  - `Document::composite_pixel(&self, x: u32, y: u32) -> [u8; 4]`.
  - `Stroke::new(kind, base: Arc<RgbaImage>, clone_offset)` — la base es el `Arc` de la capa antes del trazo (sin copiar).

- [ ] **Step 1: Pruebas (en `document.rs`)**

```rust
#[test]
fn clonar_el_documento_no_copia_pixeles() {
    let doc = Document::new(64, 64, Some([1, 2, 3, 255]));
    let snap = doc.clone();
    assert!(Arc::ptr_eq(&doc.layers[0].pixels, &snap.layers[0].pixels));
    let mut doc = doc;
    doc.layers[0].image_mut().put_pixel(0, 0, Rgba([9, 9, 9, 255]));
    assert_eq!(snap.layers[0].pixels.get_pixel(0, 0).0, [1, 2, 3, 255]);
}

#[test]
fn duplicar_da_un_id_nuevo() {
    let mut doc = Document::new(8, 8, None);
    doc.duplicate_layer();
    assert_ne!(doc.layers[0].id, doc.layers[1].id);
    assert_eq!(doc.clone().layers[1].id, doc.layers[1].id);
}

#[test]
fn componer_un_rectangulo_igual_que_recortar_el_total() {
    let doc = doc_de_prueba(300, 200); // 3 capas, opacidades 1.0/0.5/0.8, una oculta no
    let full = doc.composite();
    let part = doc.composite_rect(37, 11, 250, 199);
    assert_eq!(part, imageops::crop_imm(&full, 37, 11, 213, 188).to_image());
    assert_eq!(doc.composite_pixel(100, 50), full.get_pixel(100, 50).0);
}
```
`doc_de_prueba` es un ayudante de pruebas con píxeles deterministas (gradiente + alfa variable).

- [ ] **Step 2:** `cargo test -p pyro-core` → no compila (`image_mut`, `id`, `composite_rect` no existen).
- [ ] **Step 3:** Implementar lo de Interfaces y adaptar los llamadores (escrituras vía `image_mut()`, asignaciones con `Arc::new(img)`). Los filtros siguen devolviendo `RgbaImage`. El trazo del pincel en `tools.rs` pasa `self.doc.active_layer().pixels.clone()` (el `Arc`).
- [ ] **Step 4:** `cargo test --workspace` → todo verde, incluidas las capturas (`cargo test -p pyro-ui -- --ignored` sin `UPDATE_SNAPSHOTS`).
- [ ] **Step 5:** commit `feat(core): shared copy-on-write layers with stable ids; parallel rect compositing`.

### Task 2: Historial por diferencias

**Files:**
- Modify: `crates/pyro-core/src/history.rs` (reescritura interna; API pública igual salvo lo añadido)

**Interfaces:**
- Consumes: `LayerId`, `Layer::pixels: Arc<RgbaImage>` (Tarea 1).
- Produces (además de `push`, `undo`, `redo`, `discard_last`, `clear`, `mark_saved`, `is_modified`, `can_undo`, `can_redo`, que no cambian de firma):
  - `pub const TILE: u32 = 256; pub const MEMORY_BUDGET: usize = 768 << 20; pub const MAX_STEPS: usize = 200;`
  - `History::seal(&mut self, current: &Document)` — convierte el paso pendiente en delta; la UI lo llama al soltar el pincel. `push`, `undo` y `redo` lo llaman solos.
  - `History::bytes(&self) -> usize` — memoria contada de los pasos.

**Modelo (lo que el código no determina solo):** cada paso es un *delta inverso*: describe el estado anterior respecto del siguiente. `push(&before)` guarda `before` (clon barato) como pendiente; `seal(current)` lo compara con `current`:
- por cada capa de `before`: si existe en `current` el mismo `id` con las mismas medidas → solo los bloques de 256×256 cuyo contenido difiere (bytes del estado anterior); si el `Arc` es el mismo, cero bloques; si no existe o cambió de medidas → la capa entera (`Arc`);
- metadatos (nombre, visible, opacidad, orden, `active`, `width`, `height`) siempre; selección: `None`/igual a la actual → nada, si no la máscara entera.
`undo(doc)`: sella lo pendiente, reconstruye el estado anterior aplicando el delta a `doc` y, mientras escribe cada bloque, guarda el bloque que había → ese es el delta de rehacer (simétrico). `bytes()` suma bloques + capas enteras + máscaras; al pasar `MEMORY_BUDGET` o `MAX_STEPS` se descartan los pasos más viejos. `position`/`saved` se comportan como hoy.

- [ ] **Step 1: Pruebas**

```rust
#[test]
fn deshacer_y_rehacer_secuencias_al_azar() {
    // 600×400 (bloques parciales). Generador congruencial fijo, 300 operaciones:
    // pintar un rectángulo en la capa activa, añadir, duplicar, borrar, subir,
    // bajar, combinar, recortar, cambiar opacidad/visibilidad, seleccionar/limpiar.
    // Cada operación: history.push(&doc) antes, y se guarda doc.clone() en `estados`.
    // Luego: deshacer hasta el principio comparando cada estado (píxeles,
    // ids, metadatos, selección); rehacer hasta el final comparando igual.
}

#[test]
fn un_trazo_pequeno_cuesta_un_bloque() {
    let mut doc = Document::new(4096, 4096, Some([255, 255, 255, 255]));
    let mut h = History::default();
    h.push(&doc);
    doc.layers[0].image_mut().put_pixel(10, 10, Rgba([0, 0, 0, 255]));
    h.seal(&doc);
    assert!(h.bytes() <= 256 * 256 * 4 + 4096, "{}", h.bytes());
}

#[test]
fn doscientos_pasos_y_presupuesto() {
    // 250 trazos de un píxel en sitios distintos: can_undo 200 veces, no 201.
    // Una capa de 16384×16384 (1 GB) cambiada entera dos veces: bytes() <= MEMORY_BUDGET
    // y queda al menos 1 paso (como hoy: mínimo 1).
}
```
Las tres pruebas existentes (`undo_redo_roundtrip`, `modified_follows_the_saved_position`, `saved_state_in_a_lost_redo_branch_stays_modified`) se mantienen sin cambios.

- [ ] **Step 2:** `cargo test -p pyro-core history` → fallan `seal`/`bytes` (no existen).
- [ ] **Step 3:** Implementar. Comparar bloques por filas (`&[u8]` de cada fila del bloque).
- [ ] **Step 4:** `cargo test --workspace` → verde. En la UI, `end_drag`/fin de trazo llama a `self.history.seal(&self.doc)`.
- [ ] **Step 5:** commit `feat(core): undo history stores 256×256 tile deltas`.

### Task 3: Filtros por bandas, cancelables

**Files:**
- Create: `crates/pyro-core/src/task.rs`
- Modify: `crates/pyro-core/src/{lib,filters}.rs`

**Interfaces:**
- Produces:
  - `pub struct Cancelled;` (`Debug`, `Display` «Cancelado», `Error`).
  - `pub trait Monitor: Sync { fn cancelled(&self) -> bool; fn progress(&self, fraction: f32); }` y `pub struct NoMonitor;` (nunca cancela).
  - `Filter::run(&self, img: &RgbaImage, sel: Option<&Mask>, m: &dyn Monitor) -> Result<RgbaImage, Cancelled>` — resultado igual a `apply_masked(img, &self.apply(img), sel)`. Bandas de 128 filas en paralelo (rayon); filtros de vecindad (desenfoque, enfocar) leen un margen de `ceil(3σ) + 2` filas por arriba y abajo. Consulta `m.cancelled()` antes de cada banda; informa progreso al acabar cada una.

- [ ] **Step 1: Pruebas**

```rust
#[test]
fn por_bandas_igual_que_entero() {
    let img = imagen_de_prueba(301, 517); // ruido determinista con alfa
    let sel = Some(Mask::ellipse(301, 517, (20.0, 30.0), (280.0, 500.0)));
    for f in [Filter::GAUSSIAN_BLUR, Filter::GaussianBlur { sigma: 30.0 }, Filter::SHARPEN,
              Filter::LEVELS, Filter::HUE_SATURATION, Filter::BRIGHTNESS_CONTRAST,
              Filter::Invert, Filter::Grayscale] {
        let want = apply_masked(&img, &f.apply(&img), sel.as_ref());
        assert_eq!(f.run(&img, sel.as_ref(), &NoMonitor).unwrap(), want, "{}", f.name());
    }
}

#[test]
fn cancelar_detiene_el_filtro() {
    // Monitor que cancela tras la primera banda: run → Err(Cancelled) y se
    // llamó a progress menos veces que el número de bandas.
}
```
- [ ] **Step 2:** `cargo test -p pyro-core filters` → no compila.
- [ ] **Step 3:** Implementar. Si una banda con margen no reproduce exactamente el borde del desenfoque de `imageops`, ampliar el margen (no relajar la prueba).
- [ ] **Step 4:** `cargo test -p pyro-core` → verde.
- [ ] **Step 5:** commit `feat(core): banded, cancellable filters`.

### Task 4: Límites al abrir, CRC de Fireworks y fuzzing

**Files:**
- Modify: `crates/pyro-formats/src/{io,fwpng}.rs`
- Create: `crates/pyro-formats/tests/archivos_danados.rs`
- Create: `fuzz/Cargo.toml` (su propio `[workspace]`), `fuzz/fuzz_targets/{open_bytes,inspect_png}.rs`
- Create: `.github/workflows/fuzz.yml`
- Modify: `Cargo.toml` (`exclude = ["fuzz"]`)

**Interfaces:**
- Consumes: `Monitor` (Tarea 3).
- Produces:
  - `pub const MAX_SIDE: u32 = 32_768; pub const MAX_LAYER_BYTES: u64 = 2 << 30;`
  - `pub struct TooLarge { pub width: u32, pub height: u32 }` (`Error`; `Display` = el mensaje exacto de Global Constraints).
  - `pub fn check_size(width: u32, height: u32) -> Result<(), TooLarge>` (también la usa «Nuevo»).
  - `open_bytes(bytes, m: &dyn Monitor) -> Result<Opened>` y `open_path(path, m)`: lee dimensiones con `ImageReader::with_guessed_format()?.into_dimensions()`, aplica `check_size`, decodifica con `Limits { max_image_width/height: Some(MAX_SIDE), max_alloc: Some(MAX_LAYER_BYTES) }`; comprueba `m.cancelled()` antes y después de decodificar (`Cancelled` dentro del `anyhow::Error`).
  - `PngChunk.crc_ok: bool`; `FwInfo::damaged(&self) -> bool` (algún bloque privado con CRC malo).

- [ ] **Step 1: Pruebas** (`tests/archivos_danados.rs`, usando un ayudante que fabrica PNG con IHDR arbitrario y CRC correcto vía `crc32fast`)

```rust
#[test] fn lado_demasiado_grande() { /* IHDR 40000×10 → downcast TooLarge{40000,10}, en < 100 ms */ }
#[test] fn capa_de_mas_de_2_gb() { /* IHDR 30000×30000 → TooLarge; 32768×16384 → check_size Ok */ }
#[test] fn ninguno_cierra_la_app() {
    // truncados en 20 posiciones de un PNG válido, IHDR con longitud imposible,
    // CRC de IHDR roto, GIF/WebP/BMP/TIFF truncados, bytes al azar (LCG fijo, 200 casos):
    // open_bytes(..) devuelve Ok o Err; nunca panic (catch_unwind no hace falta: un panic falla la prueba).
}
#[test] fn fireworks_con_crc_roto_abre_aplanado() {
    // fake_fireworks_png con un byte de un bloque privado cambiado:
    // Ok, png_info.is_fireworks(), png_info.damaged()
}
```
- [ ] **Step 2:** `cargo test -p pyro-formats` → no compila (`TooLarge`, `damaged`).
- [ ] **Step 3:** Implementar. Actualizar los llamadores (`NoMonitor` donde aún no hay tarea). Fuzz: objetivos `open_bytes(data, &NoMonitor)` y `inspect_png(data)`; `fuzz.yml`: `schedule` nocturno + `workflow_dispatch`, ubuntu, toolchain nightly, `cargo install cargo-fuzz --locked`, `cargo fuzz run <t> -- -max_total_time=600` por objetivo, sube `fuzz/artifacts` si falla.
- [ ] **Step 4:** `cargo test --workspace` y `cargo deny check` → verde. (El fuzz no corre en Windows; se verifica en el CI tras el push.)
- [ ] **Step 5:** commit `feat(formats): decode limits, Fireworks chunk CRC check, damaged-file corpus and fuzz targets`.

### Task 5: Crate `pyro-services` con tareas

**Files:**
- Create: `crates/pyro-services/{Cargo.toml,src/lib.rs,src/jobs.rs}`
- Modify: `Cargo.toml` (`pyro-services` en `[workspace.dependencies]`), `.github/workflows/ci.yml` (añadir `pyro-services` al bucle de la regla «no dependen de la interfaz»), `crates/pyro-ui/Cargo.toml`

**Interfaces:**
- Consumes: `pyro_core::task::{Monitor, Cancelled}`.
- Produces:
  - `pub enum JobError { Cancelled, Failed(String) }`; `impl From<Cancelled> for JobError`.
  - `pub struct JobCtx` (implementa `Monitor`); `JobCtx::check(&self) -> Result<(), JobError>`.
  - `pub type Wake = Arc<dyn Fn() + Send + Sync>;`
  - `pub struct Job<T>`: `Job::spawn(wake: Wake, work: impl FnOnce(&JobCtx) -> Result<T, JobError> + Send + 'static) -> Job<T>`; `progress(&self) -> Option<f32>` (`None` = sin porcentaje todavía); `cancel(&self)`; `is_cancelled(&self) -> bool`; `poll(&mut self) -> JobPoll<T>`; `Drop` cancela.
  - `pub enum JobPoll<T> { Running, Done(T), Failed(String), Cancelled }` — si se pidió cancelar, `poll` devuelve `Cancelled` aunque el trabajo haya terminado con éxito.
  - `wake` se llama en cada cambio de progreso (como mucho cada 50 ms) y al terminar.

- [ ] **Step 1: Pruebas** (en `jobs.rs`, esperando con un bucle de `poll` y `sleep(1ms)` con tope de 2 s)

```rust
#[test] fn termina_y_avisa() { /* work devuelve Ok(42): Done(42); wake llamado ≥ 1 vez */ }
#[test] fn informa_progreso() { /* work pone 0.5 y espera una barrera: progress() == Some(0.5) */ }
#[test] fn cancelar_responde() { /* work en bucle con ctx.check()?: cancel() → Cancelled en < 100 ms */ }
#[test] fn cancelar_gana_al_resultado() { /* work devuelve Ok enseguida; esperar a que acabe; cancel(); poll → Cancelled */ }
#[test] fn error() { /* Err(Failed("x")) → Failed("x") */ }
```
- [ ] **Step 2:** `cargo test -p pyro-services` → falla (crate vacío).
- [ ] **Step 3:** Implementar con `std::thread` + `mpsc` + `AtomicBool`/`AtomicU32` (progreso como bits de `f32`). `#![forbid(unsafe_code)]`.
- [ ] **Step 4:** `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo tree -p pyro-services -e normal | grep -E "egui|eframe|winit"` → sin salida.
- [ ] **Step 5:** commit `feat(services): background jobs with progress and cancellation`.

### Task 6: Lienzo por bloques y rectángulo sucio

**Files:**
- Create: `crates/pyro-ui/src/app/canvas_tiles.rs`
- Modify: `crates/pyro-ui/src/app/{view,canvas,tools,mod}.rs`

**Interfaces:**
- Consumes: `Document::composite_rect`, `composite_pixel` (Tarea 1).
- Produces:
  - `pub(super) struct CanvasTiles` con `tile: u32` (= `min(1024, max_texture_side)`), `update(&mut self, ctx, doc: &Document, dirty: Dirty, opts: TextureOptions)` y `paint(&self, painter: &Painter, canvas_rect: Rect)` (dibuja cada bloque en su sitio con el mismo `uv` y escala que hoy).
  - `pub(super) enum Dirty { None, Rect { x0: u32, y0: u32, x1: u32, y1: u32 }, All }` con `union(self, other) -> Dirty`.
  - `PyroApp::canvas_dirty: bool` se sustituye por `PyroApp::dirty: Dirty`; `mark_dirty()` = `All`; `mark_rect(x0, y0, x1, y1)`. El pincel marca el rectángulo de cada pincelada; todo lo demás, `All`.
  - Al actualizar: se recompone solo `dirty` (`composite_rect`) y se sube con `TextureHandle::set_partial` a cada bloque que toca; con `All` o cambio de medidas, se recrean los bloques.
  - Se quita la comprobación de `max_texture_side` de `open_path`.
  - El cuentagotas usa `composite_pixel`.

- [ ] **Step 1: Pruebas** (kittest en un módulo nuevo `app/canvas_tests.rs`)

```rust
#[test] fn lienzo_mayor_que_un_bloque() {
    // Documento 9000×300 abierto con reset_document: CanvasTiles tiene 9 bloques,
    // ninguno de lado > 1024; tras run_steps no hay panic.
}
#[test] fn un_trazo_solo_sube_su_rectangulo() {
    // Lienzo 3000×3000: tras un clic de pincel en (10,10) con radio 5,
    // el último Dirty procesado es Rect dentro de (0,0)-(20,20) y solo 1 bloque se actualizó
    // (CanvasTiles cuenta `uploads` en pruebas).
}
```
Las capturas de referencia existentes sirven de prueba de que lo visible no cambia.
- [ ] **Step 2:** `cargo test -p pyro-ui canvas` → no compila.
- [ ] **Step 3:** Implementar.
- [ ] **Step 4:** `cargo test --workspace` y `cargo test -p pyro-ui -- --ignored` → verde sin regenerar capturas.
- [ ] **Step 5:** commit `feat(ui): tiled canvas textures with dirty-rect uploads`.

### Task 7: Tareas en la interfaz y abrir en segundo plano

**Files:**
- Create: `crates/pyro-ui/src/app/tasks.rs`, `crates/pyro-ui/src/app/tasks_tests.rs`
- Modify: `crates/pyro-ui/src/app/{mod,panels,input,tools,scenes,guard}.rs`, `crates/pyro-ui/src/app/dialogs.rs` (límite en «Nuevo»)

**Interfaces:**
- Consumes: `Job`, `JobPoll`, `Wake` (Tarea 5); `open_path(path, &dyn Monitor)`, `TooLarge`, `check_size`, `FwInfo::damaged` (Tarea 4).
- Produces:
  - `pub(super) enum TaskKind { Open(PathBuf), Filter(Filter) }` y `pub(super) struct DocTask { kind, job, generation: u64 }`; campo `PyroApp::task: Option<DocTask>` y `PyroApp::generation: u64` (sube en `reset_document`).
  - `PyroApp::busy(&self) -> bool` (= `task.is_some()`).
  - `PyroApp::start_task(&mut self, kind, job)`, `PyroApp::poll_task(&mut self, ctx)` (llamado al principio de `update`): `Done` con la generación vigente → aplica; generación distinta → descarta; `Failed` → `fail(...)` como hoy; `Cancelled` → nada, aviso «Cancelado.».
  - `Action::is_view_only(self) -> bool` (zoom, encajar, tamaño real, tema, escala de UI, paleta, preferencias); con `busy()`, `available()` es `false` para el resto, `run()` los ignora y las herramientas del lienzo no actúan (`begin_drag` y `click` vuelven sin hacer nada).
  - Barra de estado con tarea: «Abriendo «nombre»…» / «Aplicando «filtro»…», `egui::ProgressBar` (sin porcentaje → animada) de 160 px y botón «Cancelar».
  - Esc: si hay tarea y ningún diálogo/ventana abierta (paleta, preferencias, confirmación, filtro, exportar, nuevo, actualizaciones) → cancela la tarea.
  - Abrir (`Pending::Open`) lanza la tarea `Open`; al terminar hace lo que hoy hace `open_path` (recientes, `source_path`, mensaje). Si `png_info.damaged()`, el mensaje de Fireworks termina en « Sus datos de Fireworks están dañados.». Un `TooLarge` muestra su texto tal cual.
  - Cerrar la ventana con una tarea en curso: se pregunta como hoy si hay cambios; al cerrar se cancela.

- [ ] **Step 1: Pruebas** (`tasks_tests.rs`; ayudante `wait_task(&mut h)` = `run_steps(1)` hasta `!busy()` con tope de 2 s)

```rust
#[test] fn abrir_en_segundo_plano() { /* PNG 64×32 temporal: request(Open) → busy(); wait_task → doc 64×32, recientes contiene la ruta */ }
#[test] fn abrir_y_cancelar_deja_el_documento() { /* request(Open); pulsar «Cancelar» en el mismo cuadro; wait_task → doc y historial iguales que antes, estado «Cancelado.» */ }
#[test] fn esc_cancela_la_tarea() { /* igual con Key::Escape */ }
#[test] fn durante_la_tarea_no_se_edita() { /* tarea en curso (job que espera una barrera): pulsar G no cambia herramienta; clic en el lienzo con pincel no cambia píxeles; Ctrl+K sí abre la paleta */ }
#[test] fn resultado_de_otro_documento_se_descarta() { /* lanzar Open, llamar reset_document(Document::new(5,5,None)) antes de que acabe; tras wait_task el doc sigue 5×5 */ }
#[test] fn imagen_demasiado_grande_avisa() { /* PNG con IHDR 40000×10: tras wait_task, estado de error con el texto exacto */ }
#[test] fn nuevo_respeta_los_limites() { /* NewDialog 40000×10 → botón Crear deshabilitado o aviso con el texto exacto; doc sin cambiar */ }
```
Capturas nuevas (escena `Scene::Task` en `scenes.rs`, tarea simulada al 40 %): `tarea-en-curso-{claro,oscuro,alto-contraste}.png`.
- [ ] **Step 2:** `cargo test -p pyro-ui tasks` → no compila.
- [ ] **Step 3:** Implementar. `wake` = clon de `ctx` con `request_repaint()`.
- [ ] **Step 4:** `cargo test --workspace`; generar solo las capturas nuevas con `UPDATE_SNAPSHOTS=1 cargo test -p pyro-ui -- --ignored tarea`, revisarlas a la vista, y comprobar que las demás no cambian (`git status` solo muestra las nuevas).
- [ ] **Step 5:** commit `feat(ui): background document tasks with progress and cancel; open runs in the background`.

### Task 8: Filtros y exportar como tareas

**Files:**
- Modify: `crates/pyro-ui/src/app/{dialogs,mod,guard}.rs`, `crates/pyro-ui/src/app/tasks_tests.rs`

**Interfaces:**
- Consumes: `Filter::run` (Tarea 3), `Job` (Tarea 5), `DocTask`/`TaskKind::Filter` (Tarea 7).
- Produces:
  - `FilterDialog { filter: Filter, backup: Arc<RgbaImage>, preview: Option<(Filter, Job<RgbaImage>)>, shown: Option<Filter>, apply_requested: bool }`.
  - Al cambiar un control: cancelar la vista previa en curso y lanzar otra con `(filter, backup, selección)`. Al terminar: la capa activa pasa a ese resultado, `shown = Some(filter)`, `mark_dirty()`. «Aplicar»: si `shown == Some(filter)` cierra; si no, `apply_requested` y el botón muestra «Aplicando…» hasta que llegue. «Cancelar» y Esc: cancelar, capa = `backup`, `history.discard_last()`.
  - Filtros sin parámetros (Invertir, Escala de grises) → `TaskKind::Filter` con barra de estado.
  - `ExportDialog::job: Option<Job<(Vec<u8>, RgbaImage)>>`; mientras corre, el diálogo muestra `ProgressBar` animada y «Cancelar» (cancela y deja el diálogo abierto sin resultado). Exportar no cuenta como `busy()`.
  - Cerrar la app con una exportación en curso la cancela sin preguntar (el `Drop` de `Job` cancela).

- [ ] **Step 1: Pruebas**

```rust
#[test] fn vista_previa_solo_la_ultima() {
    // Diálogo de desenfoque: cambiar sigma 20 veces seguidas (state_mut + marcar cambio);
    // wait hasta preview None: la capa es Filter::run(backup, sigma final); Cancelar → píxeles == backup.
}
#[test] fn aplicar_espera_a_la_vista_previa() { /* cambiar y pulsar Aplicar en el mismo cuadro: tras esperar, la capa es la del valor final y el historial puede deshacer 1 paso */ }
#[test] fn invertir_en_segundo_plano() { /* run(Filter(Invert)) → busy(); wait_task → píxeles invertidos; Ctrl+Z restaura */ }
#[test] fn exportar_mientras_se_pinta() {
    // Abrir exportar (lanza la tarea); antes de que acabe, pintar un trazo;
    // al terminar, la vista previa de exportación es la imagen anterior al trazo y el trazo está en el documento.
}
#[test] fn cancelar_exportacion() { /* «Cancelar» del diálogo → sin bytes, diálogo abierto */ }
```
- [ ] **Step 2:** `cargo test -p pyro-ui tasks` → fallan.
- [ ] **Step 3:** Implementar.
- [ ] **Step 4:** `cargo test --workspace` y capturas sin cambios (`-- --ignored`).
- [ ] **Step 5:** commit `feat(ui): filter previews, filters and export run as background tasks`.

### Task 9: Medición y documentación

**Files:**
- Create: `crates/pyro-core/tests/medir.rs`, `crates/pyro-ui/src/app/medir_tests.rs`
- Modify: `.github/workflows/ci.yml` (paso `cargo test --release --workspace -- --ignored medir` en el job de pruebas, solo Linux), `docs/ARQUITECTURA.md`, `DESIGN.md`

**Interfaces:**
- Consumes: todo lo anterior.
- Produces: pruebas `#[ignore]` con nombre que empieza por `medir_`; cada una imprime `medir <nombre>: <ms> ms (límite <ms>)` y solo falla por tiempo con `PYRO_MEDIR_ESTRICTO=1`.

- [ ] **Step 1: Pruebas** (documento de 4096×4096 con 3 capas con contenido; cada medida = mediana de 5, el trazo = p95 de 200 fotogramas)
  - `medir_trazo_por_fotograma` (pyro-ui: pincelada + `composite_rect` del rectángulo + preparar `ColorImage` del bloque) < 16 ms.
  - `medir_sellar_historial_tras_trazo` < 50 ms.
  - `medir_operaciones_en_el_hilo_de_la_ui`: varita (contigua y no), bote, recortar, combinar, duplicar, invertir selección, borrar selección, `composite` entero, `composite_pixel`, deshacer, rehacer — cada una < 50 ms.
  - `medir_memoria_del_historial`: 100 trazos de radio 20 → imprime `bytes()`; < 64 MB.
  - Si alguna operación supera su límite: optimizarla (rayon/recorte al rectángulo) o pasarla a `TaskKind`; se anota como Ruling.
- [ ] **Step 2:** `PYRO_MEDIR_ESTRICTO=1 cargo test --release --workspace -- --ignored medir` → anotar resultados.
- [ ] **Step 3:** Documentación: ARQUITECTURA §1.2 (jobs reales: `pyro-services::jobs`, vista previa a resolución completa), §1.3 (enfoque B; A como evolución), tabla RNF: P3, P4, E1, S1 con los números medidos y la fecha, RF-10 ◐ (falta el panel, fase 4), §9.1 fase 3 ✔; DESIGN.md: componente «tarea en curso» (barra de estado: texto, barra de progreso, Cancelar; Esc).
- [ ] **Step 4:** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, reglas del CI (OS, colores, dependencias de interfaz) → todo verde.
- [ ] **Step 5:** commit `docs: phase 3 measurements, architecture and design updates`.
