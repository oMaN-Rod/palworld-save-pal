Set-Location -Path (Join-Path $PSScriptRoot "..")

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

$IPAddress = Get-BestIPAddress

Write-Host "Using IP Address: $IPAddress"

docker-compose build --build-arg PUBLIC_WS_URL="${IPAddress}:5174/ws"
docker-compose up -d

Write-Host "Build and deployment completed successfully."