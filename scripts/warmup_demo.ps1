# Warm up the local Ollama instance so it's loaded into VRAM before the live demo
Write-Host "Warming up local LLM (qwen2.5-coder:7b-q4_0)..."
$body = @{
    model = "qwen2.5-coder:7b-q4_0"
    prompt = "ping"
    stream = $false
} | ConvertTo-Json

try {
    $response = Invoke-RestMethod -Uri "http://localhost:11434/api/generate" -Method Post -Body $body -ContentType "application/json"
    Write-Host "Warm-up complete! Response: $($response.response.Trim())" -ForegroundColor Green
} catch {
    Write-Host "Failed to warm up Ollama. Is it running?" -ForegroundColor Red
}
