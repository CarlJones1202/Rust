# bulk_submit.ps1
# Reads URLs from sources.txt and submits them to the gallery-dl-api.

$api_url = "http://localhost:3000/api/requests"
$sources_file = "c:\Users\carlj\projects\sources.txt"

if (!(Test-Path $sources_file)) {
    Write-Host "Error: sources.txt not found at $sources_file" -ForegroundColor Red
    exit 1
}

$urls = Get-Content $sources_file | Where-Object { $_.Trim() -ne "" -and $_.StartsWith("http") }

Write-Host "Found $($urls.Count) URLs in sources.txt" -ForegroundColor Cyan

foreach ($url in $urls) {
    $url = $url.Trim()
    Write-Host "Submitting: $url" -NoNewline
    
    $body = @{
        url = $url
    } | ConvertTo-Json
    
    try {
        $response = Invoke-RestMethod -Uri $api_url -Method Post -Body $body -ContentType "application/json" -ErrorAction Stop
        Write-Host " [SUCCESS]" -ForegroundColor Green
    } catch {
        if ($_.Exception.Response.StatusCode -eq "Conflict") {
            Write-Host " [ALREADY EXISTS]" -ForegroundColor Yellow
        } else {
            Write-Host " [FAILED: $($_.Exception.Message)]" -ForegroundColor Red
        }
    }
}

Write-Host "Done!" -ForegroundColor Green
