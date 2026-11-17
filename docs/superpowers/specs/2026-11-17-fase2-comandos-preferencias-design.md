# Fase 2: comandos, atajos y preferencias

Fecha: 2026-11-17. Autor de las decisiones: el autor del proyecto (usuario único, ver `PRODUCT.md`). Criterio de salida de la fase: RF-13 a RF-16 de `docs/ARQUITECTURA.md`, con el alcance acotado en este documento.

## 1. Intención

El autor trabaja con los atajos de **Adobe Fireworks CS6** en la memoria muscular y quiere que Pyrotecnika responda igual desde el primer arranque, pudiendo además cambiarlos. A la vez, la app debe recordar lo que el autor ajusta (tema, escala, archivos recientes) y ofrecer una forma rápida de encontrar cualquier acción sin recordar en qué menú está.

Éxito: el autor abre Pyrotecnika en una máquina nueva y sus dedos de Fireworks funcionan; si cambia un atajo, el menú, la paleta y el teclado muestran y usan el nuevo; al reabrir la app encuentra su tema, su escala y sus últimos archivos.

## 2. Objetivos

1. **Atajos de fábrica = Fireworks CS6** (juego «Fireworks» de `Fireworks.xml`, leído de la instalación local el 2026-11-17) para todas las acciones que ya existen. Tabla en §6.
2. **Atajos configurables** (RF-13): editor en Preferencias, varios atajos por acción, aviso de conflicto con reasignación decidida por el usuario, restablecer por acción y todos, exportar e importar.
3. **Paleta de comandos** con Ctrl+K (⌘K) que busca cualquier acción por nombre (RF-14).
4. **Archivos recientes** persistentes con «Limpiar lista», y **Mostrar en carpeta** para el archivo actual (RF-15).
5. **Preferencias persistentes** de lo que ya existe: tema y escala de la interfaz, más los atajos (RF-16, acotado).
6. Un **catálogo único de comandos** del que leen el teclado, los menús, la paleta y el editor de atajos.

## 3. No objetivos (fuera de esta fase)

- Preferencias de funciones que aún no existen: idioma (fase 7), reducir movimiento (fase 7), memoria del historial (fase 3), autoguardado (fase 4). Cada fase añade su opción al llegar.
- Preferencia de actualizaciones: la búsqueda es solo manual por decisión anterior del autor; no hay nada que desactivar.
- Atajos de Fireworks cuyas funciones no existen (Guardar Ctrl+S, Recortar herramienta C, Mano H, Zoom Z, Tab para ocultar paneles, flechas para mover 1/10 px, Transformar Ctrl+T…). Quedan como **reservados** en el catálogo documentado (§6.3) para cuando lleguen, sin asignarse a otra cosa por defecto.
- SQLite. Los recientes van en TOML; SQLite se valorará cuando existan presets de exportación o de pinceles.
- Importar archivos XML de atajos de Fireworks: el autor nunca personalizó los suyos (la carpeta de usuario está vacía).
- Menú de aplicación de macOS (fase 5). Preferencias vive en Editar en los tres SO por ahora.

## 4. Antiobjetivos (fallaría aunque funcionara)

- Un atajo cambiado que un menú o la paleta siguen mostrando con el valor viejo.
- Un atajo que se pisa en silencio al reasignar.
- La app no arranca, o pierde la configuración sin avisar, porque un archivo de configuración está dañado o es de otra versión.
- Atajos que se disparan mientras el usuario escribe en un campo de texto.
- Romper el orden de prioridad actual: con un diálogo modal abierto, los atajos del documento no actúan.

## 5. Enfoques considerados

