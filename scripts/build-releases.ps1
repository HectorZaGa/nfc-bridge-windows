<#
.SYNOPSIS
    Script automatizado de compilación multi-arquitectura y empaquetado para NFC Bridge Windows.
    Genera binarios optimizados para:
    - x64 (x86_64)
    - x86 (32 bits)
    - arm64 (aarch64)
    - Portable (-portable.exe con feature 'portable' embebida)
    - Sumas de verificación SHA-256 (.sha256)
#>

param(
    [string]$Version = "1.1.0",
    [switch]$SkipArm64 = $false,
    [switch]$SkipX86 = $false
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$WindowsRepoDir = Resolve-Path (Join-Path $ScriptDir "..")
$DriverRepoDir = Resolve-Path (Join-Path $WindowsRepoDir "..\nfc-bridge-driver")
$DistDir = Join-Path $WindowsRepoDir "dist"

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "    NFC BRIDGE WINDOWS - BUILD MULTI-ARQUITECTURA v$Version" -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "Repositorio Windows: $WindowsRepoDir"
Write-Host "Repositorio Driver:  $DriverRepoDir"
Write-Host "Directorio de Salida: $DistDir"
Write-Host ""

if (Test-Path $DistDir) {
    Remove-Item -Path $DistDir -Recurse -Force
}
New-Item -ItemType Directory -Path $DistDir -Force | Out-Null

# Función para calcular y guardar hash SHA256
function Generate-Checksum {
    param([string]$FilePath)
    $hash = (Get-FileHash -Path $FilePath -Algorithm SHA256).Hash.ToLower()
    $fileName = Split-Path -Leaf $FilePath
    $checksumContent = "$hash *$fileName`n"
    $checksumFile = "$FilePath.sha256"
    [System.IO.File]::WriteAllText($checksumFile, $checksumContent)
    return "$hash *$fileName"
}

$checksumList = @()

# -------------------------------------------------------------
# 1. Compilación Estándar x64 (Release)
# -------------------------------------------------------------
Write-Host "`n[1/4] Compilando binarios estándar x64..." -ForegroundColor Green
Set-Location $WindowsRepoDir
cargo build --release --bin nfc-bridge-sync --bin nfc-bridge-tools

$syncX64Src = Join-Path $WindowsRepoDir "target\release\nfc-bridge-sync.exe"
$toolsX64Src = Join-Path $WindowsRepoDir "target\release\nfc-bridge-tools.exe"

$syncX64Dst = Join-Path $DistDir "nfc-bridge-sync-$Version-x64.exe"
$toolsX64Dst = Join-Path $DistDir "nfc-bridge-tools-$Version-x64.exe"

Copy-Item $syncX64Src $syncX64Dst -Force
Copy-Item $toolsX64Src $toolsX64Dst -Force
$checksumList += Generate-Checksum $syncX64Dst
$checksumList += Generate-Checksum $toolsX64Dst

Write-Host "  -> Creado: $(Split-Path -Leaf $syncX64Dst) ($( (Get-Item $syncX64Dst).Length / 1MB ) MB)"
Write-Host "  -> Creado: $(Split-Path -Leaf $toolsX64Dst) ($( (Get-Item $toolsX64Dst).Length / 1MB ) MB)"

# Compilar Driver x64
if (Test-Path $DriverRepoDir) {
    Set-Location $DriverRepoDir
    cargo build --release
    $driverX64Src = Join-Path $DriverRepoDir "target\release\nfc-bridge-driver.exe"
    $driverX64Dst = Join-Path $DistDir "nfc-bridge-driver-$Version-x64.exe"
    Copy-Item $driverX64Src $driverX64Dst -Force
    $checksumList += Generate-Checksum $driverX64Dst
    Write-Host "  -> Creado: $(Split-Path -Leaf $driverX64Dst) ($( (Get-Item $driverX64Dst).Length / 1MB ) MB)"
}

# -------------------------------------------------------------
# 2. Compilación Edición PORTABLE (Inmune a renombrado)
# -------------------------------------------------------------
Write-Host "`n[2/4] Compilando versión PORTABLE (-portable.exe)..." -ForegroundColor Green
Set-Location $WindowsRepoDir
cargo build --release --bin nfc-bridge-sync --features portable
cargo build --release --bin nfc-bridge-tools --features portable

$syncPortDst = Join-Path $DistDir "nfc-bridge-sync-$Version-portable.exe"
$toolsPortDst = Join-Path $DistDir "nfc-bridge-tools-$Version-portable.exe"

Copy-Item $syncX64Src $syncPortDst -Force
Copy-Item $toolsX64Src $toolsPortDst -Force
$checksumList += Generate-Checksum $syncPortDst
$checksumList += Generate-Checksum $toolsPortDst

Write-Host "  -> Creado: $(Split-Path -Leaf $syncPortDst)"
Write-Host "  -> Creado: $(Split-Path -Leaf $toolsPortDst)"

# Crear paquete ZIP Portable autosuficiente (Sync + Driver + portable.dat)
$portableZipDir = Join-Path $DistDir "nfc-bridge-portable-$Version"
New-Item -ItemType Directory -Path $portableZipDir -Force | Out-Null

Copy-Item $syncPortDst (Join-Path $portableZipDir "nfc-bridge-sync-portable.exe")
if (Test-Path $driverX64Src) {
    Copy-Item $driverX64Src (Join-Path $portableZipDir "nfc-bridge-driver.exe")
}
[System.IO.File]::WriteAllText((Join-Path $portableZipDir "portable.dat"), "NFC Bridge Portable Container v$Version`n")
[System.IO.File]::WriteAllText((Join-Path $portableZipDir "LEEME.txt"), "NFC Bridge Portable v$Version`n`nEjecuta 'nfc-bridge-sync-portable.exe'. Todo se guarda en esta carpeta.`nEl driver virtual se iniciara automaticamente en segundo plano sin ventana de consola.")

$portableZipPath = Join-Path $DistDir "nfc-bridge-$Version-portable-bundle.zip"
Compress-Archive -Path "$portableZipDir\*" -DestinationPath $portableZipPath -Force
Remove-Item -Path $portableZipDir -Recurse -Force
$checksumList += Generate-Checksum $portableZipPath
Write-Host "  -> Creado Bundle ZIP: $(Split-Path -Leaf $portableZipPath)"

# -------------------------------------------------------------
# 3. Compilación x86 (32 bits) si está disponible el target
# -------------------------------------------------------------
if (-not $SkipX86) {
    Write-Host "`n[3/4] Intentando compilar objetivo x86 (32 bits)..." -ForegroundColor Green
    Set-Location $WindowsRepoDir
    $x86Target = "i686-pc-windows-gnu"
    $x86Check = rustup target list --installed | Select-String "i686"
    if ($x86Check) {
        try {
            cargo build --release --target $x86Target --bin nfc-bridge-sync
            $syncX86Src = Join-Path $WindowsRepoDir "target\$x86Target\release\nfc-bridge-sync.exe"
            if (Test-Path $syncX86Src) {
                $syncX86Dst = Join-Path $DistDir "nfc-bridge-sync-$Version-x86.exe"
                Copy-Item $syncX86Src $syncX86Dst -Force
                $checksumList += Generate-Checksum $syncX86Dst
                Write-Host "  -> Creado: $(Split-Path -Leaf $syncX86Dst)"
            }
        } catch {
            Write-Host "  [Aviso] Target x86 no tiene linker MinGW de 32 bits disponible actualmente. Saltando." -ForegroundColor Yellow
        }
    }
}

# -------------------------------------------------------------
# 4. Compilación ARM64 si está disponible el target y linker
# -------------------------------------------------------------
if (-not $SkipArm64) {
    Write-Host "`n[4/4] Verificando objetivo ARM64 (Surface / Windows en Snapdragon)..." -ForegroundColor Green
    Set-Location $WindowsRepoDir
    $armTarget = "aarch64-pc-windows-msvc"
    $armCheck = rustup target list --installed | Select-String "aarch64"
    if ($armCheck) {
        try {
            cargo build --release --target $armTarget --bin nfc-bridge-sync
            $syncArmSrc = Join-Path $WindowsRepoDir "target\$armTarget\release\nfc-bridge-sync.exe"
            if (Test-Path $syncArmSrc) {
                $syncArmDst = Join-Path $DistDir "nfc-bridge-sync-$Version-arm64.exe"
                Copy-Item $syncArmSrc $syncArmDst -Force
                $checksumList += Generate-Checksum $syncArmDst
                Write-Host "  -> Creado: $(Split-Path -Leaf $syncArmDst)"
            }
        } catch {
            Write-Host "  [Aviso] Cross-compilación ARM64 requiere MSVC C++ ARM64 build tools. Saltando." -ForegroundColor Yellow
        }
    }
}

# Guardar catálogo consolidado de SHA256SUMS.txt
$sha256SumsFile = Join-Path $DistDir "SHA256SUMS.txt"
[System.IO.File]::WriteAllLines($sha256SumsFile, $checksumList)
Write-Host "`nChecksums SHA-256 consolidados guardados en: $(Split-Path -Leaf $sha256SumsFile)" -ForegroundColor Cyan

Write-Host "`n==========================================================" -ForegroundColor Green
Write-Host "  ¡COMPILACIÓN Y EMPAQUETADO FINALIZADOS CON ÉXITO!" -ForegroundColor Green
Write-Host "==========================================================" -ForegroundColor Green
Get-ChildItem $DistDir | Select-Object Name, Length | Format-Table -AutoSize
