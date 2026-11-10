$ErrorActionPreference = "Stop"

param(
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path,
    [switch]$CleanOnly
)

function New-ResourceZip {
    param(
        [Parameter(Mandatory = $true)]
        [string]$SourceDir
    )

    $resolvedSource = (Resolve-Path $SourceDir).Path
    $zipPath = [System.IO.Path]::ChangeExtension($resolvedSource, ".zip")

    if (Test-Path -LiteralPath $zipPath) {
        Remove-Item -LiteralPath $zipPath -Force
    }

    if ($CleanOnly) {
        Write-Host "Removed $zipPath"
        return
    }

    Write-Host "Compressing $resolvedSource -> $zipPath"
    Compress-Archive -Path (Join-Path $resolvedSource "*") -DestinationPath $zipPath -CompressionLevel Optimal
}

$targets = @(
    "src-tauri/utils/SlidevEnv",
    "src-tauri/utils/sherpa-onnx",
    "src-tauri/utils/whisper",
    "src-tauri/utils/llama",
    "src-tauri/utils/python_libs",
    "src-tauri/utils/Models/tts",
    "src-tauri/utils/Models/whisper_models"
)

Set-Location $RepoRoot

foreach ($target in $targets) {
    if (-not (Test-Path -LiteralPath $target)) {
        Write-Warning "Skip missing directory: $target"
        continue
    }
    New-ResourceZip -SourceDir $target
}

if ($CleanOnly) {
    Write-Host "Bundled resource archives cleaned."
} else {
    Write-Host "Bundled resource archives prepared."
}
