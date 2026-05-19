# nuke_all_and_redownload.ps1
# Script to purge all downloaded media and database references, and trigger a full redownload.

$api_base = "http://localhost:3000/api"
$server_running = $false

# 1. Detect if the API server is currently running
try {
    # Silence errors and set a short timeout to check API responsiveness
    $test = Invoke-RestMethod -Uri "$api_base/requests?per_page=1" -Method Get -TimeoutSec 2 -ErrorAction Stop
    $server_running = $true
} catch {
    $server_running = $false
}

if ($server_running) {
    Write-Host "`n[API Server Detected Running]" -ForegroundColor Cyan
    Write-Host "WARNING: This will nuke all downloaded media files (images, videos, thumbnails, trickplay assets, and temp) and database records, then trigger a full redownload of all requests!" -ForegroundColor Yellow
    
    $confirmation = Read-Host "Are you sure you want to proceed? (y/N)"
    if ($confirmation -ne "y" -and $confirmation -ne "yes") {
        Write-Host "Aborted." -ForegroundColor Red
        exit 0
    }

    Write-Host "Sending nuke command to the API server..." -ForegroundColor Cyan
    try {
        $response = Invoke-RestMethod -Uri "$api_base/requests/nuke" -Method Post -ErrorAction Stop
        Write-Host "`n[SUCCESS] $($response.message)" -ForegroundColor Green
        Write-Host "[SUCCESS] Re-queued $($response.requeued_count) requests for download." -ForegroundColor Green
    } catch {
        Write-Host "Failed to execute nuke command: $($_.Exception.Message)" -ForegroundColor Red
        if ($_.Exception.Response) {
            $stream = $_.Exception.Response.GetResponseStream()
            $reader = New-Object System.IO.StreamReader($stream)
            $errBody = $reader.ReadToEnd()
            Write-Host "Response body: $errBody" -ForegroundColor Red
        }
    }
} else {
    Write-Host "`n[API Server NOT Running]" -ForegroundColor Yellow
    Write-Host "WARNING: This will purge all downloaded media files from disk and reset the database directly!" -ForegroundColor Yellow
    Write-Host "This mode directly edits files and is recommended only when the server is stopped." -ForegroundColor Yellow
    
    $confirmation = Read-Host "Are you sure you want to proceed with direct disk cleanup? (y/N)"
    if ($confirmation -ne "y" -and $confirmation -ne "yes") {
        Write-Host "Aborted." -ForegroundColor Red
        exit 0
    }

    $storage_dir = Join-Path $PSScriptRoot "gallery-dl-api\storage"
    $db_file = Join-Path $PSScriptRoot "gallery-dl-api\gallery_dl.db"

    # A. Delete downloaded media files
    if (Test-Path $storage_dir) {
        Write-Host "Purging downloaded media files from disk..." -ForegroundColor Cyan
        $subdirs = @("images", "videos", "thumbnails", "trickplay", "temp")
        foreach ($subdir in $subdirs) {
            $path = Join-Path $storage_dir $subdir
            if (Test-Path $path) {
                Write-Host "  Clearing files from $path..." -ForegroundColor Gray
                # Delete files/subfolders within the directory, leaving the directory structure intact
                Remove-Item -Path "$path\*" -Recurse -Force -ErrorAction SilentlyContinue
            }
        }
        Write-Host "Disk purge complete." -ForegroundColor Green
    } else {
        Write-Host "Storage directory not found at $storage_dir. Skipping file deletion." -ForegroundColor Yellow
    }

    # B. Reset database using python helper
    if (Test-Path $db_file) {
        Write-Host "Resetting database directly via Python/SQLite..." -ForegroundColor Cyan
        $tempPyFile = [System.IO.Path]::GetTempFileName() + ".py"
        
        $sqlCode = @"
import sqlite3
import sys
import os

db_path = os.path.abspath(r"$db_file")
if not os.path.exists(db_path):
    print(f"Database file not found at: {db_path}")
    sys.exit(1)

try:
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()
    cursor.execute("PRAGMA foreign_keys = ON;")
    
    # Clean tables
    cursor.execute("DELETE FROM galleries;")
    cursor.execute("DELETE FROM videos;")
    
    # Reset all requests to pending status
    cursor.execute("UPDATE requests SET status = 'pending', error_message = None;")
    
    conn.commit()
    conn.close()
    print("Database reset successful.")
except Exception as e:
    print(f"Error resetting database: {e}")
    sys.exit(1)
"@
        Set-Content -Path $tempPyFile -Value $sqlCode
        
        try {
            python $tempPyFile
            if ($LASTEXITCODE -eq 0) {
                Write-Host "Database purge complete." -ForegroundColor Green
                Write-Host "`n[SUCCESS] Nuke complete! Please start the API server to begin redownloading." -ForegroundColor Green
            } else {
                Write-Host "Failed to reset database via Python." -ForegroundColor Red
            }
        } catch {
            Write-Host "Error running python script to reset database: $_" -ForegroundColor Red
        } finally {
            if (Test-Path $tempPyFile) {
                Remove-Item -Path $tempPyFile -Force
            }
        }
    } else {
        Write-Host "Database file not found at $db_file. Skipping database reset." -ForegroundColor Yellow
    }
}
