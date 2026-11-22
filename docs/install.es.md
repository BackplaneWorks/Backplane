<p align="left">
  <a href="install.md"><img src="media/readme/language-en.svg" alt="English" width="92" height="28"></a>
</p>

# Instalación

## Linux

```sh
{ curl -fsSL https://github.com/BackplaneWorks/Backplane/releases/latest/download/install.sh 2>/dev/null || wget -qO- https://github.com/BackplaneWorks/Backplane/releases/latest/download/install.sh; } | sh
backplane-app
```

La línea usa `curl` si existe y `wget` si no (un escritorio Ubuntu estándar tiene `wget` y no `curl`; macOS tiene `curl` y no `wget`); el instalador descarga con el que encuentre, igual que la instalación de KiCad.

x64 y arm64. Despliega la publicación para el guardián (`docs/deploy.md`): la compilación en `~/.local/share/backplane/builds/<tag>/`, `CURRENT` nombrando la que se ejecuta, y el guardián en `~/.local/share/backplane/backplane`, enlazado como `backplane` en `~/.local/bin` junto a `backplane-app` (la aplicación de escritorio). Añade Backplane a tu menú de aplicaciones; en macOS como Backplane.app en `~/Applications` (`BACKPLANE_NO_DESKTOP=1` lo omite). La aplicación de escritorio inicia el hub de su carpeta de datos bajo el guardián, que lo mantiene al día desde publicaciones firmadas, pone cada nueva compilación en periodo de prueba y vuelve atrás cuando una falla (`backplane hub status`, `restart`, `rollback`). `BACKPLANE_NO_DESKTOP_APP=1` instala solo el hub e inicia el guardián de inmediato (carpeta de datos `BACKPLANE_HOME`, por defecto `~/.backplane/hub`; puerto `BACKPLANE_PORT`, por defecto el primero libre desde 3800); `BACKPLANE_START=1` también lo inicia con la aplicación de escritorio. Vuelve a ejecutar el instalador mientras el guardián corre y le entregará la nueva compilación al guardián, en periodo de prueba. Añade `~/.local/bin` a tu `PATH` si se te pide.

## macOS (Apple silicon)

Backplane se ejecuta en Mac con Apple silicon (M1 o posterior) y macOS 13 o posterior; los Mac con Intel no son compatibles, y el instalador lo indica si se ejecuta en uno. El Backplane KiCad que instala necesita macOS 15 o posterior; en un macOS anterior, Ajustes → CAD indica el motivo.

Ejecuta el comando de una línea de arriba: la misma disposición, con la carpeta de datos por defecto en `~/Library/Application Support/Backplane/hub`, y **Backplane.app en `~/Applications`** (tu propia carpeta Aplicaciones: sin contraseña de administrador), así que Backplane aparece en Launchpad, Spotlight y el Dock. Ábrela desde ahí (o `open -a Backplane`). La aplicación no contiene ninguna compilación propia: su programa (`deploy/mac/launcher.c`) se reemplaza a sí mismo con el cliente de escritorio de la compilación que ejecuta el guardián (`~/.local/share/backplane/CURRENT`), así que las actualizaciones nunca tocan la app y el cliente es el propio proceso de la app. El hub que inicia se ejecuta como su proceso hijo. `BACKPLANE_NO_DESKTOP=1` omite la app (`backplane-app` en `~/.local/bin` sigue ejecutando el cliente desde una terminal); `BACKPLANE_NO_DESKTOP_APP=1` instala solo el hub. Con un `BACKPLANE_PREFIX` propio, el instalador lo escribe dentro de la app y vuelve a firmarla (ad hoc).

**Documentos, Escritorio y Descargas.** La primera vez que Backplane abre una de ellas (un proyecto que añadas ahí, o el selector de carpetas al entrar), macOS pregunta si *Backplane* tiene permiso; el hub, el guardián y los agentes se ejecutan bajo esa respuesta. Permítelo y el proyecto se abre. Si elegiste No permitir, Backplane lo indica en el selector o en la pestaña Archivos («macOS no ha dado acceso a Backplane a Documentos: permítelo en Configuración del Sistema → Privacidad y seguridad → Archivos y carpetas»): activa Backplane ahí, bajo Carpeta Documentos, e inténtalo de nuevo. Si se inicia desde una terminal (`backplane-app`), macOS pregunta para la terminal en su lugar.

**Las compilaciones de macOS aún no están notarizadas** (sin Apple Developer ID; la app y sus asistentes están firmados ad hoc). `install.sh` descarga con `curl`, que no marca nada en cuarentena, así que lo que instala se abre sin problemas. Si descomprimes a mano un archivo descargado con un navegador y macOS rechaza abrirlo, solo la primera vez: clic derecho (o Control + clic) → Abrir → Abrir; o en macOS 15+, intenta abrirlo, luego ve a Configuración del Sistema → Privacidad y seguridad → «Abrir de todos modos»; o ejecuta `xattr -dr com.apple.quarantine ~/Applications/Backplane.app ~/.local/share/backplane`. El instalador verifica el sha256 del archivo antes de instalar nada.

