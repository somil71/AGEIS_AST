$ErrorActionPreference = "Stop"

Write-Host "===========================================================" -ForegroundColor Cyan
Write-Host "  LATENCY BENCHMARK" -ForegroundColor Cyan
Write-Host "===========================================================" -ForegroundColor Cyan

$Queries = @(
    "authentication middleware",
    "HNSW index initialization",
    "chunking algorithm python",
    "pdf text extraction logic",
    "how to handle legacy COBOL files",
    "what does find_callers do",
    "graph community detection",
    "MCP tool dispatch routine",
    "API routing and endpoints",
    "database schema definition"
)

$SentinelExe = ".\target\release\sentinel.exe"
if (-Not (Test-Path $SentinelExe)) {
    $SentinelExe = ".\target\release\needle.exe"
}

$TotalTime = 0
$BM25Time = 0
$HNSWTime = 0
$EmbedTime = 0
$FuseTime = 0
$Count = 0

Write-Host "Running 50 queries against the index..." -ForegroundColor Yellow

for ($i = 0; $i -lt 5; $i++) {
    foreach ($q in $Queries) {
        $Output = & $SentinelExe search $q
        # Look for the timing line: ... (BM25: 1.2ms  HNSW: 3.4ms  embed: 4.5ms  fuse: 0.1ms)
        foreach ($line in $Output) {
            if ($line -match "BM25: ([\d.]+)ms\s+HNSW: ([\d.]+)ms\s+embed: ([\d.]+)ms\s+fuse: ([\d.]+)ms") {
                $BM25Time += [double]$matches[1]
                $HNSWTime += [double]$matches[2]
                $EmbedTime += [double]$matches[3]
                $FuseTime += [double]$matches[4]
                $Count++
            }
        }
    }
}

if ($Count -eq 0) {
    Write-Host "Failed to parse timings. Check your index or executable." -ForegroundColor Red
    exit 1
}

$AvgBM25 = [math]::Round($BM25Time / $Count, 2)
$AvgHNSW = [math]::Round($HNSWTime / $Count, 2)
$AvgEmbed = [math]::Round($EmbedTime / $Count, 2)
$AvgFuse = [math]::Round($FuseTime / $Count, 2)
$AvgTotal = $AvgBM25 + $AvgHNSW + $AvgEmbed + $AvgFuse

Write-Host "`nAverage Latency over $Count queries:" -ForegroundColor Green
Write-Host "BM25   : ${AvgBM25}ms"
Write-Host "HNSW   : ${AvgHNSW}ms"
Write-Host "Embed  : ${AvgEmbed}ms"
Write-Host "Fuse   : ${AvgFuse}ms"
Write-Host "-------------------"
Write-Host "Total  : ${AvgTotal}ms"
