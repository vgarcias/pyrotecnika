# Fase 4: documentos, guardado y recuperación

Estado: borrador para aprobación del autor (2026-11-21).
Criterio de salida (ARQUITECTURA §9.1): **RF-08, RF-09, RF-10 y RF-20**.

## 1. Intención

El autor trabaja con **varias imágenes abiertas a la vez** y hoy no puede
guardar su trabajo con capas: Pyrotecnika solo exporta imágenes aplanadas y
todo se pierde al cerrar o si la app falla. Esta fase convierte la app en un
editor de documentos de verdad:

- varias imágenes en **pestañas** dentro de una ventana (RF-20);
- **Guardar** con capas en OpenRaster `.ora`, el formato abierto de GIMP y
  Krita (RF-08);
- **autoguardado** y **recuperación** tras un cierre inesperado, perdiendo
  como mucho 2 minutos de trabajo (RF-09, y la parte pendiente de RNF-E1);
- **panel de historial** para ver y saltar entre los pasos (RF-10).

## 2. Objetivos

1. Pestañas con su propio documento, historial, zoom, archivo y tareas;
   herramientas, pincel, paneles y preferencias comunes.
2. Guardar / Guardar como en `.ora` que GIMP y Krita abren con sus capas, y
   abrir `.ora` (los nuestros y los de GIMP/Krita, en lo que admitimos).
3. Autoguardado cada 2 min de cada pestaña con cambios, en segundo plano.
4. Al arrancar tras un fallo, ofrecer recuperar lo autoguardado.
5. Panic hook: registro del fallo en un archivo de log.
6. Panel de historial con el nombre de cada paso; clic para ir a ese paso.

## 3. No objetivos (fuera de esta fase)

- Ventanas separadas por imagen o vista lado a lado (enfoque B, descartado).
- Arrastrar pestañas para reordenarlas o sacarlas a otra ventana.
- Copiar/pegar capas entre documentos (portapapeles: fase 5, RF-11).
- Asociación de archivos `.ora` en el SO (fase 5, RF-19).
- Escribir los bloques privados de Fireworks (formato propietario).
- Modos de fusión, grupos de capas, capas de texto o vectoriales de `.ora`
  (se leen aplanados o se ignoran con aviso; ver §6.3).

## 4. Antiobjetivos (fallaría aunque funcionara)

- Perder trabajo: cerrar una pestaña o la app con cambios sin preguntar;
  un guardado que deja un `.ora` a medias si falla a mitad.
- Sobrescribir el PNG original de Fireworks al guardar.
- Que el autoguardado congele la interfaz o se note al pintar.
- Que una pestaña actúe sobre el documento de otra (pintar, deshacer,
  aplicar un filtro, que llegue el resultado de una tarea).
- Ofrecer recuperar documentos que se cerraron bien.
- Un `.ora` dañado o malicioso que cierre la app.

## 5. Enfoques considerados

- **A. Pestañas en una sola ventana.** *Elegido por el autor (2026-11-21).*
  Barra de pestañas bajo el menú; cada pestaña es un documento; lo demás es
  común. Es lo que conoce de Fireworks y encaja con el código.
- **B. Una ventana del SO por imagen** (viewports de egui). Permite dos
  monitores, pero funciona peor con lectores de pantalla y atajos, duplica
  el menú en macOS y es más frágil. Descartado.

Orden: primero pestañas, luego `.ora`/Guardar y encima autoguardado y
recuperación, para que estos nazcan ya para varios documentos.

## 6. Comportamiento

### 6.1 Pestañas

- Barra de pestañas entre el menú y el área de trabajo. Cada pestaña:
  nombre del archivo (o «Sin título N»), «•» si tiene cambios sin guardar,
  y botón «×». La activa va invertida (regla de inversión del sistema de
  diseño). Si no caben, la barra se desplaza en horizontal.
