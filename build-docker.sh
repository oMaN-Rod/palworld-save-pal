#!/bin/bash

# Build and Run Script for PALWorld Save Pal

# Exit immediately if a command exits with a non-zero status
set -e

# Navigate to the script's directory
cd "$(dirname "$0")"

# Get the IP address
ip_address=$(hostname -I | awk '{print $1}')

# Create or update the .env file with PUBLIC_WS_URL
echo "PUBLIC_WS_URL=${ip_address}:5174/ws" >./ui/.env

# Navigate to the ui directory
cd ./ui

# Remove .svelte-kit directory if it exists
if [ -d ".svelte-kit" ]; then
    echo "Removing .svelte-kit directory..."
    rm -rf .svelte-kit
fi

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Determine which package manager to use
if command_exists bun; then
    package_manager="bun"
elif command_exists npm; then
    package_manager="npm"
elif command_exists yarn; then
    package_manager="yarn"
else
    echo "Error: No suitable package manager found. Please install Bun, npm, or Yarn." >&2
    exit 1
fi

echo "Using $package_manager as the package manager."

# Install dependencies
echo "Installing dependencies..."
$package_manager install

# Build the frontend
echo "Building the frontend..."
$package_manager run build

# Navigate back to the root directory
cd ..

# Run docker-compose up --build -d
echo "Starting Docker Compose..."
docker-compose up --build -d

echo "Build and deployment completed successfully."
