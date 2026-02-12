#!/bin/bash
# Installation script for LinkedIn Automation

set -e  # Exit on error

echo "========================================"
echo "LinkedIn Automation - Installation"
echo "========================================"
echo ""

# Check Python version
echo "Checking Python version..."
python3 --version || { echo "Error: Python 3 not found"; exit 1; }
echo ""

# Create virtual environment (optional but recommended)
if [ ! -d "venv" ]; then
    echo "Creating virtual environment..."
    python3 -m venv venv
    echo "[OK] Virtual environment created"
    echo ""
fi

# Activate virtual environment
echo "Activating virtual environment..."
source venv/bin/activate
echo "[OK] Virtual environment activated"
echo ""

# Install Python dependencies
echo "Installing Python dependencies..."
pip install --upgrade pip
pip install -r requirements.txt
echo "[OK] Python dependencies installed"
echo ""

# Install Playwright browsers
echo "Installing Playwright browsers..."
playwright install chromium
echo "[OK] Playwright browsers installed"
echo ""

# Setup complete
echo "========================================"
echo "Installation Complete!"
echo "========================================"
echo ""
echo "To get started:"
echo "  1. Activate virtual environment: source venv/bin/activate"
echo "  2. Run capture session: python browser_analysis/main.py capture"
echo ""
echo "See README.md for detailed usage instructions."