- **Estado por pestaña:** documento, historial, zoom y desplazamiento,
  texturas del lienzo, ruta de origen, ruta `.ora` de guardado, info de
  Fireworks, tarea en curso, diálogo de filtro, diálogo de exportar, origen
  del tampón. **Común:** herramienta y pincel, tolerancia y modos de
  selección, tema, escala, atajos, recientes, preferencias, paleta.
- Abrir y Nuevo crean una pestaña nueva y la activan. Abrir un archivo que
  ya está abierto activa su pestaña en vez de duplicarlo.
- Al arrancar sin archivos hay una pestaña «Sin título 1» vacía, como hoy.
  Cerrar la última pestaña deja una «Sin título» nueva (la ventana nunca
  queda sin documento).
- **Atajos nuevos** (en el catálogo de comandos, configurables): Ctrl+Tab /
  Ctrl+Mayús+Tab (pestaña siguiente / anterior), Ctrl+W (cerrar pestaña).
  Menú Ventana con la lista de pestañas abiertas.
- **Cerrar una pestaña con cambios:** la confirmación actual, con
  «Guardar», «Descartar» y «Cancelar» (foco inicial en Cancelar).
- **Cerrar la app** con varias pestañas con cambios: una sola ventana que
  las lista y ofrece «Guardar todo», «Descartar todo» o «Cancelar». Al
  guardar todo, las pestañas sin archivo piden ruta una a una; cancelar
  cualquiera de esos diálogos cancela el cierre.
- Una tarea en segundo plano (abrir, filtro) pertenece a su pestaña: sigue
  corriendo si se cambia de pestaña y su resultado se aplica a esa pestaña.
  La barra de estado muestra la tarea de la pestaña activa. Cerrar una
  pestaña cancela su tarea.
- **Memoria del historial:** el presupuesto de 768 MB es para todas las
  pestañas juntas; al pasarlo se descartan los pasos más viejos de la
  pestaña que más ocupa.

### 6.2 Guardar

- **Guardar (Ctrl+S)** y **Guardar como… (Ctrl+Mayús+S)** en el menú
  Archivo, antes de Exportar.
- Guardar escribe la ruta `.ora` de la pestaña. Si no tiene (documento
  nuevo, o abierto de PNG/JPG/Fireworks/…), actúa como Guardar como.
- Guardar como propone la carpeta del archivo de origen y el mismo nombre
  con extensión `.ora` (`cenote.png` → `cenote.ora`). El original nunca se
  toca: el diálogo solo ofrece `.ora`.
- Escritura atómica: se escribe un temporal en la misma carpeta y se
  renombra; si falla, el archivo anterior queda intacto y se avisa con la
  causa (como al exportar).
- Guardar corre en segundo plano como tarea de la pestaña (con progreso),
  pero **no bloquea la edición**: guarda una instantánea (capas `Arc`).
- Tras guardar bien: la pestaña queda sin «•», el nombre de la pestaña y el
  título pasan a `nombre.ora`, se añade a recientes y se borra su
  autoguardado.
- **Exportar ya no quita la marca de cambios** (antes sí, porque no había
  Guardar). La barra de estado dice «Cambios sin guardar» en vez de «Cambios
  sin exportar».

### 6.3 Formato `.ora`

- Contenedor ZIP según la especificación OpenRaster 0.0.6: `mimetype`
  (primero y sin comprimir, contenido `image/openraster`), `stack.xml`,
  `data/*.png` (una por capa, RGBA 8 bits), `mergedimage.png` (la imagen
  compuesta) y `Thumbnails/thumbnail.png` (≤ 256×256).
- `stack.xml`: un `<stack>` con un `<layer>` por capa, de arriba abajo, con
  `name`, `src`, `x`, `y`, `opacity` y `visibility` (`visible`/`hidden`).
  La capa activa se guarda en un atributo propio
  `pyrotecnika:selected="true"` que los demás programas ignoran.