## Windows (x64, experimental)

Descarga `backplane-<tag>-windows-x64-native.zip`, extrae **todo** el archivo en una carpeta y ejecuta `backplane-gpui.exe`. El ejecutable aún no está firmado, así que Windows SmartScreen avisa la primera vez (mira abajo). La primera ejecución pregunta **¿Instalar Backplane en %LOCALAPPDATA%\Backplane? (necesario para actualizaciones)**: Instalar copia la publicación en `%LOCALAPPDATA%\Backplane\builds\<tag>`, coloca el guardián (`backplane.exe`, que actualiza, reinicia y revierte el hub) en `%LOCALAPPDATA%\Backplane`, añade **Backplane** al menú Inicio y arranca desde allí; después puedes borrar la carpeta que extrajiste. Ahora no lo ejecuta desde la carpeta extraída, sin actualizaciones, y pregunta de nuevo la próxima vez. Se necesita Git for Windows (los comandos de shell del hub usan su `sh`); instala tus CLIs de agente, KiCad y FreeCAD por separado (Ajustes → CAD los detecta). Cerrar la ventana deja el hub ejecutándose.

Para quitar Backplane: sal de la app y detén el hub (`%LOCALAPPDATA%\Backplane\backplane.exe hub stop`), luego borra la carpeta `%LOCALAPPDATA%\Backplane` y el acceso del menú Inicio `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Backplane.lnk`. Tus proyectos, hilos y ajustes permanecen en la carpeta de datos del hub, `%APPDATA%\Backplane\hub` (y los ajustes de la ventana en `%APPDATA%\backplane-gpui`); bórralos también para eliminarlo todo.

## Desde 0.14

En Linux y macOS, pulsa Actualizar en 0.14 como de costumbre: 0.15 se instala al lado, arranca en el mismo puerto y ofrece importar tus proyectos, hilos, bots y teléfonos vinculados de 0.14 (`docs/deploy.md`, «From 0.14»). Vuelve a vincular tus otras máquinas en Ajustes, Dispositivos. Si 0.15 falla al iniciar, 0.14 regresa por sí solo. En Windows, 0.14 no puede actualizarse solo: ciérralo, descarga `backplane-<tag>-windows-x64-native.zip` de la publicación, extráelo y ejecuta `backplane-gpui.exe` (Instalar, como arriba); su configuración ofrece la misma importación.

## Qué contiene el archivo de cada plataforma

Cada archivo contiene el hub, el cliente de escritorio, el guardián, `backplane-voice` (dictado), `backplane-browser` (el asistente de la pestaña Navegador), `backplane-step2glb` (STEP a GLB con OpenCascade, de forma nativa: sin necesitar FreeCAD) y `assets/` con los íconos y modelos de gatos generados; `release.yml` falla si falta cualquiera. El cliente también incluye Chromium embebido (feature `cef`) con su runtime al lado: en Linux `libcef.so`, los archivos `.pak`, `icudtl.dat`, `v8_context_snapshot.bin` y `locales/`; en Windows lo mismo con `libcef.dll` y `chrome_elf.dll`; en macOS `Chromium Embedded Framework.framework` y las apps auxiliares `Backplane Helper*.app`, junto al cliente. El archivo de macOS también incluye `Backplane.app` (`Contents/MacOS/Backplane`, el lanzador; `Contents/Info.plist`, bundle id `works.backplane.gpui`; `Contents/Resources/Backplane.icns`; firmado ad hoc), que `install.sh` copia a `~/Applications`; `release.yml` verifica que esté ahí y firmado, y que cada Mach-O en el archivo pida como máximo macOS 13.0. La pestaña Navegador es ese Chromium dentro de la ventana en cada plataforma.

## Teléfonos

- **iOS:** instala mediante TestFlight con una invitación.
- **Android:** descarga el APK firmado desde la publicación e instálalo.

Para vincular un teléfono u otro navegador, abre **Ajustes → Dispositivos → Añadir un dispositivo** y usa el enlace de vinculación, el código QR o el código corto. El mismo menú lista los dispositivos vinculados y los revoca.

## Primera ejecución

Necesitas al menos una CLI de agente (`claude`, `codex`, `grok` o `muse`; Antigravity se instala desde Ajustes), KiCad para trabajar con placas y FreeCAD para trabajo mecánico. La primera ejecución pregunta por los conteos de uso anónimos en una ventana emergente clara. Están activados por defecto, no se envía nada antes de que respondas, y Ajustes los desactiva en cualquier momento. Consulta [telemetría](telemetry.md).

