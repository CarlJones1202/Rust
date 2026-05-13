# check_partial_downloads.ps1
# Checks for 'completed' downloads with less than 100 images and re-queues them.
# Now handles pagination to scan the entire database.

$api_base = "http://localhost:3000/api"
$min_images = 100

Write-Host "Checking for completed downloads with < $min_images images..." -ForegroundColor Cyan

$page = 1
$partial_count = 0
$total_checked = 0

do {
    try {
        $response = Invoke-RestMethod -Uri "$api_base/requests?per_page=200&page=$page" -Method Get -ErrorAction Stop
        $requests = $response.data
        $total_pages = $response.pagination.total_pages
        $total_checked += $requests.Count
        
        Write-Host "Processing page $page of $total_pages..." -ForegroundColor Gray
        
        foreach ($req in $requests) {
            # We only re-queue if it's completed, has few images, AND has no videos
            if ($req.status -eq "completed" -and $req.image_count -lt $min_images -and $req.video_count -eq 0) {
                Write-Host "Partial download found: $($req.url) (Images: $($req.image_count))" -ForegroundColor Yellow
                
                # Re-queue the request
                try {
                    Invoke-RestMethod -Uri "$api_base/requests/$($req.id)/requeue" -Method Post -ErrorAction Stop
                    Write-Host "  Successfully re-queued." -ForegroundColor Green
                    $partial_count++
                } catch {
                    Write-Host "  Failed to re-queue request $($req.id): $($_.Exception.Message)" -ForegroundColor Red
                }
            }
        }
        
        $page++
    } catch {
        Write-Host "Failed to fetch requests on page $page - $($_.Exception.Message)" -ForegroundColor Red
        break
    }
} while ($page -le $total_pages)

Write-Host "`nDone!" -ForegroundColor Green
Write-Host "Total checked: $total_checked" -ForegroundColor Cyan
Write-Host "Total re-queued: $partial_count" -ForegroundColor Cyan
