function functions_menu {
    Clear-Host
    Write-Host "==== Functions Menu ====" -ForegroundColor Cyan
    Write-Host "1. Run-GBA" -ForegroundColor Cyan
}
try {
    $scriptDir = Join-Path -Path $PSScriptRoot -ChildPath 'Functions'
    Get-ChildItem -Path $scriptDir -Filter '*.ps1' -File | ForEach-Object {
        . $_.FullName
    }
    functions_menu
} catch {
    Write-Host "An error occurred while loading functions." -ForegroundColor Red
    Write-Debug $_.Exception.Message
}