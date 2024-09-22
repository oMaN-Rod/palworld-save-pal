# Build and Run Script for PALWorld Save Pal

# Navigate to the script's directory
Set-Location -Path $PSScriptRoot

# Function to get the most appropriate IP address
function Get-BestIPAddress {
    $ipAddresses = Get-NetIPAddress -AddressFamily IPv4 | 
    Where-Object { 
        $_.IPAddress -ne '127.0.0.1' -and 
        $_.IPAddress -notlike '169.254.*' -and
        $_.IPAddress -notlike '172.*'
    } |
    Sort-Object -Property { $_.PrefixOrigin -ne 'Manual' }, PrefixLength

    if ($ipAddresses) {
        return $ipAddresses[0].IPAddress
    }
    else {
        Write-Error "No suitable IP address found. Exiting."
        exit 1
    }
}

# Get the IP address
$ip_address = Get-BestIPAddress

# Create or update the .env file with PUBLIC_WS_URL and PUBLIC_DESKTOP_MODE
@"
PUBLIC_WS_URL=${ip_address}:5174/ws
PUBLIC_DESKTOP_MODE=false
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
& $packageManager run build

if ($LASTEXITCODE -ne 0) {
    Write-Error "$packageManager run build failed. Exiting."
    exit 1
}

# Navigate back to the root directory
Set-Location -Path ".."

# Run docker-compose up --build -d
Write-Host "Starting Docker Compose..."
docker-compose up --build -d

if ($LASTEXITCODE -ne 0) {
    Write-Error "docker-compose up failed. Exiting."
    exit 1
}

Write-Host "Build and deployment completed successfully."