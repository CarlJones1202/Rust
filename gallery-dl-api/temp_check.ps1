$api_base = "http://localhost:3000/api"
$page = 1
$statuses = @{}
$counts = @{}

do {
    $response = Invoke-RestMethod -Uri "$api_base/requests?per_page=200&page=$page"
    foreach ($req in $response.data) {
        if (-not $statuses.ContainsKey($req.status)) {
            $statuses[$req.status] = 0
        }
        $statuses[$req.status]++
        
        # also count ones that look "partial"
        if ($req.status -eq "completed" -and $req.image_count -lt 100) {
            if (-not $counts.ContainsKey("partial")) {
                $counts["partial"] = 0
            }
            $counts["partial"]++
        }
    }
    $page++
} while ($page -le $response.pagination.total_pages)

Write-Host "Statuses:"
$statuses | Format-Table -AutoSize
Write-Host "Partial:"
$counts | Format-Table -AutoSize