- **A. Catálogo único de comandos + archivos TOML (elegido por el autor).** Cada comando se define una vez (identificador estable, nombre, categoría, atajos de fábrica). Teclado, menús, paleta y editor leen de él. Preferencias y recientes en `settings.toml`; atajos personalizados en `keymap.toml` (solo diferencias con Fireworks), que también es el formato de exportación e importación. Dependencias pequeñas en Rust puro: `serde`, `toml`, `directories`.
- **B. Igual que A con recientes en SQLite** (lo que decía `ARQUITECTURA.md` §1.5). Descartado ahora: código C, más compilación y tamaño por una lista de 10 rutas. Se puede añadir sin rehacer nada.
- **C. Dejar los atajos repartidos y añadir una tabla de sustituciones.** Descartado: menús, paleta y editor tendrían copias distintas y se desincronizarían (antiobjetivo 1).

## 6. Atajos de fábrica

Notación: `Ctrl` es ⌘ en macOS (egui `Modifiers::COMMAND`); `Alt` es ⌥; `Mayús` es ⇧.

### 6.1 Comandos con atajo

| Categoría | Comando | Atajo(s) de fábrica | Hoy |
|---|---|---|---|
| Archivo | Nuevo… | Ctrl+N | = |
| Archivo | Abrir… | Ctrl+O | = |
| Archivo | Exportar… | **Ctrl+Mayús+R** | Ctrl+E |
| Archivo | Salir | **Ctrl+Q** | — |
| Editar | Deshacer | Ctrl+Z | = |
| Editar | Rehacer | Ctrl+Mayús+Z | también Ctrl+Y (se quita: Fireworks no lo usa) |
| Editar | Borrar selección | Supr, Retroceso | = |
| Editar | Paleta de comandos | Ctrl+K | nuevo (en Fireworks Ctrl+K es «Ajustar a píxel», que no existe aquí) |
| Selección | Seleccionar todo | Ctrl+A | = |
| Selección | Deseleccionar | Ctrl+D | = |
| Selección | Invertir selección | Ctrl+Mayús+I | = |
| Capas | Nueva capa | **Mayús+L** (Insertar capa) | — |
| Capas | Duplicar capa | **Ctrl+Alt+D** | — |
| Capas | Subir capa | **Ctrl+Mayús+F** (Traer adelante) | — |
| Capas | Bajar capa | **Ctrl+Mayús+B** (Enviar atrás) | — |
| Capas | Combinar abajo | **Ctrl+Alt+E** | — |
| Ver | Acercar | Ctrl+= y Ctrl++ | = |
| Ver | Alejar | Ctrl+- | = |
| Ver | Ajustar a la ventana | **Ctrl+Alt+0** | Ctrl+0 |
| Ver | Tamaño real | Ctrl+1 | = (no figura en `Fireworks.xml`; es el del menú Ver de Fireworks, decisión del constructor del spec) |
| Herramientas | Mover | V | = |
| Herramientas | Marco · Elipse | **M** (alterna entre las dos) | Elipse era U |
| Herramientas | Lazo | L | = |
| Herramientas | Varita mágica | W | = |
| Herramientas | Pincel | B | = |
| Herramientas | Borrador | E | = |
| Herramientas | Tampón de clonar | S | = |
| Herramientas | Desenfocar | R | = |
| Herramientas | Bote de pintura | G | = |
| Herramientas | Cuentagotas | I | = |
| Pincel | Reducir tamaño | [ | = |
| Pincel | Aumentar tamaño | ] | = |

`Ctrl+=` en Fireworks es la tecla virtual `VK_OEM_PLUS` (187), que en el teclado español es la tecla `+`; por eso se asignan ambos.

### 6.2 Comandos sin atajo de fábrica (en menús y paleta)

Abrir reciente (submenú), Limpiar recientes, Mostrar en carpeta, Recortar a la selección, Eliminar capa, cada ajuste y filtro (Brillo/contraste…, Tono/saturación…, Niveles…, Invertir colores, Escala de grises, Desenfoque gaussiano…, Enfocar…), Tema (Seguir al sistema, Claro, Oscuro, Alto contraste), Tamaño de la interfaz (Aumentar, Reducir, Restablecer), Preferencias…, Buscar actualizaciones…

### 6.3 Reservados de Fireworks

