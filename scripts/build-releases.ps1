<#
.SYNOPSIS
    Script automatizado de compilación multi-arquitectura para NFC Bridge Windows (Sync & Tools).
    Genera binarios optimizados para:
    - x64 (x86_64)
    - x86 (32 bits)
    - arm64 (aarch64)
    - Portable (-portable.exe con feature 'portable' embebida, inmune a renombrado)
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
$DistDir = Join-Path $WindowsRepoDir "dist"

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "    NFC BRIDGE WINDOWS - BUILD MULTI-ARQUITECTURA v$Version" -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "Repositorio Windows: $WindowsRepoDir"
Write-Host "Directorio de Salida: $DistDir"
Write-Host ""

if (Test-Path $DistDir) {
    Remove-Item -Path $DistDir -Recurse -Force
}
New-Item -ItemType Directory -Path $DistDir -Force | Out-Null

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

Write-Host "  -> Creado: $(Split-Path -Leaf $syncX64Dst) ($( [math]::Round((Get-Item $syncX64Dst).Length / 1MB, 2) ) MB)"
Write-Host "  -> Creado: $(Split-Path -Leaf $toolsX64Dst) ($( [math]::Round((Get-Item $toolsX64Dst).Length / 1MB, 2) ) MB)"

# -------------------------------------------------------------
# 2. Compilación Edición PORTABLE (-portable.exe, Inmune a renombrado)
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

Write-Host "  -> Creado: $(Split-Path -Leaf $syncPortDst) ($( [math]::Round((Get-Item $syncPortDst).Length / 1MB, 2) ) MB)"
Write-Host "  -> Creado: $(Split-Path -Leaf $toolsPortDst) ($( [math]::Round((Get-Item $toolsPortDst).Length / 1MB, 2) ) MB)"

# -------------------------------------------------------------
# 3. Compilación x86 (32 bits)
# -------------------------------------------------------------
if (-not $SkipX86) {
    Write-Host "`n[3/4] Intentando compilar objetivo x86 (32 bits)..." -ForegroundColor Green
    Set-Location $WindowsRepoDir
    $x86Candidates = @("i686-pc-windows-msvc", "i686-pc-windows-gnu")
    $installedTargets = rustup target list --installed
    $x86Built = $false

    foreach ($t in $x86Candidates) {
        if ($installedTargets -contains $t) {
            try {
                cargo build --release --target $t --bin nfc-bridge-sync --bin nfc-bridge-tools
                $syncSrc = Join-Path $WindowsRepoDir "target\$t\release\nfc-bridge-sync.exe"
                $toolsSrc = Join-Path $WindowsRepoDir "target\$t\release\nfc-bridge-tools.exe"
                if ((Test-Path $syncSrc) -and (Test-Path $toolsSrc)) {
                    $syncDst = Join-Path $DistDir "nfc-bridge-sync-$Version-x86.exe"
                    $toolsDst = Join-Path $DistDir "nfc-bridge-tools-$Version-x86.exe"
                    Copy-Item $syncSrc $syncDst -Force
                    Copy-Item $toolsSrc $toolsDst -Force
                    $checksumList += Generate-Checksum $syncDst
                    $checksumList += Generate-Checksum $toolsDst
                    Write-Host "  -> Creado ($t): $(Split-Path -Leaf $syncDst)"
                    Write-Host "  -> Creado ($t): $(Split-Path -Leaf $toolsDst)"
                    $x86Built = $true
                    break
                }
            } catch {
                Write-Host "  [Aviso] Falló compilación x86 con $t (faltan librerías C/C++ x86 en el sistema)." -ForegroundColor Yellow
            }
        }
    }
    if (-not $x86Built) {
        Write-Host "  [Info] Para compilar x86 localmente instala el componente C++ x86 en Visual Studio Installer o usa el workflow de GitHub Actions." -ForegroundColor Yellow
    }
}

# -------------------------------------------------------------
# 4. Compilación ARM64 (Windows en Snapdragon / Surface ARM)
# -------------------------------------------------------------
if (-not $SkipArm64) {
    Write-Host "`n[4/4] Intentando compilar objetivo ARM64 (aarch64)..." -ForegroundColor Green
    Set-Location $WindowsRepoDir
    $armTarget = "aarch64-pc-windows-msvc"
    $installedTargets = rustup target list --installed
    $armBuilt = $false

    if ($installedTargets -contains $armTarget) {
        try {
            cargo build --release --target $armTarget --bin nfc-bridge-sync --bin nfc-bridge-tools
            $syncSrc = Join-Path $WindowsRepoDir "target\$armTarget\release\nfc-bridge-sync.exe"
            $toolsSrc = Join-Path $WindowsRepoDir "target\$armTarget\release\nfc-bridge-tools.exe"
            if ((Test-Path $syncSrc) -and (Test-Path $toolsSrc)) {
                $syncDst = Join-Path $DistDir "nfc-bridge-sync-$Version-arm64.exe"
                $toolsDst = Join-Path $DistDir "nfc-bridge-tools-$Version-arm64.exe"
                Copy-Item $syncSrc $syncDst -Force
                Copy-Item $toolsSrc $toolsDst -Force
                $checksumList += Generate-Checksum $syncDst
                $checksumList += Generate-Checksum $toolsDst
                Write-Host "  -> Creado: $(Split-Path -Leaf $syncDst)"
                Write-Host "  -> Creado: $(Split-Path -Leaf $toolsDst)"
                $armBuilt = $true
            }
        } catch {
            Write-Host "  [Aviso] Falló compilación ARM64 (faltan librerías C++ ARM64 en Visual Studio Installer)." -ForegroundColor Yellow
        }
    }
    if (-not $armBuilt) {
        Write-Host "  [Info] Para compilar ARM64 localmente instala 'MSVC C++ ARM64 build tools' en Visual Studio Installer o usa el workflow de GitHub Actions." -ForegroundColor Yellow
    }
}

# Guardar catálogo consolidado de SHA256SUMS.txt
$sha256SumsFile = Join-Path $DistDir "SHA256SUMS.txt"
[System.IO.File]::WriteAllLines($sha256SumsFile, $checksumList)
Write-Host "`nChecksums SHA-256 consolidados guardados en: $(Split-Path -Leaf $sha256SumsFile)" -ForegroundColor Cyan

Write-Host "`n==========================================================" -ForegroundColor Green
Write-Host "  ¡COMPILACIÓN DE NFC BRIDGE WINDOWS FINALIZADA!" -ForegroundColor Green
Write-Host "==========================================================" -ForegroundColor Green
Get-ChildItem $DistDir | Select-Object Name, Length | Format-Table -AutoSize
