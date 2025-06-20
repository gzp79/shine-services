# PowerShell script to set up the generative AI tool environment
# Run this script from the generative directory

Write-Host "Setting up Generative AI Tool Environment..." -ForegroundColor Green

# Check if Python is installed
try {
    $pythonVersion = python --version
    Write-Host "Found Python: $pythonVersion" -ForegroundColor Green
} catch {
    Write-Host "Python not found. Please install Python 3.8 or higher." -ForegroundColor Red
    exit 1
}

# Create virtual environment
Write-Host "Creating virtual environment..." -ForegroundColor Yellow
if (Test-Path "venv") {
    Write-Host "Virtual environment already exists. Removing..." -ForegroundColor Yellow
    Remove-Item -Recurse -Force "venv"
}

python -m venv venv

# Activate virtual environment
Write-Host "Activating virtual environment..." -ForegroundColor Yellow
& "venv\Scripts\Activate.ps1"

# Upgrade pip
Write-Host "Upgrading pip..." -ForegroundColor Yellow
python -m pip install --upgrade pip

# Install requirements
Write-Host "Installing requirements..." -ForegroundColor Yellow
pip install -r requirements.txt

# Create necessary directories
Write-Host "Creating directories..." -ForegroundColor Yellow
$directories = @(
    "data/examples",
    "data/rules", 
    "data/outputs",
    "data/cache",
    "logs"
)

foreach ($dir in $directories) {
    if (!(Test-Path $dir)) {
        New-Item -ItemType Directory -Path $dir -Force
        Write-Host "Created directory: $dir" -ForegroundColor Green
    }
}

# Copy environment file if it doesn't exist
if (!(Test-Path ".env")) {
    Copy-Item "env.example" ".env"
    Write-Host "Created .env file from env.example" -ForegroundColor Green
    Write-Host "Please edit .env file with your configuration." -ForegroundColor Yellow
}

Write-Host "Environment setup complete!" -ForegroundColor Green
Write-Host "To activate the environment in the future, run: venv\Scripts\Activate.ps1" -ForegroundColor Cyan
Write-Host "To run the tool: python src/auto_learn.py" -ForegroundColor Cyan 