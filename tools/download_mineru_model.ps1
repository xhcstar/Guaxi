# Pre-download MinerU models into:
#   src-tauri/utils/Models/mineru_models
#
# Uses the dedicated MinerU venv python built by build_mineru_env.sh.
# Points HF_HOME / MODELSCOPE_CACHE at the project models dir so the cache
# matches the runtime location configured in engine.rs.
# If MinerU is not installed, this script skips quietly (app falls back to marker).
#
# ASCII-only on purpose: Windows PowerShell 5.1 may mis-parse non-ASCII scripts.

$ErrorActionPreference = "Stop"

$RepoRoot   = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$MinerUDir  = Join-Path $RepoRoot "src-tauri/utils/MinerU"
$VenvDir    = Join-Path $MinerUDir ".venv"
$ModelsDir  = Join-Path $RepoRoot "src-tauri/utils/Models/mineru_models"
if (-not $env:MINERU_MODEL_SOURCE) { $env:MINERU_MODEL_SOURCE = "modelscope" }

New-Item -ItemType Directory -Force -Path $ModelsDir | Out-Null

function Find-Python {
    $candidates = @(
        (Join-Path $VenvDir "Scripts/python.exe"),
        (Join-Path $VenvDir "python.exe"),
        (Join-Path $VenvDir "bin/python")
    )
    foreach ($p in $candidates) {
        if (Test-Path $p) { return $p }
    }
    return $null
}

$Py = Find-Python
if (-not $Py) {
    Write-Host "[MinerU] MinerU venv not found. Run build_mineru_env.sh first. Skipping."
    exit 0
}

$env:HF_HOME               = $ModelsDir
$env:HUGGINGFACE_HUB_CACHE = $ModelsDir
$env:MODELSCOPE_CACHE      = $ModelsDir
$env:TRANSFORMERS_CACHE    = $ModelsDir
$env:TORCH_HOME            = $ModelsDir
$env:XDG_CACHE_HOME        = $ModelsDir

Write-Host "[MinerU] Models cache dir: $ModelsDir"
Write-Host "[MinerU] Model source: $($env:MINERU_MODEL_SOURCE)"

$hasMineru = $false
try { & $Py -c "import mineru" 2>$null; if ($LASTEXITCODE -eq 0) { $hasMineru = $true } } catch {}

if ($hasMineru) {
    Write-Host "[MinerU] Using mineru built-in downloader"
    & $Py -m mineru.cli.models_download 2>$null
    if ($LASTEXITCODE -ne 0) {
        & $Py -m mineru.model.download 2>$null
        if ($LASTEXITCODE -ne 0) {
            Write-Host "[MinerU] Built-in downloader unavailable; models will be fetched on first conversion."
        }
    }
} else {
    $hasMagic = $false
    try { & $Py -c "import magic_pdf" 2>$null; if ($LASTEXITCODE -eq 0) { $hasMagic = $true } } catch {}
    if ($hasMagic) {
        Write-Host "[MinerU] Detected magic_pdf (1.x); models fetched on first run."
    } else {
        Write-Host "[MinerU] Neither mineru nor magic_pdf found. Skipping."
    }
}

Write-Host "[MinerU] Done (models auto-complete on first conversion if not pre-downloaded)."