- **Leer:** capas PNG con `x`/`y` (se colocan en un lienzo del tamaño
  `w`×`h` del stack), opacidad y visibilidad. Grupos (`<stack>` anidados):
  se aplanan sus capas en orden. Modo de fusión distinto de normal,
  capas de texto o filtros: se leen como capa de bitmap normal y se avisa
  una vez («Este archivo usa funciones que Pyrotecnika aún no admite; se
  abrió aproximado»). Si falta `stack.xml` pero hay `mergedimage.png`, se
  abre aplanado con aviso.
- Límites de la fase 3 (32 768 px por lado, 2 GB por capa) para el stack y
  cada capa; además, tamaño total descomprimido ≤ 8 GB y ≤ 1 000 capas.
- Nuevo objetivo de fuzzing `open_ora`.

### 6.4 Autoguardado y recuperación

- Carpeta de recuperación del SO (nueva `pyro_platform::paths::data_dir()`:
  `%LOCALAPPDATA%\Pyrotecnika`, `~/Library/Application Support/io.Pyrotecnika`,
  `$XDG_DATA_HOME/pyrotecnika` o `~/.local/share/pyrotecnika`), subcarpeta
  `recovery/`.
- Cada 2 min, cada pestaña con cambios desde su último autoguardado se
  guarda allí en `.ora` (tarea de baja prioridad, misma escritura atómica)
  junto a un pequeño archivo de datos: ruta de origen y `.ora` si las
  tiene, nombre de la pestaña y fecha.
- Se borra el autoguardado de una pestaña al guardarla, al cerrarla
  (guardando o descartando) y al cerrar la app con normalidad.
- **Al arrancar**, si quedan autoguardados de otra sesión (la app no cerró
  bien), una ventana los lista («cenote.png — hace 3 min») con
  «Recuperar» (abre cada uno en una pestaña marcada con cambios y con su
  nombre y rutas originales), «Descartar» y «Decidir después» (se
  conservan y se vuelven a ofrecer en el siguiente arranque).
- Dos instancias abiertas a la vez no se roban los autoguardados: cada
  sesión escribe en su propia subcarpeta y al arrancar solo se ofrecen las
  de sesiones que ya no están vivas (marca de sesión con el PID y la hora de
  inicio).

### 6.5 Panic hook y log

- Al arrancar se instala un panic hook que escribe la hora, el mensaje, la
  ubicación y la traza en `data_dir()/logs/pyrotecnika.log` (se conservan
  los 7 últimos días) y luego deja que la app termine como hasta ahora.
- No se intenta guardar desde el hook (el estado puede estar roto); la
  recuperación cuenta con el autoguardado de ≤ 2 min.
- El log no guarda rutas completas de archivos del usuario, solo nombres
  (ARQUITECTURA §1.4).

### 6.6 Panel de historial

- Sección «Historial» en el panel derecho, bajo Capas, plegable. Lista los
  pasos de la pestaña activa del más viejo al más nuevo, empezando por
  «Abierto» o «Nuevo documento»; el paso actual va invertido; los pasos que
  se pueden rehacer, en `text_disabled`.
- Cada paso tiene nombre: «Pincel», «Borrador», «Tampón», «Desenfocar»,
  «Mover», «Bote de pintura», «Selección», «Deseleccionar», «Borrar
  selección», «Recortar», «Nueva capa», «Duplicar capa», «Eliminar capa»,
  «Subir capa», «Bajar capa», «Combinar abajo», «Propiedades de capa» y el
  nombre del filtro.
- Clic en un paso: deshace o rehace hasta él. Con teclado: la lista es
  navegable con flechas e Intro, y cada fila tiene nombre accesible.
- Si el presupuesto descartó pasos viejos, la primera fila dice «Pasos
  anteriores descartados por memoria».

## 7. Restricciones

- Reglas del CI vigentes (sin egui en `pyro-core`/`pyro-formats`/
  `pyro-services`; `cfg(target_os)` solo en `pyro-platform`; sin colores
  literales fuera del sistema de diseño; `cargo deny` sin GPL/LGPL/AGPL/MPL;
  clippy del CI más nuevo que el local).
