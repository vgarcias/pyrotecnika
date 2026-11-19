# Fase 3: no bloqueo y memoria

Estado: borrador para aprobación del autor (2026-11-19).
Criterio de salida (ARQUITECTURA §9.1): **RNF-P3, RNF-P4 y RNF-E1 medidos**.

## 1. Intención

Que Pyrotecnika se sienta fluida con cualquier imagen, la que sea (el autor
trabaja con tamaños y número de capas muy variados): la ventana nunca se
congela, el pincel sigue al ratón en un lienzo grande, deshacer no se queda
sin pasos por memoria, y ningún archivo —por dañado o malicioso que sea—
cierra la app.

## 2. Objetivos

1. **RNF-P4 · No bloqueo.** Ninguna operación ocupa el hilo de la interfaz
   más de 50 ms. Lo que tarde más corre en segundo plano, muestra progreso y
   se puede cancelar.
2. **RNF-P3 · Fluidez.** Pintar en un lienzo de 4096×4096 con varias capas
   mantiene ≥ 60 fps: el trabajo de CPU por fotograma durante un trazo
   (pincelada + composición + preparar la textura) es < 16 ms.
3. **Historial por diferencias.** Un paso de deshacer guarda solo los
   bloques de 256×256 que cambiaron, no el documento entero. Presupuesto
   de 768 MB (RNF-P2) y tope de pasos subido de 50 a 200.
4. **RNF-S1 · Límites al abrir.** Imágenes de más de 32 768 px por lado o
   que pidan más de 2 GB se rechazan con un mensaje claro, antes de
   reservar memoria.
5. **RNF-E1 · Sin cierres con archivos malformados**, verificado con
   fuzzing de los decodificadores y del lector de bloques de Fireworks.
6. **Medido.** Cada número de arriba tiene una prueba que lo mide y un
   resultado anotado en ARQUITECTURA.

## 3. No objetivos (fuera de esta fase)

- Capas partidas en teselas en memoria (enfoque A, descartado; ver §5).
- Formato `.ora`, autoguardado, recuperación tras fallo y panel de
  historial (fase 4; la parte «se pierde como máximo 2 min» de RNF-E1 es de
  la fase 4).
- Presupuesto de memoria configurable en Preferencias (queda la constante).
- Medir con lectores de pantalla o en GPU real dentro del CI.

## 4. Antiobjetivos (fallaría aunque funcionara)

- La ventana deja de responder (no se puede mover, cerrar ni cancelar)
  mientras algo trabaja.
- Cancelar deja el documento a medias o distinto de como estaba.
- Una tarea en segundo plano escribe sobre un documento que el usuario ya
  cambió o cerró.
- Deshacer devuelve un estado que nunca existió (bloques mezclados de dos
  pasos).
- Las capturas de referencia de las fases 1–2 cambian sin motivo visible.
- Un archivo enorme o dañado cierra la app en vez de mostrar un aviso.

## 5. Enfoques considerados

- **A. Capas en teselas de 256×256 compartidas (`Arc<Tile>`).** Lo que
  describe ARQUITECTURA §1.3. Lo que mejor escala, pero obliga a reescribir
  pincel, filtros, selección, exportar y lector de Fireworks. Descartado por
  riesgo y tamaño.
- **B. Capas enteras compartidas + historial por diferencias.** *Elegido
  por el autor (2026-11-19).* Las capas siguen siendo una imagen entera,
  pero dentro de un `Arc` con copia al escribir; el historial compara antes
  y después por bloques y guarda solo los que cambiaron; el lienzo se sube a
  la GPU por bloques sucios. Casi el mismo resultado para el usuario con una
  fracción del cambio. Se puede pasar a A más adelante sin tirar nada.
- **C. Solo compartir capas.** Deshacer guarda las capas cambiadas enteras.
  Barato, pero un trazo en 4K sigue costando 64 MB por paso. Descartado.

ARQUITECTURA §1.3 se reescribe para describir B y deja A como evolución
posible.

## 6. Comportamiento

### 6.1 Tareas en segundo plano

- Nuevo crate `pyro-services` (previsto en ARQUITECTURA §1.1) con el módulo
  `jobs`: lanza un trabajo en un hilo (con `rayon` para el paralelismo
  interno), le da un `CancelToken` y devuelve eventos `Progress(f32)` y
  `Done(Result)` por canal, despertando a la interfaz con un callback (la UI
  pasa `ctx.request_repaint`). Sin dependencia de egui.
- Los trabajos reciben una instantánea del documento (capas `Arc`, sin
  copiar píxeles) y devuelven un resultado; la interfaz lo aplica en su
  hilo. El trabajo nunca toca el documento vivo.
- **Pasan a tarea:** abrir (leer + decodificar), aplicar un filtro, la
  vista previa de un filtro y exportar (hoy ya en un hilo, sin progreso ni
  cancelación). Cualquier otra operación que la medición de §6.6 encuentre
  por encima de 50 ms en 4096×4096 se optimiza o pasa a tarea.