Combinaciones que Fireworks usa para funciones que Pyrotecnika aún no tiene. No se asignan de fábrica a otra cosa y se documentan en el código junto al catálogo: Ctrl+S, Ctrl+Mayús+S, Ctrl+W, Ctrl+X/C/V, Ctrl+T, Ctrl+Mayús+T, Ctrl+G, Ctrl+R (Importar), Ctrl+Mayús+X (vista previa de exportación), Ctrl+0 (ajustar a la selección), C, H, Z, K, P, T, U, N, Y, flechas y Mayús+flechas, Tab. El usuario sí puede asignarlas a lo que quiera.

## 7. Comportamiento

### 7.1 Catálogo de comandos

- Cada comando tiene un **identificador estable** en texto (`file.export`, `layer.merge_down`, `tool.brush`, `filter.gaussian_blur`…) que es lo que se guarda en `keymap.toml`; cambiar el nombre visible nunca invalida la configuración.
- Cada comando sabe si está **disponible** en el estado actual (p. ej. Deseleccionar sin selección), lo que usan menús y paleta para deshabilitar.
- Los menús se generan a partir del catálogo en el orden actual de `panels.rs`; el texto del atajo de cada entrada sale del mapa vigente.
- **Nuevo menú Capas** entre Selección y Filtros: Nueva capa, Duplicar capa, Eliminar capa, Subir capa, Bajar capa, Combinar abajo. Los botones del panel de capas siguen existiendo y muestran en su tooltip el nombre y el atajo vigente.
- **Un solo camino de ejecución:** teclado, menú, paleta y botones de panel ejecutan el comando por la misma función (la que hoy hace `before = doc.clone()`, `history.push` y marca el lienzo sucio para las capas), así que se comportan igual venga de donde venga.

### 7.2 Resolución de atajos

- Se mantiene la prioridad actual: con la confirmación de cambios abierta o un campo de texto con el teclado, no se resuelve ningún atajo; con otros diálogos modales, solo los que hoy ya se permiten (los de tema y escala).
- Una pulsación se resuelve contra el mapa vigente. Coincidencia exacta de modificadores (Ctrl+Mayús+Z no dispara Ctrl+Z).
- **Herramientas que comparten tecla sin modificadores alternan**, como en Fireworks. Marco y Elipse son **dos comandos** (`tool.marquee`, `tool.ellipse`), ambos con M de fábrica. Regla: al pulsar la tecla se toma el grupo de herramientas que **tienen hoy** esa tecla, en orden del catálogo; si la herramienta activa está en el grupo se pasa a la siguiente (circular); si no, a la primera del grupo. Así, si el usuario mueve Marco a Q, M solo lleva a Elipse; si le quita M a Elipse, M solo lleva a Marco. Compartir tecla entre herramientas no cuenta como conflicto; cualquier otra coincidencia sí.
- Con teclados con AltGr (Windows), AltGr llega como Ctrl+Alt; los atajos Ctrl+Alt de Fireworks pueden dispararse con AltGr fuera de un campo de texto. Es el mismo comportamiento que Fireworks y se acepta.

### 7.3 Editor de atajos (Preferencias › Atajos)

```
Atajos de teclado                       [Buscar…        ]
 Archivo
   Nuevo…                 Ctrl+N
   Exportar…              Ctrl+Mayús+R   ← seleccionado
 …
 ───────────────────────────────────────────────
 Exportar…:  [Ctrl+Mayús+R ×]  [+ Añadir]   [Restablecer]
 ⚠ Ctrl+Mayús+K ya lo usa «Paleta de comandos». [Reasignar] [Cancelar]
              [Exportar…] [Importar…] [Restablecer todos a Fireworks]
```