Trata los enlaces de vinculación y los códigos QR como credenciales. La vinculación en navegador intercambia el fragmento por una cookie HttpOnly, SameSite=Strict (Secure sobre HTTPS), y luego elimina el fragmento; los tokens existentes del navegador migran fuera de local storage tras un intercambio exitoso. Los clientes nativos de teléfono conservan su protocolo de enlace de vinculación. Cada cliente necesita la credencial de vinculación, también en esta máquina: la app de escritorio lee el token del hub desde su carpeta de datos (`<home>/token`, legible solo por tu usuario), y un navegador en esta máquina se vincula como cualquier otro (un enlace de vinculación desde Ajustes, o `backplane` abriéndolo en una instalación sin la app de escritorio). Acceso completo (*Full Access*) ejecuta las herramientas del agente con el acceso de tu usuario; elige un modo de permisos más restrictivo en el hilo cuando lo desees. La vinculación controla el acceso al hub y no proporciona aislamiento del SO para un agente con Acceso completo.

El hub conserva el archivo `<home>/token` existente byte a byte, incluido un enlace simbólico a un archivo regular. Como antes, los espacios en blanco alrededor se ignoran al vincular. Los archivos de token vacíos, ilegibles o no regulares detienen el arranque con un error en lugar de generar un reemplazo. Restaura el token original desde una copia de seguridad para conservar los clientes vinculados. En Unix el archivo abierto se restringe al modo 0600; en Windows el control de acceso proviene de la ACL de la carpeta home; los bits de modo POSIX no establecen una ACL privada en Windows.

Un token faltante se crea de forma exclusiva con modo 0600, sin requerir enlaces duros. Una creación interrumpida deja `token.backplane-creating` y rechaza el arranque. Conserva ambos archivos y recupera la credencial original antes de resolver ese marcador; no borres un token existente para silenciar el error. El bloqueo normal de la carpeta home serializa el arranque. Los sistemas de archivos deben admitir creación exclusiva y vaciado a disco (*file flushing*); en Unix también se comprueba el vaciado de directorio. La durabilidad ante cortes de energía en Windows, el comportamiento nativo de ACL y las garantías de sistemas de archivos en red requieren validación por plataforma.

Para desinstalar lo que `install.sh` instaló, detén el guardián (`backplane hub stop`) y luego:

Para una instalación personalizada, sustituye tus rutas `BACKPLANE_PREFIX`, `BACKPLANE_BIN` y `XDG_DATA_HOME` en los comandos de abajo.

```sh
rm -rf ~/.local/share/backplane
rm -f ~/.local/bin/backplane ~/.local/bin/backplane-app
rm -f ~/.local/share/applications/backplane.desktop \
  ~/.local/share/icons/hicolor/scalable/apps/backplane-desktop.svg
rm -rf ~/Applications/Backplane.app   # macOS
```

Tus hilos, ajustes y claves permanecen en la carpeta home (`~/.backplane/hub`, o `~/Library/Application Support/Backplane/hub` en macOS); bórrala también para empezar de cero. Una instalación mediante paquete (`.deb`, AUR) se desinstala desde su gestor de paquetes.

## Solución de problemas

**Linux: agentes y scopes de memoria.** En Linux el hub inicia cada agente, cada tarea `work_run`, el KiCad embebido y el asistente de navegador en su propio scope de systemd (`systemd-run --user --scope`), de modo que un proceso desbocado muera solo y no junto con el hub (`docs/HUB-RESILIENCE.md`). Al arrancar, el hub crea un scope de prueba y conserva solo las propiedades que tu systemd acepta: systemd admite `OOMPolicy` en un scope desde la versión 253, y en la 252 solo desde una revisión posterior (la 252.39 de Debian 12 lo incluye; la 249 de Ubuntu 22.04, 247 de Debian 11, 245 de Ubuntu 20.04 y 241 de Debian 10 no la tienen y nunca se les pide), y systemd anterior a 248 no tiene `ManagedOOMPreference`. El registro del hub indica lo que encontró, por ejemplo `memscope: agents and jobs run in scopes of their own in bpagent.slice (...; systemd 249; without OOMPolicy=continue (not supported here))`. Donde no se puede crear ningún scope (sin `systemd-run`, sin bus de sesión de usuario, sin systemd de usuario como en muchos contenedores y WSL, o una distribución sin systemd), los agentes arrancan sin uno y el registro lo indica una vez: `memscope: off (...)`. Si un arranque con scope aún falla (el gestor de usuario desapareció), ese agente vuelve a arrancar sin scope, y todos los siguientes también hasta que el hub se reinicie. 0.16.0 no tenía esto, y en esos sistemas cada turno fallaba con «agent exited with code 1» o `Unknown assignment: OOMPolicy=continue`.

`BACKPLANE_NO_SCOPE=1` desactiva los scopes manualmente (`BACKPLANE_NO_SCOPE=1 backplane-app`, o en el entorno con el que el guardián inicia el hub). Ya no deberías necesitarlo; si lo necesitas, por favor reporta las líneas `memscope:` del registro del hub.

Las compilaciones nocturnas (*nightly*) de `main` son versiones preliminares en la página de [Releases](https://github.com/BackplaneWorks/Backplane/releases), cada una con su propio instalador: `{ curl -fsSL https://github.com/BackplaneWorks/Backplane/releases/download/<tag>/install.sh 2>/dev/null || wget -qO- https://github.com/BackplaneWorks/Backplane/releases/download/<tag>/install.sh; } | sh`.
