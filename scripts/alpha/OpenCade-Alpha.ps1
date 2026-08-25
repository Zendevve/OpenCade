[CmdletBinding()]
param(
    [ValidateSet("Package", "Verify", "Doctor", "Launch")]
    [string]$Mode = "Doctor",
    [string]$KitRoot = ".",
    [string]$ApiUrl,
    [string]$RetroArchRoot,
    [string]$StunServer
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$requiredKitFiles = @(
    "opencade-client.exe",
    "opencade-match-probe.exe",
    "opencade-match-verify.exe",
    "opencade-alpha-summary.exe",
    "OpenCade-Alpha.ps1",
    "README.txt",
    "RETROARCH_TEST.md",
    "MATCH_REPORT_TEMPLATE.md"
)

function Resolve-KitRoot {
    param([string]$Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        New-Item -ItemType Directory -Path $Path -Force | Out-Null
    }
    return (Resolve-Path -LiteralPath $Path).Path
}

function Write-Checksums {
    param([string]$Root)

    $lines = Get-ChildItem -LiteralPath $Root -File |
        Where-Object { $_.Name -ne "SHA256SUMS.txt" } |
        Sort-Object Name |
        ForEach-Object {
            $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $_.FullName).Hash.ToLowerInvariant()
            "$hash *$($_.Name)"
        }
    $lines | Set-Content -LiteralPath (Join-Path $Root "SHA256SUMS.txt") -Encoding ascii
}

function Package-Kit {
    param([string]$Root)

    $repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
    $sources = @{
        "opencade-client.exe" = "apps\client\src-tauri\target\release\opencade-client.exe"
        "opencade-match-probe.exe" = "target\release\opencade-match-probe.exe"
        "opencade-match-verify.exe" = "target\release\opencade-match-verify.exe"
        "opencade-alpha-summary.exe" = "target\release\opencade-alpha-summary.exe"
        "OpenCade-Alpha.ps1" = "scripts\alpha\OpenCade-Alpha.ps1"
        "README.txt" = "scripts\alpha\README.txt"
        "RETROARCH_TEST.md" = "docs\alpha\RETROARCH_TEST.md"
        "MATCH_REPORT_TEMPLATE.md" = "docs\alpha\MATCH_REPORT_TEMPLATE.md"
    }

    foreach ($destination in $sources.Keys) {
        $source = Join-Path $repositoryRoot $sources[$destination]
        if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
            throw "Required package input is missing: $source"
        }
        Copy-Item -LiteralPath $source -Destination (Join-Path $Root $destination) -Force
    }

    Write-Checksums -Root $Root
    Write-Host "Packaged OpenCade alpha kit at $Root"
}

function Verify-Kit {
    param([string]$Root)

    foreach ($name in $requiredKitFiles) {
        if (-not (Test-Path -LiteralPath (Join-Path $Root $name) -PathType Leaf)) {
            throw "Alpha kit is incomplete: $name is missing"
        }
    }

    $checksumPath = Join-Path $Root "SHA256SUMS.txt"
    if (-not (Test-Path -LiteralPath $checksumPath -PathType Leaf)) {
        throw "Alpha kit is incomplete: SHA256SUMS.txt is missing"
    }

    $verifiedNames = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($line in Get-Content -LiteralPath $checksumPath) {
        if ($line -notmatch '^([0-9a-fA-F]{64}) \*([^\\/]+)$') {
            throw "Invalid checksum entry: $line"
        }
        $expected = $Matches[1].ToLowerInvariant()
        $name = $Matches[2]
        if (-not $verifiedNames.Add($name)) {
            throw "Duplicate checksum entry: $name"
        }
        $path = Join-Path $Root $name
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Checksummed file is missing: $name"
        }
        $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash.ToLowerInvariant()
        if ($actual -ne $expected) {
            throw "Checksum mismatch: $name"
        }
    }

    foreach ($name in $requiredKitFiles) {
        if (-not $verifiedNames.Contains($name)) {
            throw "Required file is not checksummed: $name"
        }
    }
    if ($verifiedNames.Count -ne $requiredKitFiles.Count) {
        throw "Expected $($requiredKitFiles.Count) checksummed files, verified $($verifiedNames.Count)"
    }
    Write-Host "Verified $($verifiedNames.Count) alpha-kit files."
}