- Lista agrupada por categoría con búsqueda por nombre o por atajo.
- «+ Añadir» espera la siguiente combinación; Esc cancela la captura. Una tecla sin ningún modificador solo se admite para herramientas y para los comandos que ya la usan de fábrica (Supr, Retroceso, [ y ]); para el resto se pide al menos un modificador, y **Mayús cuenta** (Mayús+L de Nueva capa es válido). La misma regla la aplican la captura del editor, la lectura de `keymap.toml` y la importación.
- Si la combinación ya la usa otro comando: aviso con el nombre de ese comando y botones Reasignar (se la quita al otro) o Cancelar.
- Los atajos modificados se distinguen de los de fábrica sin usar solo el color (p. ej. texto semibold y la marca «modificado» en el nombre accesible).
- Exportar escribe el `keymap.toml` vigente donde diga el usuario. **Importar reemplaza** tus personalizaciones por las del archivo (un comando que no aparece en él vuelve a su atajo de Fireworks); antes de aplicar se valida: identificadores desconocidos y combinaciones inválidas se ignoran y se informan; si dos comandos del archivo usan la misma combinación, gana el primero y se informa. La misma regla de conflictos vale al leer al arrancar un `keymap.toml` editado a mano (aviso en la barra de estado).

### 7.4 Paleta de comandos

- Ctrl+K abre una ventana flotante centrada arriba con un campo de búsqueda y la lista de comandos (nombre, categoría, atajo vigente).
- Búsqueda insensible a mayúsculas y a tildes, por subcadena y por iniciales; los disponibles primero, los no disponibles se muestran deshabilitados.
- Flechas mueven la selección, Intro ejecuta, Esc o clic fuera cierra. La ejecución pasa por el mismo camino que el menú (incluida la confirmación de cambios sin exportar cuando aplique).
- Incluye los archivos recientes como «Abrir reciente: nombre.png».
- Accesible: la lista anuncia el elemento seleccionado y su atajo.

### 7.5 Recientes y Mostrar en carpeta

- Se registra un archivo al **abrirlo con éxito**. Máximo 10, el más reciente primero, sin duplicados (ruta normalizada).
- Archivo › Abrir reciente lista nombre del archivo con la ruta completa en el tooltip; al final «Limpiar lista».
- Si al elegirlo el archivo ya no existe: aviso en la barra de estado y se quita de la lista.
- Archivo › Mostrar en carpeta abre el gestor de archivos del SO con el archivo actual seleccionado (Windows: Explorador; macOS: Finder; Linux: el gestor por defecto vía `org.freedesktop.FileManager1` y, si no responde, abrir la carpeta). Deshabilitado si el documento no viene de un archivo. Vive en `pyro-platform` (regla de CI: código por SO solo ahí).

### 7.6 Preferencias

- Editar › Preferencias… abre una ventana modal con dos secciones: **Apariencia** (tema, tamaño de la interfaz) y **Atajos** (§7.3).
- Los cambios se aplican al momento y se guardan al cerrar la ventana y también al cambiar tema o escala desde el menú Ver.
- El menú Ver › Tema y Tamaño de la interfaz siguen existiendo y escriben la misma preferencia.

### 7.7 Archivos en disco

