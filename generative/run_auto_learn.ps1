# PowerShell script to run auto-learning with virtual environment

# Activate virtual environment
Write-Host "Activating virtual environment..." -ForegroundColor Green
& ".\venv\Scripts\Activate.ps1"

# Set PYTHONPATH to include the current directory
$env:PYTHONPATH = "."

# Run the auto-learning script
Write-Host "Running auto-learning script..." -ForegroundColor Green
python -m src.auto_learn

# Keep the window open to see results
Write-Host "Press any key to exit..." -ForegroundColor Yellow
$null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown") 