function Assert-ApiUrl {
    param([string]$Value)

    if ([string]::IsNullOrWhiteSpace($Value)) {
        throw "-ApiUrl is required for Doctor and Launch modes"
    }
    $uri = [Uri]$Value
    if ($uri.Scheme -notin @("http", "https") -or -not $uri.IsAbsoluteUri) {
        throw "ApiUrl must be an absolute HTTP or HTTPS URL"
    }
    if ($uri.UserInfo -or $uri.Query -or $uri.Fragment) {
        throw "ApiUrl must not contain credentials, a query, or a fragment"
    }
    return $Value.TrimEnd('/')
}

function Assert-StunServer {
    param([string]$Value)

    if ([string]::IsNullOrWhiteSpace($Value)) {
        return
    }
    if ($Value -notmatch '^((?:\d{1,3}\.){3}\d{1,3}):(\d{1,5})$') {
        throw "StunServer must be a numeric IPv4 address and port"
    }
    $address = $null
    $port = [int]$Matches[2]
    if (-not [Net.IPAddress]::TryParse($Matches[1], [ref]$address) -or $port -lt 1 -or $port -gt 65535) {
        throw "StunServer must be a valid numeric IPv4 address and port"
    }
}

function Invoke-Doctor {
    param([string]$Root)

    Verify-Kit -Root $Root
    $normalizedApiUrl = Assert-ApiUrl -Value $ApiUrl
    Assert-StunServer -Value $StunServer

    if ([string]::IsNullOrWhiteSpace($RetroArchRoot)) {
        throw "-RetroArchRoot is required for Doctor and Launch modes"
    }
    $retroRoot = (Resolve-Path -LiteralPath $RetroArchRoot).Path
    foreach ($relativePath in @("retroarch.exe", "cores\fbneo_libretro.dll", "VERSION.txt", "ROMs")) {
        if (-not (Test-Path -LiteralPath (Join-Path $retroRoot $relativePath))) {
            throw "RetroArch layout is incomplete: $relativePath is missing"
        }
    }

    foreach ($endpoint in @("health", "ready")) {
        $response = Invoke-WebRequest -UseBasicParsing -Uri "$normalizedApiUrl/$endpoint" -TimeoutSec 10
        if ($response.StatusCode -ne 200) {
            throw "$endpoint returned HTTP $($response.StatusCode)"
        }
    }

    $reports = Join-Path $Root "reports"
    New-Item -ItemType Directory -Path $reports -Force | Out-Null
    Write-Host "Alpha doctor passed. Reports directory: $reports"
}

$resolvedKitRoot = Resolve-KitRoot -Path $KitRoot

switch ($Mode) {
    "Package" {
        Package-Kit -Root $resolvedKitRoot
        Verify-Kit -Root $resolvedKitRoot
    }
    "Verify" {
        Verify-Kit -Root $resolvedKitRoot
    }
    "Doctor" {
        Invoke-Doctor -Root $resolvedKitRoot
    }
    "Launch" {
        Invoke-Doctor -Root $resolvedKitRoot
        $env:OPENCADE_API_URL = (Assert-ApiUrl -Value $ApiUrl)
        $env:OPENCADE_RETROARCH_ROOT = (Resolve-Path -LiteralPath $RetroArchRoot).Path
        if ([string]::IsNullOrWhiteSpace($StunServer)) {
            Remove-Item Env:OPENCADE_STUN_SERVER -ErrorAction SilentlyContinue
        } else {
            $env:OPENCADE_STUN_SERVER = $StunServer
        }
        & (Join-Path $resolvedKitRoot "opencade-client.exe")
        if ($LASTEXITCODE -ne 0) {
            throw "OpenCade client exited with code $LASTEXITCODE"
        }
    }
}