- Nueva dependencia prevista: `zip` sin funciones por defecto, solo
  `deflate` (licencias MIT/Apache/Zlib; comprobar con `cargo deny`).
  `stack.xml` se escribe y lee sin crate de XML si basta un lector mínimo
  y robusto; si no, `quick-xml` (MIT).
- RNF-P4: ninguna operación de guardar, autoguardar, recuperar o cambiar
  de pestaña bloquea la interfaz más de 50 ms.
- Español, colores neutros, capturas existentes sin cambios salvo la barra
  de pestañas y el panel de historial (capturas nuevas en los tres temas).

## 8. Pruebas

- Ida y vuelta `.ora`: capas, nombres, opacidad, visibilidad, capa activa,
  medidas, píxeles exactos; `mergedimage.png` igual a la composición.
- `.ora` de referencia de GIMP y Krita (generados a mano, pequeños, en el
  repo) se abren con sus capas.
- Archivos `.ora` dañados (ZIP truncado, `stack.xml` roto, `src` que no
  existe, rutas con `..`, capas enormes, bombas de descompresión): error o
  aviso, nunca `panic`; fuzzing `open_ora`.
- Pestañas (kittest): abrir dos archivos → dos pestañas; pintar en una no
  cambia la otra; deshacer por pestaña; tarea de una pestaña se aplica a la
  suya aunque se cambie de pestaña; Ctrl+Tab/Ctrl+W; abrir un archivo ya
  abierto activa su pestaña; cerrar con cambios pregunta; cerrar la app con
  varias pestañas con cambios.
- Guardar: Guardar en documento nuevo pide ruta; Guardar como propone
  `nombre.ora` junto al origen; un fallo de escritura deja el archivo
  anterior intacto; el PNG de Fireworks nunca se sobrescribe.
- Autoguardado y recuperación: tras simular un cierre sin limpiar, el
  siguiente arranque ofrece recuperar y abre los documentos con su nombre;
  «Decidir después» los conserva; un cierre normal no deja nada; dos
  sesiones no se pisan.
- Panic hook: un `panic` en una prueba escribe una entrada en el log.
- Panel de historial: nombres de los pasos, clic salta, pasos descartados.
- Capturas nuevas: barra de pestañas con 3 pestañas (una con cambios) y
  panel de historial, en claro, oscuro y alto contraste; ventana de
  recuperación en oscuro.

## 9. Documentación

- ARQUITECTURA: RF-08, RF-09, RF-10, RF-20 ✔; §1.4 (autoguardado, log y
  panic hook tal como quedan); §1.5 (carpeta de datos); §9.1 fase 4 ✔;
  RNF-E1 completo.
- DESIGN.md: barra de pestañas, panel de historial, ventana de
  recuperación, ventana de cerrar con varias pestañas.
- `docs/ATAJOS.md` regenerado con Guardar, Guardar como, Cerrar pestaña y
  cambio de pestaña.

## 10. Decisiones del constructor del spec (revisables)

- Pestañas antes que `.ora`, en la misma fase (no 4a/4b).
- Guardar solo en `.ora`; el original (PNG, Fireworks…) nunca se
  sobrescribe y Guardar como propone `nombre.ora` a su lado.
- Exportar deja de quitar la marca de cambios sin guardar.
- Cerrar la app con varias pestañas con cambios: una sola ventana con
  Guardar todo / Descartar todo / Cancelar.
- Presupuesto de historial común a todas las pestañas.
- Panic hook solo escribe el log; la recuperación se apoya en el
  autoguardado.
- Autoguardados por sesión para que dos instancias no se pisen.
- Capas de grupo y modos de fusión de `.ora` ajenos: se aplanan o se leen
  como normales, con aviso.

## 11. Lo que queda al constructor

- Estructura interna de las pestañas (cómo se parte `PyroApp`).
- Formato del archivo de datos del autoguardado y de la marca de sesión.
- Lector XML mínimo propio o `quick-xml`.
- Forma exacta de los nombres de paso en el historial.
- Contenido de los `.ora` de referencia de GIMP y Krita.
