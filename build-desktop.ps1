# Build and Run Script for PALWorld Save Pal

# Navigate to the script's directory
Set-Location -Path $PSScriptRoot

pyinstaller desktop.spec

if ($LASTEXITCODE -ne 0) {
    Write-Error "pyinstaller failed. Exiting."
    exit 1
}

# Remove build directory
if (Test-Path -Path ".\build\") {
    Remove-Item -Path ".\build\" -Recurse -Force
}

if (Test-Path -Path ".\dist\build\") {
    Remove-Item -Path ".\dist\build\" -Recurse -Force
}


# Create or update the .env file with PUBLIC_WS_URL and PUBLIC_DESKTOP_MODE
@"
PUBLIC_WS_URL=127.0.0.1:5174/ws
PUBLIC_DESKTOP_MODE=true
"@ | Set-Content -Path ".\ui\.env"

# Navigate to the ui directory
Set-Location -Path ".\ui"

# Remove .svelte-kit directory if it exists
if (Test-Path -Path ".svelte-kit") {
    Remove-Item -Path ".svelte-kit" -Recurse -Force
}

# Function to check if a command exists
function Test-Command($command) {
    $oldPreference = $ErrorActionPreference
    $ErrorActionPreference = 'stop'
    try {
        if (Get-Command $command) { return $true }
    }
    catch { return $false }
    finally { $ErrorActionPreference = $oldPreference }
}

# Determine which package manager to use
$packageManager = if (Test-Command 'bun') {
    'bun'
}
elseif (Test-Command 'npm') {
    'npm'
}
elseif (Test-Command 'yarn') {
    'yarn'
}
else {
    Write-Error "No suitable package manager found. Please install Bun, npm, or Yarn."
    exit 1
}

Write-Host "Using $packageManager as the package manager."

# Install dependencies
Write-Host "Installing dependencies..."
& $packageManager install

if ($LASTEXITCODE -ne 0) {
    Write-Error "$packageManager install failed. Exiting."
    exit 1
}

# Build the frontend
Write-Host "Building the frontend..."
if ($packageManager -eq 'bun') {
    & $packageManager run build
}
else {
    & $packageManager run build
}

if ($LASTEXITCODE -ne 0) {
    Write-Error "$packageManager run build failed. Exiting."
    exit 1
}

# Navigate back to the root directory
Set-Location -Path ".."

# Copy build to dist
Copy-Item -Path ".\build\" -Destination ".\dist\" -Recurse -Force
Copy-Item -Path ".\data\" -Destination ".\dist\" -Recurse -Force

Write-Host "Done building the desktop app."