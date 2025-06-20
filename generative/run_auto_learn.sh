#!/bin/bash

# Bash script to run auto-learning with virtual environment

echo "Activating virtual environment..."
source venv/bin/activate

# Set PYTHONPATH to include the current directory
export PYTHONPATH="."

echo "Running auto-learning script..."
python -m src.auto_learn

echo "Press any key to exit..."
read -n 1 