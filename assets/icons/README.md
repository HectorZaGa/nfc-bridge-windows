# Iconos de escritorio Windows (Sync, Tools, Windows Tray e Instaladores)

Coloca aquí archivos `.ico` reales (recomendado 16x16, 32x32, 48x48, 256x256 RGBA):

- `sync/app.ico`: Ícono principal de **NFC Bridge Sync** (ejecutable, ventana y Administrador de tareas).
- `sync/tray.ico`: Ícono de **NFC Bridge Sync** en la bandeja del sistema (Windows Tray, recurso PE ID `2`).
- `tools/app.ico`: Ícono principal de **NFC Bridge Tools**.
- `tools/tray.ico`: Ícono de **NFC Bridge Tools** en la bandeja del sistema (recurso PE ID `2`).
- `installer/sync-setup.ico`: Ícono del instalador de Sync.
- `installer/sync-uninstall.ico`: Ícono del desinstalador de Sync (`DisplayIcon`).
- `installer/tools-setup.ico`: Ícono del instalador de Tools.
- `installer/tools-uninstall.ico`: Ícono del desinstalador de Tools (`DisplayIcon`).
- `installer/driver-setup.ico`: Ícono del instalador del Driver.
- `installer/driver-uninstall.ico`: Ícono del desinstalador del Driver (`DisplayIcon`).

Si alguno de estos archivos `.ico` no existe todavía, `build.rs` genera automáticamente un respaldo procedural en compilación.
