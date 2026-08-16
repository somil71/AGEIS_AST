$ErrorActionPreference = "Stop"

Write-Host "===========================================================" -ForegroundColor Cyan
Write-Host "  SENTINEL AUDITOR - LIVE POLICY CONTRADICTION DEMO" -ForegroundColor Cyan
Write-Host "===========================================================" -ForegroundColor Cyan

# 1. Clean up old index if it exists to ensure a clean demo state
$DemoDir = "D:\AEGIS_AST\demo"
$IndexDir = "$DemoDir\.needle"

if (Test-Path $IndexDir) {
    Write-Host "Cleaning up previous demo state..." -ForegroundColor Yellow
    Remove-Item -Recurse -Force $IndexDir
}

# Change to the project root to run cargo, but tell it to work inside the demo dir
Set-Location "D:\AEGIS_AST"

# Ensure the executable is built
Write-Host "Building Sentinel Auditor (if needed)..." -ForegroundColor Yellow
cargo build --release

$SentinelExe = "D:\AEGIS_AST\target\release\sentinel.exe"
if (-Not (Test-Path $SentinelExe)) {
    # If the workspace was renamed, try needle.exe
    $SentinelExe = "D:\AEGIS_AST\target\release\needle.exe"
}

# 2. Ingest the policy document
Write-Host "`n[1/3] Ingesting Finance Ministry Data Policy..." -ForegroundColor Green
Set-Location $DemoDir
& $SentinelExe policy ingest finance_ministry_data_policy.md --name "Finance Ministry Data Handling Standard"

# 3. Index the codebase
Write-Host "`n[2/3] Indexing Codebase (demo directory)..." -ForegroundColor Green
& $SentinelExe init .

# 4. Run the Compliance Audit
Write-Host "`n[3/3] Running AI Compliance Audit (cross-referencing code vs policy)..." -ForegroundColor Green
& $SentinelExe audit

Write-Host "`n===========================================================" -ForegroundColor Cyan
Write-Host "  DEMO COMPLETE" -ForegroundColor Cyan
Write-Host "===========================================================" -ForegroundColor Cyan