- **Mientras corre una tarea que va a cambiar el documento** (abrir,
  aplicar filtro): la barra de estado muestra el nombre, una barra de
  progreso y «Cancelar»; Esc también cancela. El lienzo se sigue viendo,
  con zoom y desplazamiento, pero las herramientas, los atajos de edición y
  los menús que cambian el documento están desactivados. Cerrar la ventana
  pregunta como hoy y cancela la tarea.
- **Exportar** puede correr mientras se sigue editando (lee una
  instantánea); su progreso va en el propio diálogo de exportación, con
  «Cancelar».
- **Vista previa de filtros:** a resolución completa, para que lo que se
  ve sea exactamente lo que se aplica (ARQUITECTURA §1.2 hablaba de una
  vista reducida; se corrige). Al mover un control se cancela la vista
  previa en curso y se lanza otra; el lienzo muestra la última terminada
  en lugar de la capa activa. «Aceptar» aplica esa vista previa si sus
  parámetros coinciden con los actuales, o espera a la que está en curso;
  «Cancelar» o Esc vuelven exactamente al estado anterior.
- **Abrir:** si se cancela, sigue el documento anterior sin cambios. Si
  falla, aviso en la barra de estado (como hoy).
- Cancelar responde en menos de 100 ms: los trabajos consultan el token
  entre bandas de filas.
- Solo una tarea que cambie el documento a la vez; exportar puede ir en
  paralelo.
- **Resultados caducados:** el documento lleva un número de generación
  que cambia al abrir, crear o cerrar. Una tarea recuerda la generación de
  su instantánea y su resultado se descarta si ya no coincide.
- **Cerrar la ventana con una exportación en curso** la cancela sin
  preguntar: aún no se ha escrito nada en disco (el archivo se guarda
  después, desde el diálogo).
- **Prioridad de Esc:** primero el diálogo o ventana abierta (paleta,
  preferencias, confirmación, filtro, exportar), después la tarea en curso
  y, por último, lo que Esc hace hoy en el lienzo.

### 6.2 Capas compartidas

- `Layer::pixels` pasa a `Arc<RgbaImage>`; editar usa copia al escribir
  (`Arc::make_mut`). Clonar el documento (instantáneas, historial) no copia
  píxeles. La primera edición tras una instantánea copia esa capa una vez
  (~20 ms en 4K, aceptable).
- La API pública de `pyro-core` se mantiene en lo posible para que pincel,
  filtros y exportar cambien poco.

### 6.3 Historial por diferencias

- Cada paso guarda el estado anterior con las capas compartidas. Cuando el
  paso se cierra (siguiente cambio, deshacer, o al soltar el pincel), las
  capas cuyo `Arc` difiere del documento actual se comparan por bloques de
  256×256 y se guardan solo los bloques distintos; las que no cambiaron no
  cuestan nada.
- Los cambios de estructura (añadir, borrar, reordenar capas, recortar,
  nombre, opacidad, visibilidad) y la selección siguen funcionando igual
  que hoy en deshacer/rehacer.
- El coste de memoria de un paso es proporcional al área cambiada. El
  presupuesto de 768 MB cuenta los bloques guardados; al pasarlo se
  descartan los pasos más viejos. Tope de 200 pasos.
- Comparar al cerrar el paso cabe en < 50 ms en 4096×4096 por capa
  cambiada (medido en §6.6); si no, se compara solo el rectángulo que la
  operación declara como tocado.
- «Modificado» (el punto del título y la pregunta al cerrar) se comporta
  igual que hoy.

### 6.4 Lienzo por bloques

- La textura del lienzo se parte en bloques (tamaño a elegir por el
  constructor; 512 o 1024). Un cambio marca como sucio solo el rectángulo
  afectado; se recompone y se sube solo ese rectángulo de los bloques que
  toca.
- Esto también evita pedir a la GPU una textura mayor que su máximo, que
  hoy haría caer la app con imágenes grandes: ningún bloque supera
  1024×1024.
- Lo visible no cambia: las capturas de referencia deben seguir iguales.
- El cuentagotas lee el píxel compuesto sin componer todo el documento.

### 6.5 Límites al abrir

- La lectura usa `image::ImageReader` con `Limits`: ≤ 32 768 px por lado
  y ≤ 2 GB de reserva del decodificador (`max_alloc`). Además, la imagen
  resultante (ancho × alto × 4 bytes) no puede pasar de 2 GB: 32 768 ×
  16 384 cabe, 32 768 × 32 768 no. Todo se comprueba con las dimensiones de
  la cabecera, antes de decodificar.
- Mensaje: «La imagen es demasiado grande (A×B px). Pyrotecnika abre
  imágenes de hasta 32 768 px por lado y 2 GB por capa.»
- «Nuevo» respeta los mismos dos límites. No hay tope del total del
  documento (varias capas) en esta fase.
- El lector de bloques de Fireworks (`inspect_png`) comprueba longitudes
  (ya lo hace) y además el CRC de los bloques privados de Fireworks. Si un
  bloque privado tiene el CRC mal, la imagen se abre igual que hoy (la
  vista aplanada) y la barra de estado avisa de que los datos de Fireworks
  están dañados; ningún archivo que abre hoy deja de abrir. La protección
  contra sobrescribir el original de Fireworks sigue activa en ese caso.

