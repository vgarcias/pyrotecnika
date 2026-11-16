# Instalación de Pyrotecnika

Los instaladores de cada versión están en
[GitHub Releases](https://github.com/vgarcias/pyrotecnika/releases/latest).

| Sistema | Uso personal | Despliegue (IT / MDM) |
|---|---|---|
| Windows 10/11 (x64) | `pyrotecnika_X.Y.Z_x64-setup.exe` | `pyrotecnika_X.Y.Z_x64_*.msi` |
| macOS 11 o posterior (Apple Silicon e Intel) | `Pyrotecnika_X.Y.Z_universal.dmg` | `Pyrotecnika_X.Y.Z_universal.pkg` |
| Linux x86_64 | `pyrotecnika_X.Y.Z_x86_64.AppImage` | `.deb` (Debian, Ubuntu, Mint) · `.rpm` (Fedora, openSUSE, RHEL) |

> **Los paquetes todavía no están firmados.** Windows y macOS muestran un aviso
> la primera vez; abajo se explica cómo continuar. Comprueba siempre la suma
> SHA-256 de lo que descargues (ver [Verificar la descarga](#verificar-la-descarga)).

## Requisitos

- **Windows:** 10 u 11 de 64 bits, con DirectX 12 o Vulkan (cualquier tarjeta gráfica de los últimos diez años).
- **macOS:** 11 Big Sur o posterior. El mismo paquete funciona en Apple Silicon y en Intel.
- **Linux:** x86_64 con glibc 2.35 o posterior (Ubuntu 22.04, Debian 12, Fedora 36 o equivalentes), GTK 3 y controladores Vulkan u OpenGL (Mesa sirve). Funciona en Wayland y en X11.

## Windows

### Instalador `.exe` (recomendado)

1. Ejecuta `pyrotecnika_X.Y.Z_x64-setup.exe`.
2. Si aparece **"Windows protegió su PC"** (SmartScreen), pulsa
   **Más información → Ejecutar de todas formas**.
3. Se instala solo para tu usuario en `%LOCALAPPDATA%\Pyrotecnika` y no pide permisos de administrador.

La instalación silenciosa es `pyrotecnika_X.Y.Z_x64-setup.exe /S`.

### Paquete `.msi` (por máquina)

Pensado para administradores: instala en `Program Files` para todos los usuarios
y se puede desplegar por GPO, Intune o SCCM.

```powershell
msiexec /i pyrotecnika_X.Y.Z_x64_en-US.msi /qn
```

Pyrotecnika **no** se registra como programa predeterminado para PNG o JPG. Para
abrir imágenes con ella usa *Abrir con → Elegir otra aplicación*.

## macOS

### `.dmg` (recomendado)

1. Abre el `.dmg` y arrastra **Pyrotecnika** a **Aplicaciones**.
2. La primera vez, macOS dirá que no puede comprobar el desarrollador. Haz
   **clic derecho (o Control + clic) sobre la app → Abrir → Abrir**.
   En macOS 15 o posterior, si no aparece esa opción: *Ajustes del Sistema →
   Privacidad y seguridad →* **Abrir igualmente** (al final de la página).

Pyrotecnika aparece en *Abrir con* para PNG, JPEG, GIF, WebP, BMP y TIFF, sin
quitarle a Vista Previa el papel de app predeterminada.

### `.pkg` (MDM)

Instala `Pyrotecnika.app` en `/Applications`. Para instalarlo desde la terminal:

```sh
sudo installer -pkg Pyrotecnika_X.Y.Z_universal.pkg -target /
```

Al no estar firmado, los sistemas MDM que exigen paquetes firmados lo rechazarán
hasta que se active la firma de código.

## Linux

### AppImage (cualquier distribución)

```sh
chmod +x pyrotecnika_X.Y.Z_x86_64.AppImage
./pyrotecnika_X.Y.Z_x86_64.AppImage
```

Si da un error sobre FUSE, instala `libfuse2` (Ubuntu 22.04: `sudo apt install libfuse2`;
Ubuntu 24.04 o posterior: `sudo apt install libfuse2t64`).

### Debian, Ubuntu y derivadas

```sh
sudo apt install ./pyrotecnika_X.Y.Z_amd64.deb
```

### Fedora, openSUSE, RHEL

```sh
sudo dnf install ./pyrotecnika-X.Y.Z-1.x86_64.rpm      # Fedora / RHEL
sudo zypper install ./pyrotecnika-X.Y.Z-1.x86_64.rpm   # openSUSE
```

Los paquetes `.deb` y `.rpm` añaden Pyrotecnika al menú de aplicaciones y a
*Abrir con* para las imágenes.

## Verificar la descarga

Cada versión publica `SHA256SUMS.txt`. Compara la suma con la del archivo descargado:

```powershell
# Windows (PowerShell)
Get-FileHash .\pyrotecnika_X.Y.Z_x64-setup.exe -Algorithm SHA256
```

```sh
# macOS
shasum -a 256 Pyrotecnika_X.Y.Z_universal.dmg
# Linux (comprueba todos los archivos descargados a la vez)
sha256sum --ignore-missing -c SHA256SUMS.txt
```

## Actualizar

Pyrotecnika nunca se conecta a internet por su cuenta. Para buscar una versión
nueva usa **Ayuda › Buscar actualizaciones…**: consulta el último release
estable en GitHub (GitHub ve tu dirección IP, como en cualquier descarga).

| Instalado con | Qué hace |
|---|---|
| `.exe` (Windows), `.dmg` (macOS), AppImage | Descarga la versión nueva, **comprueba su firma** y la instala; después pide reiniciar. En Windows la app se cierra y el instalador la vuelve a abrir. |
| `.msi`, `.pkg`, `.deb`, `.rpm` | Solo avisa y enlaza a las descargas: instala la versión nueva con el mismo método (o lo hará tu departamento de IT). |

Si tienes cambios sin exportar, primero te pregunta si quieres exportarlos.

Cada paquete se firma con una llave minisign y la app solo instala lo que esté
firmado con ella; la clave pública está en
[`packaging/updater-pubkey.txt`](../packaging/updater-pubkey.txt). Si la firma
no coincide, no se instala nada.

## Desinstalar

| Formato | Cómo |
|---|---|
| Windows `.exe` | *Configuración → Aplicaciones → Aplicaciones instaladas → Pyrotecnika → Desinstalar*, o `%LOCALAPPDATA%\Pyrotecnika\uninstall.exe` (`/S` para hacerlo en silencio) |
| Windows `.msi` | *Aplicaciones instaladas*, o `msiexec /x pyrotecnika_X.Y.Z_x64_en-US.msi /qn` |
| macOS | Arrastra `Pyrotecnika.app` desde *Aplicaciones* a la Papelera |
| `.deb` | `sudo apt remove pyrotecnika` |
| `.rpm` | `sudo dnf remove pyrotecnika` (o `sudo zypper remove pyrotecnika`) |
| AppImage | Borra el archivo |

Esta versión todavía no guarda preferencias ni otros datos fuera de los
archivos que tú abres y guardas, así que desinstalar no deja restos.
