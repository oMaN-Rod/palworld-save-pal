# Bump version script for palworld_save_pal
param(
    [string]$Version = $null,
    [switch]$Help
)

if ($Help) {
    Write-Host "Usage: .\build-desktop.ps1 [-Version <version>] [-Help]"
    Write-Host ""
    Write-Host "Parameters:"
    Write-Host "  -Version <version>  Set the version number (e.g., '1.0.0')"
    Write-Host "  -Help              Show this help message"
    Write-Host ""
    exit 0
}

Set-Location -Path $PSScriptRoot

# Function to update version in a file
function Update-Version {
    param(
        [string]$FilePath,
        [string]$NewVersion,
        [string]$Pattern,
        [string]$Replacement
    )
    
    if (Test-Path $FilePath) {
        $content = Get-Content -Path $FilePath -Raw
        $newContent = $content -replace $Pattern, $Replacement
        Set-Content -Path $FilePath -Value $newContent -NoNewline
        Write-Host "Updated version to $NewVersion in $FilePath"
    }
    else {
        Write-Warning "File not found: $FilePath"
    }
}

# Get or set version
if ($Version) {
    # Validate version format (basic semver check)
    if ($Version -notmatch '^\d+\.\d+\.\d+(-[a-zA-Z0-9\-\.]+)?(\+[a-zA-Z0-9\-\.]+)?$') {
        Write-Error "Invalid version format. Please use semantic versioning (e.g., '1.0.0', '1.0.0-beta', '1.0.0+build.1')"
        exit 1
    }
    
    Write-Host "Updating version to $Version..."
    
    # Update __version__.py
    Update-Version -FilePath ".\palworld_save_pal\__version__.py" -NewVersion $Version -Pattern '__version__ = "[^"]*"' -Replacement "__version__ = `"$Version`""
    
    # Update pyproject.toml
    Update-Version -FilePath ".\pyproject.toml" -NewVersion $Version -Pattern 'version = "[^"]*"' -Replacement "version = `"$Version`""
    
    $version = $Version
}
else {
    # Read current version
    $version = (Get-Content -Path ".\palworld_save_pal\__version__.py" | Select-String -Pattern "__version__").Line.Split('"')[1]
}