# dev.ps1 - Start both servers for Polymarket Trader
# Run with: .\dev.ps1

$ErrorActionPreference = "Stop"
$ProjectDir = $PSScriptRoot
$BackendDir = Join-Path $ProjectDir "backend"
$FrontendDir = Join-Path $ProjectDir "frontend"

Write-Host "Starting polymarket-trader dev environment..." -ForegroundColor Cyan
Write-Host "============================================" -ForegroundColor Cyan

# Check for .env file
if (-not (Test-Path (Join-Path $ProjectDir ".env"))) {
    Write-Host "Warning: .env file not found. Creating from .env.example..." -ForegroundColor Yellow
    if (Test-Path (Join-Path $ProjectDir ".env.example")) {
        Copy-Item (Join-Path $ProjectDir ".env.example") (Join-Path $ProjectDir ".env")
        Write-Host "Created .env - Please edit and set JWT_SECRET!" -ForegroundColor Yellow
    } else {
        Write-Host "Error: .env.example not found. Please create .env manually." -ForegroundColor Red
        exit 1
    }
}

# Cleanup on exit
$cleanup = {
    Write-Host ""
    Write-Host "Shutting down..." -ForegroundColor Yellow
}
Register-EngineEvent -SourceIdentifier PowerShell.Exiting -Action $cleanup | Out-Null

# Ensure database directory exists
if (-not (Test-Path (Join-Path $BackendDir "data"))) {
    New-Item -ItemType Directory -Path (Join-Path $BackendDir "data") | Out-Null
}

# Start backend in new PowerShell window
Write-Host "Starting backend (port 3001)..." -ForegroundColor Yellow
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd `"$BackendDir`"; cargo run --release"

Start-Sleep -Seconds 3

# Start frontend in new PowerShell window
Write-Host "Starting frontend (port 3000)..." -ForegroundColor Yellow
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd `"$FrontendDir`"; bun run dev"

Write-Host ""
Write-Host "============================================" -ForegroundColor Green
Write-Host "Backend:  http://localhost:3001" -ForegroundColor Green
Write-Host "Frontend: http://localhost:3000" -ForegroundColor Green
Write-Host "============================================" -ForegroundColor Green
Write-Host "Close both PowerShell windows to stop."
