# Setup Environment Script
# This script installs Python, gallery-dl, VS Build Tools, and Rust.

# Run as Administrator check
if (!([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Host "[WARNING] This script should be run as Administrator to install system packages." -ForegroundColor Yellow
    Write-Host "[INFO] Attempting to restart as Administrator in a new window..." -ForegroundColor Cyan
    Start-Process powershell -ArgumentList "-NoProfile -ExecutionPolicy Bypass -NoExit -File `"$PSCommandPath`"" -Verb RunAs
    Exit
}

function Write-Host-Success {
    param([string]$Message)
    Write-Host "[SUCCESS] $Message" -ForegroundColor Green
}

function Write-Host-Info {
    param([string]$Message)
    Write-Host "[INFO] $Message" -ForegroundColor Cyan
}

function Write-Host-Warning {
    param([string]$Message)
    Write-Host "[WARNING] $Message" -ForegroundColor Yellow
}

# 1. Install Python
Write-Host-Info "Checking for Python..."
if (!(Get-Command python -ErrorAction SilentlyContinue)) {
    Write-Host-Info "Python not found. Installing latest version via winget..."
    winget install -e --id Python.Python.3 --source winget --accept-package-agreements --accept-source-agreements
    
    # Refresh PATH for the current session
    $env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path", "User")
    
    if (!(Get-Command python -ErrorAction SilentlyContinue)) {
        Write-Host-Warning "Python installed but not found in PATH. You may need to restart your terminal."
    } else {
        Write-Host-Success "Python installed successfully."
    }
} else {
    Write-Host-Success "Python is already installed: $(python --version)"
}

# 2. Install gallery-dl and yt-dlp
Write-Host-Info "Installing gallery-dl and yt-dlp via pip..."
python -m pip install -U gallery-dl yt-dlp

if ($LASTEXITCODE -eq 0) {
    Write-Host-Success "gallery-dl and yt-dlp installed successfully."
} else {
    Write-Host-Warning "Failed to install gallery-dl or yt-dlp. Ensure pip is updated."
}

# 3. Install FFmpeg
Write-Host-Info "Checking for FFmpeg..."
if (!(Get-Command ffmpeg -ErrorAction SilentlyContinue)) {
    Write-Host-Info "FFmpeg not found. Installing via winget..."
    winget install -e --id Gyan.FFmpeg --source winget --accept-package-agreements --accept-source-agreements
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host-Success "FFmpeg installed successfully."
    } else {
        Write-Host-Warning "Failed to install FFmpeg via winget."
    }
} else {
    Write-Host-Success "FFmpeg is already installed."
}

# 3b. Add FFmpeg to PATH
Write-Host-Info "Adding FFmpeg to User PATH..."
$wingetPackages = "$env:LOCALAPPDATA\Microsoft\WinGet\Packages"
$ffmpegDir = if (Test-Path $wingetPackages) {
    Get-ChildItem -Path $wingetPackages -Directory -Filter "Gyan.FFmpeg*" -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1
}
if ($ffmpegDir) {
    $ffmpegBin = Get-ChildItem -Path $ffmpegDir.FullName -Directory -Filter "ffmpeg*" -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1
    if ($ffmpegBin) {
        $binPath = Join-Path $ffmpegBin.FullName "bin"
        if (Test-Path $binPath) {
            $currentPath = [System.Environment]::GetEnvironmentVariable("Path", "User")
            if ($currentPath -notlike "*$binPath*") {
                [System.Environment]::SetEnvironmentVariable("Path", "$currentPath;$binPath", "User")
                $env:Path += ";$binPath"
                Write-Host-Success "Added $binPath to User PATH."
            } else {
                Write-Host-Info "FFmpeg bin directory is already in User PATH."
            }
        } else {
            Write-Host-Warning "FFmpeg extracted directory found but no bin subdirectory at $binPath."
        }
    } else {
        Write-Host-Warning "FFmpeg package found but no extracted directory inside."
    }
} else {
    Write-Host-Warning "Could not locate FFmpeg winget package directory."
}

# 4. Add gallery-dl to PATH
Write-Host-Info "Configuring environment variables for gallery-dl..."
$pathsToAdd = @()

# A. Standard Python installation's Scripts directory
if (Get-Command python -ErrorAction SilentlyContinue) {
    $pythonExe = (Get-Command python).Source
    $pythonDir = Split-Path $pythonExe
    $installScriptsPath = Join-Path $pythonDir "Scripts"
    if (Test-Path $installScriptsPath) {
        $pathsToAdd += $installScriptsPath
    }
}

# B. User Site-Packages Scripts directory
$pythonUserBase = python -m site --user-base
if ($pythonUserBase) {
    $userScriptsPath = Join-Path $pythonUserBase "Scripts"
    if (Test-Path $userScriptsPath) {
        $pathsToAdd += $userScriptsPath
    }
}

if ($pathsToAdd.Count -eq 0) {
    Write-Host-Warning "Could not locate any Python Scripts directory."
} else {
    foreach ($scriptsPath in $pathsToAdd) {
        $currentPath = [System.Environment]::GetEnvironmentVariable("Path", "User")
        if ($currentPath -notlike "*$scriptsPath*") {
            Write-Host-Info "Adding $scriptsPath to User PATH..."
            [System.Environment]::SetEnvironmentVariable("Path", "$currentPath;$scriptsPath", "User")
            $env:Path += ";$scriptsPath"
            Write-Host-Success "Added $scriptsPath to User PATH."
        } else {
            Write-Host-Info "$scriptsPath is already in User PATH."
        }
    }
}

# 5. Install Visual Studio Build Tools (C++ Workload)
Write-Host-Info "Checking for Visual Studio Build Tools (C++ Workload)..."
$vswherePath = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$hasMsvc = $false
if (Test-Path $vswherePath) {
    $instances = & $vswherePath -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -format json | ConvertFrom-Json
    if ($instances) {
        $hasMsvc = $true
    }
}

if (!$hasMsvc) {
    Write-Host-Info "Visual Studio C++ Build Tools not found. Installing via winget..."
    winget install -e --id Microsoft.VisualStudio.2022.BuildTools --source winget --accept-package-agreements --accept-source-agreements --override "--passive --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host-Success "Visual Studio Build Tools C++ Workload installed successfully."
    } else {
        Write-Host-Warning "Failed to install Visual Studio Build Tools via winget. You may need to install it manually."
    }
} else {
    Write-Host-Success "Visual Studio C++ Build Tools are already installed."
}

# 6. Install Rust
Write-Host-Info "Checking for Rust..."
if (!(Get-Command rustc -ErrorAction SilentlyContinue)) {
    Write-Host-Info "Rust not found. Downloading rustup-init.exe..."
    $rustupUrl = "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe"
    $rustupPath = Join-Path $env:TEMP "rustup-init.exe"
    
    Invoke-WebRequest -Uri $rustupUrl -OutFile $rustupPath
    
    Write-Host-Info "Running rustup-init.exe (default installation)..."
    Start-Process -FilePath $rustupPath -ArgumentList "-y" -Wait
    
    Write-Host-Success "Rust installation initiated. You may need to restart your terminal to use rustc/cargo."
} else {
    Write-Host-Success "Rust is already installed: $(rustc --version)"
}

Write-Host-Info "Setup complete! Please restart your terminal to ensure all changes take effect."
