# Iconos de escritorio Windows (Sync y Tools)

Cada producto guarda aquí sus 4 archivos `.ico` reales (recomendado incluir 16x16, 32x32, 48x48 y 256x256 RGBA):

- `sync/app.ico`: Ícono principal de **NFC Bridge Sync** (ejecutable, ventana y Administrador de tareas).
- `sync/tray.ico`: Ícono de **NFC Bridge Sync** en la bandeja del sistema (Windows Tray, recurso PE ID `2`).
- `sync/setup.ico`: Ícono del instalador de **NFC Bridge Sync**.
- `sync/uninstall.ico`: Ícono del desinstalador de **NFC Bridge Sync** (`DisplayIcon`).
- `tools/app.ico`: Ícono principal de **NFC Bridge Tools**.
- `tools/tray.ico`: Ícono de **NFC Bridge Tools** en la bandeja del sistema (recurso PE ID `2`).
- `tools/setup.ico`: Ícono del instalador de **NFC Bridge Tools**.
- `tools/uninstall.ico`: Ícono del desinstalador de **NFC Bridge Tools** (`DisplayIcon`).

Los íconos del **Driver** (`app.ico`, `setup.ico`, `uninstall.ico`) viven directamente en `nfc-bridge-driver/assets/icons/`.

Puedes sincronizar y convertir automáticamente desde `E:\Github\NFC Bridge\iconos` ejecutando `sync-icons.ps1`. Si algún `.ico` aún no existe, `build.rs` genera un respaldo procedural en compilación.
