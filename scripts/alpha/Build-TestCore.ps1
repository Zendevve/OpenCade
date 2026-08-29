[CmdletBinding()]
param(
    [string]$OutputDirectory = "fixtures\libretro\opencade-test-core\build"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$source = Join-Path $repositoryRoot "fixtures\libretro\opencade-test-core\opencade_test_core.c"
$output = Join-Path $repositoryRoot $OutputDirectory
New-Item -ItemType Directory -Path $output -Force | Out-Null

$compiler = Get-Command cl.exe -ErrorAction SilentlyContinue
if (-not $compiler) {
    throw "Microsoft C compiler cl.exe is required to build the OpenCade test core"
}

Push-Location $output
try {
    & $compiler.Source /nologo /LD /O2 /W4 /WX /TC $source /link /OUT:opencade_test_libretro.dll
    if ($LASTEXITCODE -ne 0) {
        throw "OpenCade test core compilation failed with code $LASTEXITCODE"
    }
} finally {
    Pop-Location
}

Write-Host "Built original OpenCade test core at $output\opencade_test_libretro.dll"
