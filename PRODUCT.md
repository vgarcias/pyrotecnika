# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

> Nota: el esquema solo admite `web`, `ios`, `android` o `adaptive`. Pyrotecnika es en realidad una **app nativa de escritorio** (Windows, macOS, Linux) hecha con Rust + egui, que dibuja su propia interfaz y se ve igual en los tres sistemas. No hay HTML, CSS ni navegador: las herramientas web de Impeccable (`live`, `generate`, detector) no aplican.

## Users

El usuario principal es el propio autor (confirmado el 2026-11-14). Es una herramienta personal: se diseña para su trabajo, no para un público externo. Si a otros les sirve, bien, pero sus necesidades no dirigen las decisiones.

Trabajos que hace con ella, todos frecuentes (confirmado):

- Preparar imágenes para web: recortar, redimensionar, ajustar y exportar optimizado.
- Retocar fotos: color, niveles, clonar, limpiar.
- Componer con capas y selecciones: gráficos, banners, maquetas.
- Abrir archivos PNG de Fireworks antiguos para reutilizarlos.

## Product Purpose

Editor de imágenes de escritorio que recupera, como app propia, la parte de edición bitmap de Adobe Fireworks: capas y selección, pintura y retoque, ajustes y filtros, exportación optimizada y apertura de PNG de Fireworks.

Éxito significa que el autor puede hacer sus cuatro trabajos de principio a fin en un solo programa rápido, en cualquiera de sus tres sistemas operativos.

## Positioning

- Solo la parte de edición de imágenes de Fireworks, sin el resto del producto. No es un clon de su aspecto ni una copia: la identidad visual es propia.
- Binario nativo ligero que arranca en menos de medio segundo, sin navegador embebido.
- Idéntico en Windows, macOS y Linux porque dibuja cada píxel de su interfaz.
- Abre PNG de Fireworks (como imagen aplanada) y exporta con control fino de formato y tamaño.

## Operating Context

- Escritorio con ratón y teclado; ventana mínima de 900×560 lógicos; pantallas con escalado HiDPI.
- Sesiones de trabajo sobre archivos locales: abrir, editar, exportar para web.
- El lienzo es el centro de la pantalla: el entorno que lo rodea debe ser neutro para no alterar la percepción del color.
- Formatos de entrada: PNG, JPEG, GIF, WebP, BMP, TIFF y `.fw.png`. Salida: PNG-8/24/32, JPEG, GIF, WebP con tamaño estimado.
- Idioma de la interfaz: español por defecto; inglés previsto.

## Capabilities and Constraints

Fuente de verdad: `docs/ARQUITECTURA.md` (requisitos en §5, hoja de ruta de 8 fases en §9).

- **Existe hoy:** capas, selección (marco, elipse, lazo, varita), pincel, borrador, tampón, desenfocar, bote, cuentagotas, ajustes y filtros con vista previa, exportación optimizada, apertura de PNG de Fireworks.
- **Previsto:** formato nativo con capas (`.ora`), autoguardado y recuperación, historial por teselas, portapapeles de imagen, atajos configurables, paleta de comandos, recientes, preferencias, pestañas, i18n.
- **Stack fijo:** Rust + egui/eframe con backend wgpu. La UI es de modo inmediato; no hay DOM.
- **Licencias:** MIT OR Apache-2.0; ninguna dependencia copyleft (GPL, LGPL, AGPL, MPL), verificado con `cargo deny`. Por eso los iconos son un subconjunto de la fuente Lucide y no SVG.
- **Rendimiento como requisito:** primer frame < 500 ms, 60 fps durante un trazo en 4096×4096, ninguna operación bloquea la UI más de 50 ms, 0 % de CPU en reposo.
- **Sin telemetría** y sin bandeja del sistema.
- **PNG de Fireworks:** las capas propietarias no se leen; se abre la imagen aplanada y se informa.
- **Estado de la interfaz:** hoy usa el tema por defecto de egui. El sistema de diseño (fase 1) está especificado en `docs/ARQUITECTURA.md` §4 pero no implementado.

## Brand Commitments

- Nombre: **Pyrotecnika**. Motivo de identidad declarado en la arquitectura: brasas y chispas.
- No imitar el aspecto de Fireworks ni de ningún otro producto existente.
- Iconografía Lucide (ISC); tipografías previstas Inter y JetBrains Mono (OFL), incluidas en el binario.
- Los tokens de color y sus contrastes de `docs/ARQUITECTURA.md` §4.1 son la propuesta vigente de la fase 1.

## Evidence on Hand

- `docs/ARQUITECTURA.md`: arquitectura, sistema de diseño propuesto, requisitos y hoja de ruta.
- Código de la interfaz en `crates/pyro-ui/src/app/`; iconos en `crates/pyro-ui/assets/icons/`.
- Muestras reales de PNG de Fireworks en la instalación local de Fireworks CS6 (no redistribuibles).
- No hay logotipo, capturas de referencia, usuarios externos, testimonios ni métricas de uso. No inventarlos.

## Product Principles

1. **El lienzo manda.** La interfaz cede protagonismo a la imagen y no altera cómo se percibe su color.
2. **Nunca hacer esperar.** La respuesta inmediata pesa más que cualquier adorno.
3. **Un mismo producto en los tres sistemas**, respetando solo las convenciones que el usuario notaría (menú de macOS, orden de botones, tecla de comando).
4. **Herramienta de autor.** Se optimiza para los flujos reales de una persona experta, no para el primer uso de un desconocido.
5. **Identidad propia.** Inspirada en lo que Fireworks hacía bien, nunca en cómo se veía.

## Accessibility & Inclusion

Objetivo WCAG 2.2 (detalle en `docs/ARQUITECTURA.md` §5.2, RNF-A1 a A4): todo operable con teclado, roles y nombres en AccessKit para lectores de pantalla, alternativa numérica a las operaciones del lienzo, foco siempre visible, escala de UI de 80–200 %, tema de alto contraste, opción de reducir movimiento e información nunca solo por color.
