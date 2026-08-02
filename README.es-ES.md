

# OpenNiri-Windows

[![CI](https://github.com/AdEx-Partners-DE/OpenNiri-Windows/actions/workflows/ci.yml/badge.svg)](https://github.com/AdEx-Partners-DE/OpenNiri-Windows/actions/workflows/ci.yml)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](LICENSE)

OpenNiri-Windows es un administrador de ventanas de tipo *tiling* (mosaico) desplazable para Windows 10/11, desarrollado en Rust.

Incorpora el modelo de espacio de trabajo horizontal al estilo Niri a Windows nativo sin reemplazar DWM.

## Posicionamiento del Producto

La mayoría de los programas *tiling* para Windows se basan en árboles/BSP. OpenNiri-Windows prioriza el desplazamiento (*scroll-first*):

- Las ventanas se disponen en una cinta horizontal.
- Tu monitor actúa como un cuadro de visualización (*viewport*) sobre esa cinta.
- La navegación mantiene una coherencia espacial a medida que se agregan ventanas.
- Te mueves a través del contexto del espacio de trabajo en lugar de reconstruir constantemente árboles de división.

## ¿A quién va dirigido?

- Usuarios que trabajan principalmente con el teclado y gestionan muchas ventanas a diario.
- Ingenieros, analistas, operadores y creadores con configuraciones de un monitor o varios monitores.
- Equipos que buscan un *tiling* para Windows de código abierto con arquitectura transparente y base de código en Rust.

## Resumen de Capacidades

Implementado actualmente:

- Espacios de trabajo para varios monitores con comandos de enfoque y movimiento conscientes del monitor
- Atajos globales con recarga de configuración en tiempo real
- Alternancia de ventanas flotantes y pantalla completa
- Preajustes de ancho (`Win+1/2/3`) y equilibrado (`Win+0`)
- Animaciones de desplazamiento suaves, sugerencias de ajuste (*snap*) y gestos del trackpad
- Opción de enfoque que sigue al mouse (*focus-follows-mouse*)
- Acciones en la bandeja del sistema (pausar/recargar/abrir config/abrir registros/salir)
- Persistencia del espacio de trabajo y comportamiento más seguro de apagado/recuperación
- Modo seguro para resolución de problemas (`--safe-mode`)
- Diagnóstico integrado (`openniri-cli doctor`)

## Estado del Producto

OpenNiri-Windows está en fase **alfa** y en desarrollo activo.

Lo que esto significa en la práctica:

- El comportamiento principal está implementado y probado en CI.
- La UX prioriza actualmente el teclado y la configuración (aún no hay un flujo completo de configuración por GUI).
- Algunas ventanas gestionadas por Windows/sistema pueden rechazar operaciones de movimiento o estilo.
- Estado de lanzamiento actual (a 2026-02-08): aún no hay un lanzamiento público etiquetado, por lo que la compilación desde código fuente es la ruta de instalación principal.

## Inicio Rápido

### Opción A: Descarga (Cuando esté disponible)

1. Abre [GitHub Releases](https://github.com/AdEx-Partners-DE/OpenNiri-Windows/releases)
2. Si hay un lanzamiento etiquetado disponible, descarga el archivo `.zip` más reciente
3. Extrae `openniri.exe` y `openniri-cli.exe` en una carpeta
4. (Opcional) Agrega la carpeta a tu `PATH`
5. Genera una configuración por defecto:
   ```
   openniri-cli init
   ```
6. Inicia el demonio (*daemon*):
   ```
   openniri-cli run
   ```

### Opción B: Compilar desde Código Fuente (Recomendado para la fase Alfa actual)

Prerrequisitos: [Rust](https://rustup.rs) *GNU toolchain* (`stable-x86_64-pc-windows-gnu`) y enlace MinGW

```bash
rustup toolchain install stable-x86_64-pc-windows-gnu
git clone https://github.com/AdEx-Partners-DE/OpenNiri-Windows.git
cd OpenNiri-Windows
cargo +stable-x86_64-pc-windows-gnu build --release
cargo +stable-x86_64-pc-windows-gnu run -p openniri-cli -- init
cargo +stable-x86_64-pc-windows-gnu run -p openniri-cli -- run
```

El espacio de trabajo está configurado para GNU (`x86_64-pc-windows-gnu`) en `.cargo/config.toml`, y los binarios se colocan en `target/release/`.

## Primeros Pasos Después de la Instalación

| Paso | Comando | Qué Hace |
|------|---------|--------------|
| Crear configuración | `openniri-cli init` | Escribe un `config.toml` por defecto con comentarios |
| Iniciar demonio | `openniri-cli run` | Inicia el administrador de ventanas en segundo plano |
| Verificar estado | `openniri-cli status` | Muestra versión, monitores, ventanas, tiempo activo |
| Alternar pausa | `openniri-cli toggle-pause` (alias: `pause`) | Pausa/reanuda el mosaico sin detener el demonio |
| Ejecutar diagnóstico | `openniri-cli doctor` | Verifica binario, configuración, demonio y estado del sistema |
| Detener demonio | `openniri-cli stop` | Solicita apagado, espera confirmación de limpieza y ejecuta restauración de emergencia local si falla la confirmación |
| Restauración local de emergencia | `openniri-cli emergency-uncloak` (alias: `restore-windows`) | Restauración local de visibilidad de mejor esfuerzo sin IPC del demonio |
| Recargar configuración | `openniri-cli reload` | Aplica cambios de configuración sin reiniciar |

Si el escritorio se vuelve irresponsivo o el enfoque queda atrapado, usa primero la recuperación no destructiva:

```
openniri-cli panic-revert
openniri-cli status   # debe fallar (tiempo de espera O "Daemon is not running...") antes de ejecutar nuevamente
```

Si `panic-revert` no puede confirmar la respuesta del demonio, ahora también ejecuta automáticamente una ruta de restauración de visibilidad local.  
`openniri-cli stop` ahora utiliza la misma red de seguridad de restauración local cuando la confirmación de apagado es ambigua.
También puedes usar `openniri-cli recover` como un alias corto para `panic-revert`.
Si no hay terminal accesible, usa en la bandeja **Emergency: Uncloak All Windows** y luego **Exit**.

Inicia el modo seguro solo después de confirmar el apagado:

```
openniri-cli run --safe-mode
```

Para una guía completa paso a paso, consulta [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md).
Para detalles sobre recuperación de incidentes, consulta [docs/SUPPORT_PLAYBOOK.md](docs/SUPPORT_PLAYBOOK.md).

## Atajos Predeterminados

| Tecla | Acción |
|---|---|
| `Win+H / Win+L` | Enfoque izquierda / derecha |
| `Win+J / Win+K` | Enfoque abajo / arriba |
| `Win+Shift+H / Win+Shift+L` | Mover columna izquierda / derecha |
| `Win+Ctrl+H / Win+Ctrl+L` | Encoger / ampliar columna |
| `Win+Ctrl+Escape` | Restauración de visibilidad de emergencia + revertir pánico del demonio |
| `Win+Alt+H / Win+Alt+L` | Enfoque monitor izquierda / derecha |
| `Win+Alt+Shift+H / Win+Alt+Shift+L` | Mover ventana al monitor izquierda / derecha |
| `Win+Shift+Q` | Cerrar ventana enfocada |
| `Win+F` | Alternar flotante |
| `Win+Shift+F` | Alternar pantalla completa |
| `Win+1 / Win+2 / Win+3` | Establecer ancho a 1/3, 1/2, 2/3 |
| `Win+0` | Igualar todos los anchos de columna |
| `Win+R` | Actualizar (reenumerar ventanas) |

## Iniciar Automáticamente con Windows

```bash
openniri-cli autostart enable
```

Esto escribe una entrada en el Registro bajo `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` que inicia el demonio al iniciar sesión. Para deshabilitarlo:

```bash
openniri-cli autostart disable
```

## Rutas de Configuración y Ejecución

Archivo de configuración:

- `%APPDATA%\openniri\config\config.toml`

Datos de estado:

- `%APPDATA%\openniri\data\workspace-state.json`

Registros del demonio:

- `%TEMP%\openniri-daemon.log`
- `%TEMP%\openniri-daemon.err.log`

## Arquitectura

OpenNiri-Windows es un espacio de trabajo Rust:

| Crate | Responsabilidad |
|---|---|
| `openniri-core-layout` | Motor de diseño independiente de la plataforma |
| `openniri-platform-win32` | Integración Win32 y operaciones de ventanas |
| `openniri-ipc` | Protocolo de comando/respuesta con tubería nombrada (*named-pipe*) |
| `openniri-daemon` | Bucle de eventos en tiempo de ejecución y gestión de estado |
| `openniri-cli` | Interfaz de línea de comandos para el usuario |

Documentación técnica:

- [Getting Started](docs/GETTING_STARTED.md) - Guía paso a paso
- [Configuration](docs/CONFIGURATION.md) - Referencia completa de configuración con ejemplos
- [Troubleshooting](docs/TROUBLESHOOTING.md) - Problemas comunes y guía de depuración
- [Specification](docs/SPEC.md) - Especificación de diseño
- [Architecture](docs/ARCHITECTURE.md) - Descripción general de la arquitectura
- [Windows Constraints](docs/WINDOWS_CONSTRAINTS.md) - Restricciones de la plataforma Win32

## Restricciones de la Plataforma

OpenNiri-Windows es un **controlador de ventanas**, no un compositor.

- DWM permanece como el compositor.
- Las ventanas elevadas o protegidas pueden rechazar cambios de ubicación/estilo.
- El comportamiento puede variar según el marco de aplicaciones (Win32/WPF/Electron/UWP).

## Contribuir

Consulta `CONTRIBUTING.md`.

## Licencia

GPL-3.0. Consulta `LICENSE`.
