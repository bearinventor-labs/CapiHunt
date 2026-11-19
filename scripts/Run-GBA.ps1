##### START FILE #####

# Name: Get-Develop.ps1
# Description: This script helps set up the development environment for the CoffeeCompany game. It provides options to install developer tools, download templates, and run the game in various modes.

### START SCRIPT ###

function Run-GBA {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $false)]
        [string]$GameName = "CapiHunt"
    )
    <#
    .SYNOPSIS
        Deploys a game to the development environment.
    .PARAMETER GameName
        The name of the game to deploy.
    #>
    # ----------------------------------
    # Game Variables
    # ----------------------------------
    $workDir = "C:\Users\$($env:USERNAME)\Documents\GitHub"
    $downloadDir = "C:\Users\$($env:USERNAME)\Downloads"
    # ----------------------------------
    # Local Variables
    # ----------------------------------
    $gameDir = Join-Path -Path $workDir -ChildPath $gameName
    $scriptDir = Split-Path -Parent $PSScriptRoot

    function main_menu {
        Clear-Host
        Write-Host "==== Get-Develop ====" -ForegroundColor Cyan
        Write-Host "1. Install Developer Tools" -ForegroundColor Cyan
        Write-Host "2. Download 'The agb book' Developer Template" -ForegroundColor Cyan
        Write-Host "3. Run your game in Debug Mode (if available)" -ForegroundColor Yellow
        Write-Host "4. Run your game in Cheat Mode (if available)" -ForegroundColor Red
        Write-Host "5. Run your game in Release Mode" -ForegroundColor Green
        Write-Host "6. Compile your game for GBA Hardware" -ForegroundColor Magenta
        Write-Host "C. Clean Cargo" -ForegroundColor Cyan
        Write-Host "Q. Quit" -ForegroundColor Cyan
    }
    function tools_menu {
        Clear-Host
        Write-Host "==== Developer Tools ====" -ForegroundColor Cyan
        Write-Host "A. Install All Tools" -ForegroundColor Green
        Write-Host "---- Git" -ForegroundColor Cyan
        Write-Host "---- Rust" -ForegroundColor Cyan
        Write-Host "---- Rustup" -ForegroundColor Cyan
        Write-Host "---- mGBA" -ForegroundColor Cyan
        Write-Host "B. Back to Main Menu" -ForegroundColor Cyan
    }
    while ($true) {
        main_menu
        $decision = Read-Host -Prompt "(Default - 5)"
        switch (Clean-Prompt -Prompt $decision) {
            # ----------------------------------
            # Install Developer Tools
            # ----------------------------------
            "1" {
            tools_menu
            $toolDecision = Read-Host -Prompt "(Default - A)"
            
            if (Clean-Prompt -Prompt $toolDecision -eq "B") {
                continue
            } else {
                Write-Host "Checking for Developer Tools..." -ForegroundColor Cyan
                # ----------------------------------
                # Try to Find Git
                # ----------------------------------
                try { 
                    if (Get-Command -Name "git" -ErrorAction SilentlyContinue) {
                        $gitVersion = (git --version).Trim()
                        Write-Host "$gitVersion" -ForegroundColor Green
                    } else {
                        Write-Host "Git is not installed." -ForegroundColor Yellow
                        Write-Host "Would you like to install Git? (This will open a browser to download the GUI installer)" -ForegroundColor Cyan
                        $cmd = Read-Host -Prompt "(Y/n)"
                        if (Clean-Prompt -Prompt $cmd -eq "Y") {
                            Start-Process "https://github.com/git-guides/install-git"
                        } else {
                            Write-Host "Git installation canceled." -ForegroundColor Yellow
                        }
                    }
                } catch {
                    Write-Host "An error occurred while checking for Git." -ForegroundColor Red
                    Write-Debug $_.Exception.Message
                }
                # ----------------------------------
                # Try to Find Rust
                # ----------------------------------
                try {
                    if (Get-Command -Name "rustc" -ErrorAction SilentlyContinue) {
                        $rustVersion = (rustc --version).Trim()
                        Write-Host "$rustVersion" -ForegroundColor Green
                    } else {
                        Write-Host "Rust is not installed." -ForegroundColor Yellow
                        Write-Host "Would you like to install Rust? (This will open a browser to download the installer)" -ForegroundColor Cyan
                        $cmd = Read-Host -Prompt "(Y/n)"
                        if (Clean-Prompt -Prompt $cmd -eq "Y") {
                            Start-Process "https://www.rust-lang.org/tools/install"
                        } else {
                            Write-Host "Rust installation canceled." -ForegroundColor Yellow
                        }
                    }
                } catch {
                    Write-Host "An error occurred while checking for Rust." -ForegroundColor Red
                    Write-Debug $_.Exception.Message
                }
                # ----------------------------------
                # Try to Find Rustup
                # ----------------------------------
                try {
                    if (Get-Command -Name "rustup" -ErrorAction SilentlyContinue) {
                        $rustupVersion = (rustup --version).Trim()
                        Write-Host "$rustupVersion" -ForegroundColor Green
                    } else {
                        Write-Host "Rustup is not installed." -ForegroundColor Yellow
                        Write-Host "Would you like to install Rustup? (This will open a browser to download the installer)" -ForegroundColor Cyan
                        $cmd = Read-Host -Prompt "(Y/n)"
                        if (Clean-Prompt -Prompt $cmd -eq "Y") {
                            Start-Process "https://rustup.rs/"
                        } else {
                            Write-Host "Rustup installation canceled." -ForegroundColor Yellow
                        }
                    }
                } catch {
                    Write-Host "An error occurred while checking for Rustup." -ForegroundColor Red
                    Write-Debug $_.Exception.Message
                }
                # ----------------------------------
                # Try to Find mGBA
                # ----------------------------------
                try {
                    if (Get-Command -Name "mgba-qt" -ErrorAction SilentlyContinue) {
                        $mgbaVersion = (mgba-qt --version).Trim()
                        Write-Host "$mgbaVersion" -ForegroundColor Green
                    } else {
                        Write-Host "mGBA is not installed." -ForegroundColor Yellow
                        Write-Host "Would you like to install mGBA? (This will open a browser to download the installer)" -ForegroundColor Cyan
                        $cmd = Read-Host -Prompt "(Y/n)"
                        if (Clean-Prompt -Prompt $cmd -eq "Y") {
                            Start-Process "https://mgba.io/"
                        } else {
                            Write-Host "mGBA installation canceled." -ForegroundColor Yellow
                        }
                    }
                } catch {
                    Write-Host "An error occurred while checking for mGBA." -ForegroundColor Red
                    Write-Debug $_.Exception.Message
                }
                # ----------------------------------
                # Try to Find agb-gbafix
                # ----------------------------------
                try {
                    if (Get-Command -Name "agb-gbafix" -ErrorAction SilentlyContinue) {
                        $agbGbafixVersion = (agb-gbafix --version).Trim()
                        Write-Host "$agbGbafixVersion" -ForegroundColor Green
                    } else {
                        Write-Host "agb-gbafix is not installed." -ForegroundColor Yellow
                        Write-Host "Would you like to install agb-gbafix? (This will use a cargo command to install)" -ForegroundColor Cyan
                        $cmd = Read-Host -Prompt "(Y/n)"
                        if (Clean-Prompt -Prompt $cmd -eq "Y") {
                            cargo install agb-gbafix
                        } else {
                            Write-Host "agb-gbafix installation canceled." -ForegroundColor Yellow
                        }
                    }
                } catch {
                    Write-Host "An error occurred while checking for agb-gbafix." -ForegroundColor Red
                    Write-Debug $_.Exception.Message
                }
                # ----------------------------------
                # Create Link for agb-gbafix
                # ----------------------------------
                try {
                    Write-Host "Creating Link for agb-gbafix..." -ForegroundColor Cyan
                    New-Item -itemtype hardlink -path "C:\Program Files\mGBA\agb-gbafix.exe" -value "C:\Program Files\mGBA\agb-gbafix.exe"
                    Write-Host "Link created successfully." -ForegroundColor Green
                } catch {
                    Write-Host "An error occurred while creating the link for agb-gbafix." -ForegroundColor Red
                    Write-Debug $_.Exception.Message
                }
                # # # # #
                # PAUSE #
                # # # # # 
                Write-Host "All Developer Tools Checked and/or Installed." -ForegroundColor Green
                Read-Host -Prompt ": Press Enter to continue "
            }
            }
            # ----------------------------------
            # Download 'The agb book' Developer Template
            # ----------------------------------
            "2" {
            Write-Host "Getting Developer Template from 'The agb book'..." -ForegroundColor Cyan
            # ----------------------------------
            # Set Download Path
            # ----------------------------------
            try {
                $downloadPath = Read-Host -Prompt "Enter the download path (Default: $downloadDir)"
                if ([string]::IsNullOrWhiteSpace($downloadPath)) {
                    $downloadPath = $downloadDir
                }
                $downloadFile = Join-Path -Path $downloadPath -ChildPath "template"
            } catch {
                Write-Host "An error occurred while setting the download path." -ForegroundColor Red
                Write-Debug $_.Exception.Message
            }
            # ----------------------------------
            # Create Download Path 
            # ----------------------------------
            try {
                if (-not (Test-Path $downloadPath)) {
                    New-Item -ItemType Directory -Path $downloadPath -Force
                }
            } catch {
                Write-Host "An error occurred while creating the download path." -ForegroundColor Red
                Write-Debug $_.Exception.Message
            }
            # ----------------------------------
            # Download Developer Template
            # ----------------------------------
            try {
                Write-Host "Downloading Developer Template to $downloadFile..." -ForegroundColor Cyan
                git clone https://github.com/agbrs/template.git "$downloadFile"
                Write-Host "Developer Template downloaded successfully." -ForegroundColor Green
            } catch {
                Write-Host "An error occurred while downloading the Developer Template." -ForegroundColor Red
                Write-Debug $_.Exception.Message
            }
            # ----------------------------------
            # Rename Downloaded Template Folder
            # ----------------------------------
            try {
                $newFolderName = Read-Host -Prompt "Enter the new folder name for the Developer Template (Default: $gameName)"
                if ([string]::IsNullOrWhiteSpace($newFolderName)) {
                    $newFolderName = $gameName
                }
                if (Test-Path $newFolderName) {
                    Write-Host "Folder with the name $newFolderName already exists." -ForegroundColor Red
                } else {
                    Rename-Item -Path $downloadFile -NewName $newFolderName
                    Write-Host "Folder renamed successfully to $newFolderName." -ForegroundColor Green
                }
            } catch {
                Write-Host "An error occurred while renaming the Developer Template folder." -ForegroundColor Red
                Write-Debug $_.Exception.Message
            }
            # ----------------------------------
            # Restructure Downloaded Template Folder
            # ----------------------------------
            try {
                $gitFolder = Join-Path -Path $newFolderName -ChildPath ".git"
                $readmeFile = Join-Path -Path $newFolderName -ChildPath "README.md"
                if (Test-Path $gitFolder) {
                    Remove-Item -Path $gitFolder -Recurse -Force
                    Write-Host "Removed .git folder." -ForegroundColor Green
                }
                if (Test-Path $readmeFile) {
                    Rename-Item -Path $readmeFile -NewName "AGB_README.md"
                    Write-Host "Renamed README.md to AGB_README.md." -ForegroundColor Green
                }
            } catch {
                Write-Host "An error occurred while restructuring the Developer Template folder." -ForegroundColor Red
                Write-Debug $_.Exception.Message
            }
            # # # # #
            # PAUSE #
            # # # # # 
            Write-Host "Template downloaded and renamed successfully." -ForegroundColor Green
            Read-Host -Prompt ": Press Enter to continue "
            }
            # ----------------------------------
            # Run your game in Debug Mode (if available)
            # ----------------------------------
            "3" {

            }
            # ----------------------------------
            # Run your game in Cheat Mode (if available)
            # ----------------------------------
            "4" {

            }
            # ----------------------------------
            # Run your game in Release Mode
            # ----------------------------------
            "5" {
            # ----------------------------------
            # Push Location to Working Directory
            # ----------------------------------
            Push-Location -Path $newFolderName
            # ----------------------------------
            # Run the game in Release Mode
            # ----------------------------------
            try {
                cargo build --release
                Write-Host "Game compiled successfully in Release Mode." -ForegroundColor Green
                try {
                    cargo run --release
                    Write-Host "Game ran successfully in Release Mode." -ForegroundColor Green
                } catch {
                    Write-Host "An error occurred while running the game in Release Mode." -ForegroundColor Red
                    Write-Debug $_.Exception.Message    
                }
            } catch {
                Write-Host "An error occurred while compiling the game in Release Mode." -ForegroundColor Red
                Write-Debug $_.Exception.Message
            }
            Read-Host -Prompt ": Press Enter to continue "
            # ----------------------------------
            # Pop Location back to original directory
            # ----------------------------------
            Pop-Location
            }
            # ----------------------------------
            # Compile your game for GBA Hardware
            # ----------------------------------
            "6" {

            }
            # ----------------------------------
            # Clean Cargo
            # ----------------------------------
            "C" {
            Write-Host "Would you like to clean the cargo build? (This will dramatically increase build times)" -ForegroundColor Cyan
            $cmd = Read-Host -Prompt "(Y/n)"
            if ($cmd -eq "Y") {
                cargo clean
            } else {
                Write-Host "Cargo clean canceled." -ForegroundColor Yellow
            }
            }
            "Q" { 
            exit 0
            }
            default {
            Write-Host "Invalid selection." -ForegroundColor Red
            Read-Host -Prompt ": Press Enter to continue"
            main_menu
            }
        }
    }
}

### END SCRIPT ###

##### END FILE #####