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
$IPAddress = Get-BestIPAddress

Write-Host "Using IP Address: $IPAddress"

# Build and run docker compose
docker-compose build --build-arg PUBLIC_WS_URL="${IPAddress}:5174/ws"
docker-compose up -d

Write-Host "Build and deployment completed successfully."