### 6.6 Medición

- Pruebas de rendimiento marcadas `#[ignore]`, en `release`, sobre
  4096×4096 con 3 capas:
  - trazo de 200 puntos: trabajo por fotograma (pincelada + componer el
    rectángulo sucio + preparar los bloques) < 16 ms en p95;
  - cerrar un paso de historial tras un trazo: < 50 ms;
  - cada operación que queda en el hilo de la interfaz (varita, bote,
    recortar, combinar, duplicar, invertir selección, cuentagotas,
    deshacer/rehacer): < 50 ms;
  - memoria del historial tras 100 trazos pequeños: muy por debajo de 768 MB
    (el número exacto se anota).
- Se corren con `cargo test --release -- --ignored medir`. Siempre
  imprimen los tiempos; solo fallan por tiempo con `PYRO_MEDIR_ESTRICTO=1`
  (en la máquina del autor). El CI las ejecuta sin esa variable, para que
  no se rompan sin exigir tiempos (las máquinas de CI varían).
- Resultados anotados en ARQUITECTURA (tabla de RNF, columna de estado).

### 6.7 Fuzzing

- Carpeta `fuzz/` con `cargo-fuzz` y tres objetivos: `open_bytes`,
  `inspect_png` y el decodificado con límites.
- Nightly y Linux: un job del CI (programado cada noche y lanzable a mano)
  corre cada objetivo 10 minutos; un fallo deja el archivo que lo provoca
  como artefacto.
- Además, en estable y en los tres SO: una prueba normal que abre un
  corpus pequeño de archivos dañados (truncados, dimensiones absurdas,
  CRC roto, bloques de longitud imposible) y comprueba que todos dan error
  y ninguno cierra la app. Cada fallo que encuentre el fuzzing se añade al
  corpus.

## 7. Restricciones

- Reglas del CI vigentes: `pyro-core` y `pyro-formats` sin egui;
  `cfg(target_os)` solo en `pyro-platform`; sin colores literales fuera del
  sistema de diseño; `cargo deny` (sin GPL/LGPL/AGPL/MPL).
- `pyro-services` tampoco depende de egui (misma regla del CI).
- `#![forbid(unsafe_code)]` en `pyro-core` y `pyro-formats`.
- egui/eframe 0.31; textos de la interfaz en español; colores neutros.
- Las capturas de referencia existentes no cambian (salvo la barra de
  progreso nueva, que tiene su propia captura).

## 8. Pruebas

- Unitarias de historial: deshacer/rehacer tras trazos, filtros y cambios
  de estructura devuelve exactamente cada estado anterior (comparando
  píxeles); memoria proporcional al área cambiada.
- Unitarias de tareas: progreso, cancelación (responde y no aplica nada),
  error, y que el resultado de una tarea no se aplica si el documento ya
  no es el que se instantaneó.
- kittest: abrir con progreso y cancelar deja el documento anterior; Esc
  cancela; durante la tarea las herramientas no actúan; vista previa de
  filtro y Cancelar restauran; exportar con progreso.
- Límites: cabeceras de 40 000×10 (lado) y 30 000×30 000 (memoria)
  rechazadas sin reservar memoria; corpus de archivos dañados; PNG de
  Fireworks con CRC privado roto se abre aplanado con aviso.
- Capturas nuevas: barra de estado con una tarea en curso (claro, oscuro,
  alto contraste).
- Rendimiento (§6.6) y fuzzing (§6.7).

## 9. Documentación

- ARQUITECTURA §1.2 (jobs), §1.3 (historial: enfoque B), RNF-P3/P4/E1/S1
  con resultados medidos, §9.1 fase 3 ✔.
- DESIGN.md: componente «tarea en curso» en la barra de estado.

## 10. Decisiones del constructor del spec (revisables)

- Crate nuevo `pyro-services` para `jobs`, como prevé ARQUITECTURA, en vez
  de meterlo en `pyro-core`.
- Durante una tarea que cambia el documento, la edición se bloquea (no se
  encola); la vista sigue viva.
- Exportar se permite en paralelo con la edición.
- Tope de pasos de deshacer 200 (antes 50); presupuesto sigue en 768 MB.
- Umbrales de rendimiento exigidos solo en la máquina del autor, no en CI.
- Fuzzing nocturno de 10 min por objetivo (ARQUITECTURA decía 30; se
  ajusta al tiempo gratuito del CI).
- Vista previa de filtros a resolución completa (no reducida).
- CRC roto en un bloque de Fireworks: abre aplanado con aviso, no falla.
- 2 GB por capa además de 32 768 px por lado.
- Cerrar con una exportación en curso la cancela sin preguntar.
- Número de generación del documento para descartar resultados caducados.
- Esc: diálogo abierto > tarea en curso > lienzo.

## 11. Lo que queda al constructor

- Tamaño exacto de los bloques del lienzo (512 o 1024) y forma de la API
  de `jobs`.
- Cómo se declara el rectángulo tocado por cada operación.
- Qué operaciones concretas pasan a tarea tras medir (§6.6).
- Corpus inicial de archivos dañados.
