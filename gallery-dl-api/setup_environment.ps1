# Setup Environment Script
# This script installs Python, gallery-dl, and Rust.

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

# 4. Add gallery-dl to PATH
Write-Host-Info "Configuring environment variables for gallery-dl..."
$pythonUserBase = python -m site --user-base
$scriptsPath = Join-Path $pythonUserBase "Scripts"

if (Test-Path $scriptsPath) {
    $currentPath = [System.Environment]::GetEnvironmentVariable("Path", "User")
    if ($currentPath -notlike "*$scriptsPath*") {
        Write-Host-Info "Adding $scriptsPath to User PATH..."
        [System.Environment]::SetEnvironmentVariable("Path", "$currentPath;$scriptsPath", "User")
        $env:Path += ";$scriptsPath"
        Write-Host-Success "Added $scriptsPath to User PATH."
    } else {
        Write-Host-Info "$scriptsPath is already in User PATH."
    }
} else {
    Write-Host-Warning "Could not locate Python Scripts directory at $scriptsPath."
}

# 5. Install Rust
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
