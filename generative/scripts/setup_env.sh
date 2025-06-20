#!/bin/bash
# Bash script to set up the generative AI tool environment
# Run this script from the generative directory

echo "Setting up Generative AI Tool Environment..."

# Check if Python is installed
if ! command -v python3 &> /dev/null; then
    echo "Python3 not found. Please install Python 3.8 or higher."
    exit 1
fi

PYTHON_VERSION=$(python3 --version)
echo "Found Python: $PYTHON_VERSION"

# Create virtual environment
echo "Creating virtual environment..."
if [ -d "venv" ]; then
    echo "Virtual environment already exists. Removing..."
    rm -rf venv
fi

python3 -m venv venv

# Activate virtual environment
echo "Activating virtual environment..."
source venv/bin/activate

# Upgrade pip
echo "Upgrading pip..."
pip install --upgrade pip

# Install requirements
echo "Installing requirements..."
pip install -r requirements.txt

# Create necessary directories
echo "Creating directories..."
directories=("data/examples" "data/rules" "data/outputs" "data/cache" "logs")

for dir in "${directories[@]}"; do
    if [ ! -d "$dir" ]; then
        mkdir -p "$dir"
        echo "Created directory: $dir"
    fi
done

# Copy environment file if it doesn't exist
if [ ! -f ".env" ]; then
    cp env.example .env
    echo "Created .env file from env.example"
    echo "Please edit .env file with your configuration."
fi

echo "Environment setup complete!"
echo "To activate the environment in the future, run: source venv/bin/activate"
echo "To run the tool: python src/auto_learn.py" 