- Carpeta: `directories::ProjectDirs` (`config_dir()`): `%APPDATA%\Pyrotecnika\config\` en Windows, `~/Library/Application Support/<id>/` en macOS, `~/.config/pyrotecnika/` en Linux.
- `settings.toml`: `schema = 1`, `[appearance] theme = "system" | "light" | "dark" | "high-contrast"`, `ui_scale = 1.0`, `[recent] files = [...]`.
- `keymap.toml`: `schema = 1` y una tabla `[bindings]` con `"file.export" = ["Ctrl+Shift+R"]`, solo para los comandos que difieren de fábrica; una lista vacía significa «sin atajo». Combinaciones en texto inglés neutro (`Ctrl`, `Alt`, `Shift`, nombre de tecla de egui); en pantalla se muestran localizadas (Mayús, Supr) y con símbolos en macOS.
- Escritura atómica (archivo temporal + renombrar) para no dejar archivos a medias.
- Lectura tolerante: claves desconocidas se ignoran; valores fuera de rango se acotan (escala 0.8–2.0). Un archivo ilegible se renombra a `*.toml.bak`, la app arranca con los valores de fábrica y avisa una vez en la barra de estado. Un `schema` mayor que el conocido se lee con lo que se entienda y no se sobrescribe hasta que el usuario cambie algo; antes de esa primera escritura se copia a `*.toml.bak` (una vez) y se avisa.
- Ninguna lectura ni escritura de estos archivos bloquea la UI de forma perceptible (son de pocos KB; se aceptan síncronas).

## 8. Restricciones

- Reglas del CI vigentes: `cfg(target_os)` solo en `pyro-platform`; `pyro-core` y `pyro-formats` sin dependencias de interfaz; sin `Color32` literales fuera de `crates/pyro-ui/src/design/`.
- Licencias: nada GPL/LGPL/AGPL/MPL (`cargo deny`). `serde` (MIT/Apache), `toml` (MIT/Apache), `directories` (MIT/Apache) cumplen; el constructor lo verifica con `cargo deny`.
- Sistema de diseño de la fase 1: los diálogos nuevos usan los tokens, `widgets::primary_button` para la acción principal y el anillo de foco global; todo operable con teclado y con nombre accesible.
- Interfaz en español.
- Ningún archivo de más de ~600 líneas (RNF-M1): `input.rs`, `panels.rs` y `mod.rs` ya son grandes; el catálogo, el mapa, el editor y la paleta van en módulos propios.

## 9. Pruebas

- Unitarias: análisis y escritura de combinaciones (ida y vuelta), mapa de fábrica igual a la tabla §6.1, detección de conflictos, alternancia de herramientas con la misma tecla, fusión fábrica + `keymap.toml`, lectura tolerante (dañado, versión mayor, claves desconocidas, escala fuera de rango), lista de recientes (tope, orden, duplicados).
- `egui_kittest`: reasignar un atajo en el editor y comprobar que el menú muestra el nuevo y que la pulsación ejecuta el comando (flujo citado en `ARQUITECTURA.md` §7); abrir la paleta con Ctrl+K, buscar «gaus» y ejecutar con Intro; ningún atajo actúa con un campo de texto enfocado.
- Capturas de referencia nuevas: paleta abierta y Preferencias (Apariencia y Atajos) en los tres temas.
- Las pruebas usan una carpeta de configuración temporal, nunca la real del usuario.

## 10. Documentación

- `ARQUITECTURA.md` §1.5: recientes en `settings.toml`; SQLite pospuesto hasta los presets. §5.1: RF-13 a RF-16 con el alcance de este documento. §9.1: fase 2 ✔ al terminar.
- `docs/` gana una tabla de atajos de fábrica para el usuario (puede generarse del catálogo).

## 11. Decisiones del constructor del spec (revisables)

- Rehacer pierde Ctrl+Y para quedar igual que Fireworks.
- Tamaño real = Ctrl+1 aunque no figure en `Fireworks.xml`.
- Paleta en Ctrl+K pese a que Fireworks lo use para «Ajustar a píxel» (inexistente aquí).
- Capas: Mayús+L, Ctrl+Mayús+F/B y Ctrl+Alt+D se toman de los equivalentes de Fireworks para objetos y capas.
- Preferencias sin atajo de fábrica.
- Recientes: solo archivos abiertos, no exportados.
- Menú Capas nuevo (Fireworks no lo tiene como menú propio, pero sí los editores de la categoría como Paint Shop Pro, y hace las acciones de capa encontrables).
- Marco y Elipse son dos comandos que comparten M; la alternancia sigue a qué herramientas tienen la tecla en ese momento.
- Mayús cuenta como modificador para atajos de comandos que no son herramientas.
- Importar reemplaza las personalizaciones, no se mezcla con ellas.
- Un `settings.toml` de una versión más nueva se copia a `.bak` antes de sobrescribirlo.

## 12. Lo que queda al constructor

Estructura interna de módulos y tipos, forma exacta del índice de búsqueda de la paleta, aspecto de la paleta y del editor dentro del sistema de diseño, nombres de los identificadores de comando siguiendo el patrón `categoría.acción`.
