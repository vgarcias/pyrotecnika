# Pyrotecnika — Propuesta técnica multiplataforma

> Documento de arquitectura previo al desarrollo multiplataforma.
> Estado de partida: v0.1.0, ~2 950 líneas de Rust, solo Windows, núcleo de edición funcional.
> Fecha: 2026-11-11.

## Índice

1. [Punto de partida](#0-punto-de-partida)
2. [Propuesta de arquitectura](#1-propuesta-de-arquitectura)
3. [Comparativa y justificación del stack](#2-comparativa-y-justificación-del-stack)
4. [Estructura de directorios y módulos](#3-estructura-de-directorios-y-módulos)
5. [Sistema de diseño visual](#4-sistema-de-diseño-visual)
6. [Requisitos funcionales y no funcionales](#5-requisitos-funcionales-y-no-funcionales)
7. [Estrategia de compatibilidad multiplataforma](#6-estrategia-de-compatibilidad-multiplataforma)
8. [Plan de pruebas](#7-plan-de-pruebas)
9. [Estrategia de compilación y distribución](#8-estrategia-de-compilación-y-distribución)
10. [Hoja de ruta y decisiones pendientes](#9-hoja-de-ruta-y-decisiones-pendientes)

---

## 0. Punto de partida

| Área | Hoy | Problema para el objetivo |
|---|---|---|
| Núcleo (`src/lib.rs`: document, selection, paint, filters, history, export, io, fwpng) | Separado de la UI, sin dependencias de egui, 30+ pruebas unitarias | Bien. Mezcla lógica de edición con códecs/E/S en el mismo crate |
| UI (`src/app.rs`, 1 369 líneas) | Un solo archivo: herramientas, paneles, diálogos, entrada, texturas | Difícil de mantener; atajos fijos en código (`TOOLS`); sin sistema de diseño |
| Tareas en segundo plano | Solo la exportación (`std::thread::spawn` + `mpsc`) | Los filtros y la apertura de archivos grandes bloquean la UI |
| Historial | Copias completas del documento (≤768 MB, ≤50 pasos) | Con documentos 4K de varias capas quedan muy pocos pasos |
| Formato nativo | No existe: las capas se pierden al guardar | Sin recuperación ante fallos ni guardado con capas |
| Plataforma | `windows_subsystem`, `rfd` para diálogos | Nada específico de macOS/Linux; sin preferencias persistentes |
| Render | `eframe` 0.31 con backend `glow` (OpenGL) por defecto | OpenGL está obsoleto en macOS |

**Conclusión:** la base técnica (Rust + egui) sirve para el objetivo multiplataforma. Lo que hace falta es reorganizar, no reescribir: separar el código en crates, añadir una capa de plataforma, un sistema de diseño y la infraestructura de empaquetado.

---

## 1. Propuesta de arquitectura

Arquitectura por capas con dependencias en un solo sentido (de arriba abajo). El núcleo no conoce la UI ni el sistema operativo.

```
┌──────────────────────────────────────────────────────────────┐
│ app (bin)        raíz de composición: arranque, logging,     │
│                  panic hook, inyección de servicios           │
├──────────────────────────────────────────────────────────────┤
│ pyro-ui          egui: tema/tokens, widgets, paneles,        │
│                  herramientas, diálogos, registro de comandos │
├───────────────────────────┬──────────────────────────────────┤
│ pyro-services             │ pyro-platform                    │
│ jobs (pool + cancelación) │ portapapeles, notificaciones,    │
│ settings, recientes (DB)  │ menú nativo, rutas, llavero,     │
│ cliente HTTP, actualizador│ "mostrar en carpeta", tema SO    │
├───────────────────────────┴──────────────────────────────────┤
│ pyro-formats     abrir/exportar, OpenRaster (.ora), PNG-8,   │
│                  lector de bloques Fireworks, límites decode  │
├──────────────────────────────────────────────────────────────┤
│ pyro-core        Document, Layer, Mask, pintura, filtros,    │
│                  composición, historial por teselas           │
└──────────────────────────────────────────────────────────────┘
```

### 1.1 Principios

- **El núcleo es puro.** `pyro-core` no lee archivos, no usa hilos y no depende de egui. Recibe y devuelve buffers, así que se prueba de forma determinista y se puede reutilizar (CLI de procesamiento por lotes, plugins futuros).
- **Comandos en vez de llamadas directas.** El `enum Action` actual pasa a ser un **registro de comandos** (`CommandId` → nombre, categoría, atajo por defecto, `is_enabled(&State)`, `run`). El menú, los atajos de teclado, la paleta de comandos (Ctrl/⌘+K) y el menú nativo de macOS leen todos del mismo registro. Así los atajos son configurables y cada acción se puede hacer con teclado.
- **La plataforma va detrás de traits.** `pyro-platform` expone `trait Clipboard`, `trait Notifier`, `trait SecretStore`, etc., con una implementación por SO compilada con `#[cfg(target_os)]`, más una implementación falsa para pruebas.
- **La UI nunca espera.** Toda operación de más de ~16 ms (filtros, abrir, exportar, guardar, actualizar) es un `Job` en `pyro-services::jobs`.

### 1.2 Modelo de concurrencia

```
Hilo UI (egui, 60 fps)                       Pool de trabajo (rayon)
  │  submit(Job{input: Arc<Snapshot>}) ───────▶  ejecuta por teselas, en paralelo
  │                                              consulta CancelToken entre teselas
  │  ◀── JobEvent::Progress(f32) / Done(res) ──  envía por canal + ctx.request_repaint()
  │  aplica el resultado y lo registra en el historial
```

- Los filtros muestran primero una **vista previa a resolución reducida** (≤1 Mpx) y aplican el resultado completo en segundo plano.
- Las instantáneas son `Arc` (copia al escribir), así que lanzar un job no copia píxeles.
- Solo un job de escritura a la vez por documento; los de lectura (exportar) pueden ir en paralelo.

### 1.3 Historial por teselas

Las capas se guardan como teselas de 256×256 en `Arc<Tile>`. Un paso de deshacer guarda **solo las teselas modificadas**. Un trazo de pincel sobre un lienzo 4K pasa de costar ~33 MB por capa a unos pocos cientos de KB, y el presupuesto de 768 MB alcanza para cientos de pasos.

### 1.4 Errores y recuperación ante fallos

- Errores de dominio con `thiserror` en las librerías y `anyhow` solo en `app`. Ningún `unwrap()` en rutas de E/S.
- Se avisa al usuario con un toast no modal que incluye la causa ("No se pudo leer X: permiso denegado").
- **Autoguardado** del documento en `.ora` en la carpeta de recuperación cada 2 min si hay cambios (es un job de baja prioridad que no bloquea).
- **Panic hook:** escribe el log, intenta un guardado de emergencia y, en el siguiente arranque, ofrece "Recuperar documentos".
- **Logs** con `tracing` y archivo rotativo (7 días) en la carpeta de logs del SO. No se registran rutas completas de archivos del usuario, solo los nombres.

### 1.5 Datos locales, base de datos y APIs REST

| Necesidad | Solución | Ubicación |
|---|---|---|
| Preferencias | `serde` + TOML con versión de esquema y migraciones | `config_dir()/settings.toml` |
| Atajos personalizados | TOML (`keymap.toml`), se valida al cargar y se detectan conflictos | `config_dir()` |
| Recientes, presets de exportación y de pinceles | SQLite (`rusqlite` embebido, sin servidor) | `data_dir()/library.db` |
| Recuperación y logs | Archivos | `cache_dir()` / `data_local_dir()` |
| REST | `ureq` + `rustls`, **solo HTTPS**, *timeouts*, *user-agent* genérico | `pyro-services::net` |

Las rutas salen del crate `directories`, así que cada SO usa su convención (ver §6). Hoy REST solo se usa para el feed de actualizaciones. El módulo `net` es el punto de extensión para integraciones futuras (almacenamiento en la nube, bibliotecas de recursos).

---

## 2. Comparativa y justificación del stack

### 2.1 El requisito que decide la elección

Pyrotecnika es un **editor de píxeles**. Lo crítico es mover buffers de varios megapíxeles entre la lógica y la GPU en cada trazo con latencia baja. Este requisito pesa más que la comodidad de desarrollo.

| Criterio (peso) | **Rust + egui/wgpu** (actual) | Tauri + React/TS | Electron | Flutter | Qt 6 (C++/QML) | Slint / Iced (Rust) |
|---|---|---|---|---|---|---|
| Rendimiento con buffers de píxeles (×3) | **Excelente**: textura GPU directa, sin IPC | Malo: los píxeles cruzan IPC serializados o tienen que ir a WebGL/WASM | Malo, igual que Tauri, y más memoria | Bueno, pero vía FFI a un núcleo nativo | Excelente | Bueno |
| Mismo aspecto en los 3 SO (×3) | **Idéntico**: dibuja todo él mismo | Variable: WebView2, WKWebView y WebKitGTK difieren en motor, fuentes y bugs | Idéntico (incluye Chromium) | Idéntico | Alto | Idéntico |
| Memoria y arranque (×2) | **~40–80 MB, <0,5 s** | ~80–150 MB | 200–400 MB, 1–2 s | ~100 MB | ~80 MB | ~40 MB |
| Tamaño del binario (×1) | **~10–15 MB** | ~10 MB + webview del sistema | 90–150 MB | ~25 MB | 40+ MB con Qt | ~10 MB |
| Reutilizar el código existente (×3) | **100 %** | El núcleo sí; la UI se reescribe | El núcleo vía N-API; la UI se reescribe | Reescritura parcial (Dart) | Reescritura (C++) | El núcleo sí; la UI se reescribe |
| Accesibilidad (×2) | Buena: AccessKit integrado (UIA, NSAccessibility, AT-SPI) | Muy buena (DOM) | Muy buena | Buena | Muy buena | Parcial |
| Integración de escritorio (×2) | Media: crates aparte (`muda`, `arboard`, `notify-rust`, `keyring`) | Muy buena (plugins oficiales) | Muy buena | Media | Muy buena | Media |
| Licencia y coste (×1) | MIT/Apache | MIT/Apache | MIT | BSD | LGPL/comercial | GPL/comercial (Slint) o MIT (Iced) |

### 2.2 Decisión: mantener **Rust + egui**, con backend **wgpu**

- **Rendimiento:** el lienzo es una textura en la GPU que se actualiza por regiones. Con Tauri o Electron cada trazo tendría que cruzar la frontera nativo↔JS o pasar a un lienzo WebGL, con lo que el núcleo en Rust perdería sentido.
- **Consistencia:** egui dibuja cada píxel de la interfaz, así que el producto se ve igual en los tres SO. Esto cumple directamente el criterio de aceptación ("un mismo producto"). Tauri depende de tres motores web distintos.
- **No parece una web empaquetada:** es un binario nativo, sin navegador, que arranca en menos de medio segundo.
- **Continuidad:** se conserva todo el núcleo actual y sus pruebas.
- **Cambiar `glow` → `wgpu`:** usa Metal en macOS (OpenGL está obsoleto allí), DX12/Vulkan en Windows y Vulkan/GL en Linux. Además da compute shaders para acelerar filtros en el futuro.

**Debilidades asumidas y cómo se mitigan:**

| Debilidad de egui | Mitigación |
|---|---|
| Los menús no son nativos | En macOS, barra de menú global nativa con `muda`, generada desde el registro de comandos. En Windows y Linux, barra de menú egui dentro de la ventana con el sistema de diseño (es lo habitual en editores profesionales) |
| El lienzo no es accesible por naturaleza | Alternativas textuales y numéricas para todo (ver §5, RNF-A) |
| Modo inmediato: hay que cuidar el coste por frame | Repintado solo con eventos (`request_repaint` bajo demanda), texturas por teselas sucias |

Se descartan Tauri y Electron por los criterios con más peso. Flutter y Qt obligarían a reescribir la UI sin ninguna ventaja sobre lo que ya hay.

### 2.3 Dependencias objetivo

| Función | Crate | Notas |
|---|---|---|
| Ventana, render, a11y | `eframe` (features `wgpu`, `accesskit`, `persistence`, `wayland`, `x11`) | |
| Paralelismo | `rayon` | Filtros y composición por teselas |
| Diálogos de archivo | `rfd` | Nativo en los 3 SO; en Linux usa el portal XDG |
| Menú nativo macOS | `muda` | |
| Portapapeles (incluye imágenes) | `arboard` | |
| Notificaciones | `notify-rust` (Linux/macOS), `winrt-notification` (Windows) | |
| Llavero | `keyring` | Credential Manager, Keychain, Secret Service |
| Rutas | `directories` | |
| Abrir y "mostrar en carpeta" | `opener` | |
| Preferencias | `serde`, `toml` | |
| Base de datos | `rusqlite` (feature `bundled`) | |
| HTTP | `ureq` + `rustls` | Sin OpenSSL, la misma pila TLS en los 3 SO |
| Logs | `tracing`, `tracing-appender` | |
| Errores | `thiserror`, `anyhow` | |
| Iconos | Fuente Lucide (ISC) en subconjunto | Sin `resvg` (MPL-2.0) |
| Empaquetado y actualizaciones | `cargo-packager`, `cargo-packager-updater`, `cargo-generate-rpm` | Ver §8 |

---

## 3. Estructura de directorios y módulos

Se convierte el repositorio en un **workspace de Cargo**:

```
pyrotecnika/
├─ Cargo.toml                    # [workspace], versión y lints compartidos
├─ deny.toml                     # licencias y advisories (cargo-deny)
├─ packager.toml                 # configuración de cargo-packager
├─ crates/
│  ├─ pyro-core/                 # ← hoy: document, selection, paint, filters, history
│  │  └─ src/{document,layer,tile,selection,paint,filters,composite,history}.rs
│  ├─ pyro-formats/              # ← hoy: io, export, fwpng
│  │  └─ src/{open,export,png8,ora,fireworks,limits}.rs
│  ├─ pyro-services/
│  │  └─ src/{jobs,settings,library_db,net,updater,recovery}.rs
│  ├─ pyro-platform/
│  │  └─ src/{lib,clipboard,notify,menu,paths,secrets,reveal,theme}.rs
│  │     └─ os/{windows,macos,linux}.rs
│  └─ pyro-ui/                   # ← hoy: app.rs, partido en módulos
│     └─ src/
│        ├─ app.rs               # estado raíz y bucle update()
│        ├─ commands/            # registro, keymap, paleta de comandos
│        ├─ design/              # tokens, theme.rs, typography, icons/, motion
│        ├─ widgets/             # botones, sliders numéricos, swatches, toasts
│        ├─ panels/              # tools, layers, options, history, status
│        ├─ canvas/              # viewport, zoom/pan, overlays, texturas por tesela
│        ├─ tools/               # una herramienta por archivo (trait Tool)
│        └─ dialogs/             # nuevo, exportar, filtros, preferencias, atajos
├─ app/pyrotecnika/              # bin: main.rs (composición), icono, recursos
├─ assets/{fonts,icons,brand}/
├─ examples/fwinfo.rs
├─ benches/                      # criterion
├─ fuzz/                         # cargo-fuzz: decoders y lector Fireworks
├─ tests/golden/                 # imágenes de referencia
├─ packaging/{windows,macos,linux}/  # wxs/nsi, entitlements.plist, .desktop, metainfo
├─ docs/{ARQUITECTURA.md,INSTALACION.md,DESARROLLO.md}
└─ .github/workflows/{ci.yml,release.yml}
```

> **Estado tras la fase 0:** existen `pyro-core`, `pyro-formats`, `pyro-platform` (cola de archivos a abrir + manejador de macOS) y `pyro-ui`, con la interfaz partida en `app/{mod,tools,view,canvas,input,panels,dialogs}.rs` (ninguno pasa de 420 líneas). Los módulos de formatos conservan sus nombres (`io`, `export`, `fwpng`). `pyro-services` y las carpetas `commands/`, `design/` y `widgets/` llegan en las fases 1–3.

Regla de dependencias, comprobada en CI con `cargo tree`: `core ← formats ← services/platform ← ui ← app`. `pyro-core` y `pyro-formats` no pueden depender de `egui`.

---

## 4. Sistema de diseño visual

La identidad es propia ("Pyrotecnika": brasas, chispas) y no imita el aspecto de ningún producto existente. Los tokens viven en `pyro-ui/src/design/tokens.rs`, y ningún widget usa colores literales.

### 4.1 Color: tokens semánticos

| Token | Oscuro | Claro | Uso |
|---|---|---|---|
| `bg.app` | `#16171A` | `#F3F4F6` | Fondo de la ventana |
| `bg.panel` | `#1E2024` | `#FFFFFF` | Paneles laterales y barras |
| `bg.raised` | `#272A30` | `#F8F9FA` | Controles, popovers |
| `bg.canvas` | `#101114` | `#E4E6EA` | Alrededor del lienzo (neutro, no altera la percepción del color) |
| `border.subtle` | `#353941` | `#E1E4E8` | Divisores decorativos |
| `border.control` | `#727884` | `#8A909A` | Bordes de inputs y controles |
| `text.primary` | `#E8EAED` | `#1B1D21` | Texto |
| `text.secondary` | `#A4A9B3` | `#545A64` | Etiquetas y ayudas |
| `accent` | `#FF7A3D` | `#C2410C` | Herramienta activa, selección, botón primario |
| `on.accent` | `#1A0E07` | `#FFFFFF` | Texto sobre el acento |
| `focus.ring` | `#6CB4FF` | `#1D6FD1` | Anillo de foco de teclado (2 px) |
| `danger` | `#FF6B6B` | `#C62828` | Errores y acciones destructivas |

**Contraste medido (WCAG 2.2) sobre `bg.panel`:**

| Par | Oscuro | Claro | Mínimo exigido |
|---|---|---|---|
| `text.primary` | 13.5:1 | 16.9:1 | 4.5:1 (1.4.3) ✔ |
| `text.secondary` | 6.9:1 | 6.9:1 | 4.5:1 ✔ |
| `accent` como texto o icono | 6.3:1 | 5.2:1 | 4.5:1 ✔ |
| `on.accent` / `accent` | 7.3:1 | 5.2:1 | 4.5:1 ✔ |
| `focus.ring` | 7.5:1 | 4.9:1 | 3:1 (1.4.11) ✔ |
| `border.control` (sobre `bg.raised`) | 3.2:1 | 3.05:1 | 3:1 ✔ |

Se ofrece además un tema de **alto contraste** (texto `#FFFFFF`/`#000000`, bordes de 2 px). El modo por defecto es "Seguir al sistema" (`dark-light` / `ctx.system_theme()`) y el usuario puede fijar claro u oscuro.

### 4.2 Tipografía

- Se incluye **Inter** (OFL) en el binario para que el texto se vea igual en los tres SO; las fuentes del sistema varían entre ellos. Los valores numéricos usan **JetBrains Mono** (OFL) con dígitos tabulares.
- Escala base (con escala de UI = 100 %): `caption 11` · `body 13` · `label 13 semibold` · `title 15` · `heading 18`.
- **Escala de UI de 80–200 %**, independiente del DPI del monitor (`ctx.set_zoom_factor`). Se cambia con Ctrl/⌘ + `+`/`-`/`0` en los paneles; en el lienzo esas teclas hacen zoom de imagen.

### 4.3 Espaciado, forma y elevación

- Retícula de 4 px: `2 · 4 · 8 · 12 · 16 · 24 · 32`.
- Radios: `4` (controles), `6` (tarjetas y popovers), `8` (ventanas modales).
- Elevación con borde sutil y una sola sombra suave solo en elementos flotantes. Los paneles son planos.
- Objetivo de clic mínimo: **24×24 px** (WCAG 2.5.8); los botones de herramienta son de 32×32.

### 4.4 Iconografía

- Iconos de **Lucide** (licencia ISC) cargados como **subconjunto de su fuente tipográfica** (`crates/pyro-ui/assets/icons/lucide-subset.ttf`, 6 KB), no como SVG. Así egui los dibuja con su motor de texto, nítidos a cualquier escala, sin depender de `resvg` (MPL-2.0). Los iconos propios que falten se añadirán a una fuente propia con la misma retícula de 24 px y trazo de 2 px.
- Se colorean con tokens (`text.secondary` en reposo, `accent` cuando están activos), así que funcionan en ambos temas.
- Todo icono lleva tooltip con nombre y atajo, y nombre accesible para AccessKit.

### 4.5 Estados de componente

`default → hover (+4 % luminancia) → pressed (−4 %) → selected (accent) → focused (anillo de 2 px, offset de 2 px) → disabled (opacidad 40 %, sin hover)`. El foco **siempre** es visible con teclado (WCAG 2.4.7 / 2.4.11) y nunca se indica solo con color: el elemento seleccionado también lleva un borde o un fondo distinto.

### 4.6 Movimiento

- Duraciones de 120 ms (hover/press) y 180 ms (paneles, toasts) con *ease-out*, usando `ctx.animate_bool_with_time`.
- Nada se anima en el lienzo durante un trazo.
- La opción **"Reducir movimiento"** (por defecto sigue al SO) pone todas las duraciones a 0.

### 4.7 Diseño adaptable

- Las ventanas modales se ajustan al 90 % del área disponible y tienen scroll interno.
- **< 1100 px de ancho:** el panel derecho (capas/opciones) se colapsa en pestañas.
- **< 900 px:** la barra de herramientas pasa a una columna compacta y las opciones de herramienta a una barra superior.
- El tamaño mínimo de ventana sigue siendo 900×560 (lógicos). Se guarda la geometría por monitor.

---

## 5. Requisitos funcionales y no funcionales

### 5.1 Funcionales

✔ = existe hoy · ◐ = parcial · ○ = nuevo

| ID | Requisito | Estado |
|---|---|---|
| RF-01 | Abrir PNG, JPEG, GIF, WebP, BMP, TIFF | ✔ |
| RF-02 | Abrir PNG de Fireworks como imagen aplanada, informando de que hay bloques propietarios | ✔ |
| RF-03 | Capas: crear, duplicar, borrar, reordenar, visibilidad, opacidad, combinar hacia abajo | ✔ |
| RF-04 | Selección: marco, elipse, lazo, varita mágica; sumar, restar e intersecar; invertir | ✔ |
| RF-05 | Pincel, borrador, tampón de clonar, desenfocar, bote, cuentagotas | ✔ |
| RF-06 | Ajustes y filtros con vista previa: brillo/contraste, tono/saturación, niveles, desenfoque, enfoque, invertir, escala de grises | ◐ (la vista previa bloquea la UI) |
| RF-07 | Exportar optimizado: PNG-8/24/32, JPEG, GIF, WebP, con tamaño estimado | ✔ |
| RF-08 | **Formato nativo con capas** (OpenRaster `.ora`, estándar abierto compatible con GIMP y Krita) | ○ |
| RF-09 | Autoguardado y recuperación tras un cierre inesperado | ○ |
| RF-10 | Deshacer/rehacer por teselas y panel de historial | ◐ |
| RF-11 | Portapapeles: copiar y pegar la selección o la imagen como píxeles con otras apps; pegar como nueva capa o nuevo documento | ○ |
| RF-12 | Arrastrar y soltar archivos al lienzo (abrir) o al panel de capas (añadir como capa); arrastrar hacia fuera para exportar | ◐ (solo abrir) |
| RF-13 | Atajos de teclado configurables, con detección de conflictos e importación/exportación | ○ |
| RF-14 | Paleta de comandos (Ctrl/⌘+K) con búsqueda de cualquier acción | ○ |
| RF-15 | Archivos recientes (persistentes, se pueden limpiar) y "Mostrar en carpeta" | ○ |
| RF-16 | Preferencias: tema, escala de UI, idioma, reducir movimiento, memoria del historial, autoguardado, actualizaciones | ○ |
| RF-17 | Notificación nativa al terminar una exportación o un lote largo **solo si la ventana no tiene foco** | ○ |
| RF-18 | Comprobación de actualizaciones (manual o automática, desactivable) e instalación firmada | ○ |
| RF-19 | Asociación de archivos (`.ora`, opcionalmente `.png`) y abrir desde el gestor de archivos o "Abrir con" | ◐ (solo argumento de línea de comandos en Windows) |
| RF-20 | Varios documentos en pestañas | ○ |
| RF-21 | Idiomas: español (por defecto) e inglés, con `fluent` | ○ |
| RF-22 | Exportación por lotes (carpeta → formato) | ○ (futuro) |

**Bandeja del sistema:** no se implementa. Un editor de imágenes no tiene trabajo residente que mostrar, y añadir un icono de bandeja iría contra las convenciones de los tres SO. Se reconsidera solo si aparece la exportación por lotes en segundo plano (RF-22).

### 5.2 No funcionales

| ID | Categoría | Requisito medible |
|---|---|---|
| RNF-P1 | Arranque | Primer frame en < 500 ms en frío con un SSD (máquina de referencia: 4 núcleos, 2020) |
| RNF-P2 | Memoria | < 120 MB en reposo con un documento de 1920×1080; historial ≤ presupuesto configurable (768 MB por defecto) |
| RNF-P3 | Fluidez | La UI a ≥ 60 fps durante un trazo en un lienzo de 4096×4096; latencia del pincel < 16 ms |
| RNF-P4 | No bloqueo | Ninguna operación bloquea el hilo de UI más de 50 ms; todo lo más largo muestra progreso y se puede cancelar |
| RNF-P5 | Reposo | 0 % de CPU sin interacción (repintado solo por eventos) |
| RNF-E1 | Estabilidad | Ningún `panic` alcanzable con archivos malformados (verificado con fuzzing); tras un fallo se pierde como máximo el trabajo de los últimos 2 min |
| RNF-S1 | Entradas | Límites de decodificación (`image::Limits`: ≤ 32 768 px por lado, ≤ 2 GB de memoria) contra bombas de descompresión; el lector de bloques de Fireworks comprueba longitudes y CRC |
| RNF-S2 | Red | Solo HTTPS (rustls) con verificación de certificados; nunca se baja a HTTP; *timeouts* de 10 s |
| RNF-S3 | Actualizaciones | Paquetes firmados con ed25519 (minisign); se rechazan si la firma no coincide con la clave pública incluida en el binario |
| RNF-S4 | Secretos | Las credenciales futuras (nube) se guardan en el llavero del SO, nunca en TOML ni en logs; ninguna clave en el repositorio (se revisa con `gitleaks` en CI); las claves de firma solo existen como secretos de CI |
| RNF-S5 | Dependencias | `cargo deny` (advisories, licencias, orígenes) y `cargo audit` bloquean el merge; `#![forbid(unsafe_code)]` en `pyro-core` y `pyro-formats` |
| RNF-S6 | Privacidad | Sin telemetría. La comprobación de actualizaciones solo envía versión y SO, y se puede desactivar |
| RNF-A1 | Teclado | Toda acción es accesible con teclado: Tab/Shift+Tab, F6 para cambiar de panel, flechas en listas, Esc para cerrar |
| RNF-A2 | Lectores de pantalla | Todos los widgets tienen rol y nombre en AccessKit (Narrador/NVDA, VoiceOver, Orca); los cambios de herramienta, zoom y selección se anuncian como región *live* |
| RNF-A3 | Lienzo | Las operaciones del lienzo tienen una alternativa numérica: selección por coordenadas, mover con flechas (1 px, 10 px con Shift) y diálogo de transformación (WCAG 2.5.7) |
| RNF-A4 | Visual | Contrastes de §4.1, escala de UI de 80–200 %, foco visible, información nunca solo por color |
| RNF-M1 | Mantenibilidad | `clippy -D warnings`; ningún archivo de más de ~600 líneas; cobertura ≥ 80 % en `pyro-core` y `pyro-formats` |
| RNF-X1 | Consistencia | Las capturas de UI de referencia coinciden entre los 3 SO (tolerancia de diferencia perceptual ≤ 1 %) |

---

## 6. Estrategia de compatibilidad multiplataforma

### 6.1 Plataformas soportadas

| SO | Mínimo | Arquitecturas | Backend gráfico |
|---|---|---|---|
| Windows | 10 22H2, 11 | x86_64, aarch64 | DX12 (Vulkan/GL como respaldo) |
| macOS | 12 Monterey | Universal (arm64 + x86_64) | Metal |
| Linux | Ubuntu 22.04, Fedora 39, Debian 12, Arch (glibc ≥ 2.35) | x86_64 (aarch64 opcional) | Vulkan (GL como respaldo); Wayland y X11 |

### 6.2 Particularidades por SO

| Aspecto | Windows | macOS | Linux |
|---|---|---|---|
| Tecla de comando | Ctrl | ⌘ (`Modifiers::command` lo resuelve solo) | Ctrl |
| Menú | Barra egui en la ventana | **Barra global nativa** (`muda`) con menús *App* (Acerca de, Preferencias ⌘,, Salir ⌘Q), *Ventana* y *Ayuda* | Barra egui en la ventana |
| Preferencias | Editar → Preferencias | Pyrotecnika → Ajustes… (⌘,) | Editar → Preferencias |
| Orden de botones en diálogos | Aceptar · Cancelar | Cancelar · Aceptar | Cancelar · Aceptar (GNOME) |
| Decoración de ventana | Nativa | Nativa (semáforo) | Nativa (CSD en Wayland vía winit) |
| Rutas de datos | `%APPDATA%\Pyrotecnika` | `~/Library/Application Support/Pyrotecnika` | `$XDG_CONFIG_HOME` / `$XDG_DATA_HOME/pyrotecnika` |
| Notificaciones | Toast de WinRT (necesita AUMID, que pone el instalador) | `UNUserNotificationCenter` | D-Bus `org.freedesktop.Notifications` |
| Llavero | Credential Manager | Keychain | Secret Service (GNOME Keyring/KWallet) |
| Abrir archivos desde el SO | Argumento de línea de comandos + instancia única (named pipe) | Evento Apple `openFiles` (requiere un *hook* en `NSApplicationDelegate`; **riesgo técnico**, ver §9) | Argumento + `.desktop` con `%F` |
| HiDPI | Per-Monitor v2 (manifiesto) | Retina automático | Escala de Wayland o `Xft.dpi`, incluida la fraccional |

**Reglas:**

1. Todo código con `#[cfg(target_os)]` vive en `pyro-platform/os/*`. Fuera de ese módulo no se permite ningún `cfg` de SO (se comprueba con un `grep` en CI).
2. La UI pregunta **capacidades**, no SO: `platform.has_native_menu()`, `platform.dialog_button_order()`.
3. Nada del producto depende de una función que no exista en los tres SO. Si un SO no ofrece algo, hay un comportamiento equivalente (por ejemplo, el toast interno cuando no hay servidor de notificaciones).

### 6.3 HiDPI

- Todas las medidas de la UI son lógicas; el lienzo convierte con `pixels_per_point`. Así se corrige de raíz el problema de escalado detectado en Windows.
- Con zoom del 100 %, un píxel de imagen equivale a **un píxel físico**, con muestreo `Nearest` a partir de 100 % y *mipmaps* por debajo.
- Al mover la ventana a un monitor con otra escala, las texturas se regeneran sin reiniciar.

---

## 7. Plan de pruebas

| Nivel | Herramienta | Qué cubre | Cuándo |
|---|---|---|---|
| Unitarias | `cargo test` | Ya existen 30+ (compositing, selección, pincel, filtros, export, Fireworks). Se amplían a teselas e historial | Cada push |
| Propiedades | `proptest` | Invariantes: `combine` es asociativo, `invert(invert(m)) == m`, deshacer×N + rehacer×N = identidad, recortar no sale de los límites | Cada push |
| Golden images | `tests/golden` + comparación con tolerancia | Salida de cada filtro y exportación contra referencias; detecta regresiones de color | Cada push |
| Integración | `tests/` por crate | Roundtrip `.ora`, apertura de todos los formatos, PNG de Fireworks **sintéticos** (los archivos de Adobe no se redistribuyen), migración de settings, DB de recientes | Cada push |
| Fuzzing | `cargo-fuzz` | `open_bytes`, `inspect_png`, lector `.ora` | Nocturno, 30 min por objetivo |
| UI funcional | `egui_kittest` (AccessKit) | Flujos: nuevo → pintar → deshacer; seleccionar → filtro → cancelar; exportar; reasignar un atajo. Se navega **por roles accesibles**, lo que a la vez verifica RNF-A2 | Cada push, en los 3 SO |
| Visual entre plataformas | Snapshots de `egui_kittest` (render wgpu) | Capturas de referencia de cada panel en tema claro, oscuro y alto contraste, al 100 % y 200 %; comparación con tolerancia ≤ 1 % | Cada push, en los 3 SO |
| Rendimiento | `criterion` + script de arranque | Desenfoque 4K, composición de 10 capas 4K, coste de un trazo, tiempo hasta el primer frame, RSS en reposo. Falla si hay una regresión > 10 % respecto a `main` | Cada PR (máquina fija) |
| Accesibilidad manual | Lista de comprobación WCAG 2.2 | NVDA + Narrador, VoiceOver, Orca; navegación solo con teclado; 200 % de escala | Antes de cada release menor |
| Instalación | VMs limpias | Instalar, abrir un archivo desde el gestor, actualizar desde la versión N-1, desinstalar sin dejar restos (salvo datos del usuario si así se elige) | Antes de cada release |

**CI (GitHub Actions)**, matriz `windows-latest`, `macos-14`, `ubuntu-22.04`:
`fmt --check` → `clippy -D warnings` → `test --workspace` → kittest y snapshots → `cargo deny` / `cargo audit` / `gitleaks` → `build --release` → subir artefactos.
En Linux se usa Mesa `lavapipe` (Vulkan por software) para que los snapshots sean reproducibles sin GPU.

---

## 8. Estrategia de compilación y distribución

### 8.1 Versionado

- **SemVer**, con una sola fuente: `[workspace.package] version` en `Cargo.toml`.
- Etiqueta `vX.Y.Z` → `release.yml`. Mensajes de commit *Conventional Commits* → `CHANGELOG.md` generado con `git-cliff`.
- Canales `stable` y `beta` (versiones `X.Y.Z-beta.N`), elegibles en Preferencias.

### 8.2 Paquetes

| SO | Formatos | Herramienta | Firma |
|---|---|---|---|
| Windows | **EXE** (NSIS, por usuario, sin admin) y **MSI** (WiX, por máquina, para IT y GPO) | `cargo-packager` | Authenticode con Azure Trusted Signing (o certificado OV/EV) + sello de tiempo |
| macOS | **DMG** (arrastrar a Aplicaciones) y **PKG** (`productbuild`, para MDM) con un `.app` universal | `cargo-packager` + `lipo` + `productbuild` | Developer ID Application/Installer, *hardened runtime*, **notarización** con `notarytool` y *staple* |
| Linux | **AppImage**, **DEB**, **RPM** | `cargo-packager` (AppImage, deb), `cargo-generate-rpm` | Firma GPG de deb/rpm y del repositorio; AppImage firmada y con `SHA256SUMS.asc` |

> **Por ahora sin firma de código** (decisión 9.2-1): la columna "Firma" describe el objetivo. Los pasos ya están en el pipeline, pero no se ejecutan hasta que se configuren los certificados.

Todos los paquetes incluyen icono en todas las resoluciones, asociación de `.ora`, el archivo `.desktop` + AppStream `metainfo.xml` (Linux), `Info.plist` con `CFBundleDocumentTypes` (macOS) y AUMID (Windows).

La compilación de release usa `lto = "fat"`, `codegen-units = 1`, `panic = "unwind"` (necesario para el guardado de emergencia) y `strip = "symbols"`. Los símbolos de depuración se guardan aparte, por si hace falta analizar un fallo.

### 8.3 Actualizaciones

- Feed JSON estático en HTTPS: `latest.json` en los assets del último release estable de GitHub, con versión, notas, URL y **firma minisign** por plataforma (lo genera `packaging/updater/make_latest_json.py` en `release.yml`).
- Solo se busca cuando el usuario lo pide (*Ayuda › Buscar actualizaciones…*); la app nunca se conecta por su cuenta.
- `pyro-platform/src/updates.rs` descarga (ureq + rustls con los certificados del SO), **verifica la firma con la clave pública incluida en el binario** (`packaging/updater-pubkey.txt`, `minisign-verify`) y aplica. Usa el formato de `cargo-packager-updater`, pero no ese crate: depende de `option-ext` (MPL-2.0), que la política de licencias excluye.
- La llave privada y su contraseña son los secretos `CARGO_PACKAGER_SIGN_PRIVATE_KEY(_PASSWORD)` del repositorio; hay una copia en el gestor de contraseñas del mantenedor. Si se perdiera, las copias instaladas no podrían actualizarse solas.

| Formato instalado | Comportamiento |
|---|---|
| EXE (NSIS), DMG/.app, AppImage | Se actualiza solo, al reiniciar, con confirmación del usuario |
| MSI, PKG | Solo avisa; los gestiona IT (MSI admite despliegue silencioso con `msiexec /i … /qn`) |
| DEB, RPM | Solo avisa; la actualización llega por `apt`/`dnf` desde el repositorio firmado |

- Antes de actualizar se comprueba que no haya documentos sin guardar.

### 8.4 Desinstalación

| SO | Procedimiento | Datos del usuario |
|---|---|---|
| Windows EXE | "Aplicaciones instaladas" o `Uninstall.exe` | El desinstalador pregunta si borrar `%APPDATA%\Pyrotecnika` |
| Windows MSI | "Aplicaciones instaladas" o `msiexec /x {ProductCode}` | Se conservan (política estándar de MSI) |
| macOS | Arrastrar `Pyrotecnika.app` a la Papelera | Se documenta cómo borrar `~/Library/{Application Support,Caches,Logs}/Pyrotecnika`; menú *Ayuda → Restablecer y borrar datos* |
| DEB / RPM | `sudo apt remove pyrotecnika` / `sudo dnf remove pyrotecnika` | `~/.config` y `~/.local/share/pyrotecnika` se conservan; *Ayuda → Restablecer* los borra |
| AppImage | Borrar el archivo | Igual que arriba |

`docs/INSTALACION.md` documenta todo esto, incluidos los requisitos (Vulkan/GL en Linux) y la verificación de firmas.

---

## 9. Hoja de ruta y decisiones pendientes

### 9.1 Fases (cada una deja la app funcionando)

| Fase | Contenido | Criterio de salida |
|---|---|---|
| **0. Cimientos** ✔ | Workspace en crates; `app.rs` partido en módulos; backend wgpu; CI en los 3 SO; licencias; `cargo-deny`; prueba de concepto de abrir archivos en macOS | Verificado en los tres SO en el CI de GitHub (jobs `test` y `macos-open-files`) |
| **1. Sistema de diseño** | Tokens, temas claro/oscuro/alto contraste, Inter, iconos, estados, escala de UI | Snapshots de referencia aprobados |
| **2. Comandos y preferencias** | Registro de comandos, atajos configurables, paleta, settings TOML, recientes en SQLite | RF-13 a RF-16 |
| **3. No bloqueo y memoria** | Jobs con progreso y cancelación, historial por teselas, límites de decodificación, fuzzing | RNF-P3, P4 y E1 medidos |
| **4. Formato nativo y recuperación** | `.ora`, autoguardado, panic hook, pestañas | RF-08 a RF-10, RF-20 |
| **5. Integración de escritorio** | Portapapeles de imagen, arrastrar y soltar completo, notificaciones, menú macOS, asociación de archivos, instancia única | RF-11, 12, 17, 19 |
| **6. Distribución** ◐ | Paquetes de los 6 formatos ✔ (más NSIS), feed y updater ✔, firma de código y notarización (pendientes, decisión 9.2-1) | Instalar → actualizar → desinstalar en VMs limpias |
| **7. Accesibilidad y pulido** | Auditoría WCAG 2.2 con lectores de pantalla reales, i18n en/es | Lista de comprobación de §7 al 100 % |

### 9.2 Decisiones tomadas (2026-11-11)

1. **Firma de código: ninguna por ahora.** Los paquetes salen sin firmar. Gatekeeper (macOS) y SmartScreen (Windows) mostrarán avisos, y `docs/INSTALACION.md` explicará cómo autorizar la app. Los pasos de firma y notarización de `release.yml` quedan preparados, pero se omiten mientras no existan los secretos (`APPLE_*`, `AZURE_SIGNING_*`). Activarlos más adelante no requiere tocar el código. La firma **minisign** de las actualizaciones (RNF-S3) sí se aplica desde el principio, porque no cuesta nada.
2. **Alojamiento: GitHub.** El CI se hace con GitHub Actions (matriz Windows/macOS/Linux) y el feed de actualizaciones son los assets de GitHub Releases.
3. **Licencia: MIT OR Apache-2.0** (la convención de Rust). `license = "MIT OR Apache-2.0"` en el workspace, con `LICENSE-MIT` y `LICENSE-APACHE`. `cargo-deny` solo admite dependencias con licencias compatibles (MIT, Apache-2.0, BSD, ISC, Zlib, Unicode, OFL para las fuentes) y rechaza GPL/AGPL.
4. **Abrir archivos en macOS: prueba de concepto en la fase 0.** Implementada en `crates/pyro-platform/src/os/macos.rs`. En vez de tocar el delegate de winit, se registra un manejador del evento Apple `kAEOpenDocuments` en `NSAppleEventManager` al recibir `NSApplicationWillFinishLaunchingNotification`, que es el único momento en que no lo sobrescribe `NSApplication` y aún llegan los archivos del arranque. Compila para `aarch64-apple-darwin`. La validación en ejecución la hace `packaging/macos/poc-open-files.sh` en el job `macos-open-files` del CI: abre un archivo en frío y otro en caliente. Si falla, RF-19 queda en macOS limitado a Archivo → Abrir y arrastrar a la ventana, y se documenta.
