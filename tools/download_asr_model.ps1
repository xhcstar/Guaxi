# Download the sherpa-onnx SenseVoice multilingual ASR model used by the
# realtime speech-to-text feature, into:
#   src-tauri/utils/Models/asr/sense-voice
#
# Design:
#   1. Prefer ModelScope's Python SDK through the existing Marker virtual env.
#      This mirrors settingview.rs::run_modelscope_download and avoids fragile
#      website "resolve/master" URLs that may return 404.
#   2. Use a per-target MODELSCOPE_CACHE directory, so a stale global cache lock
#      in the user profile cannot block this project download.
#   3. Download only the runtime files: model.int8.onnx/model.onnx and tokens.txt.
#      The helper script copies those files into the requested local directory.
#   4. Keep GUAXI_ASR_SOURCE=github as an explicit fallback for the release
#      archive, but never modify the virtual env itself.
#
# Overrides (env vars):
#   GUAXI_ASR_SOURCE=github        Use GitHub release tar.bz2 instead of ModelScope.
#   GUAXI_ASR_FULL=1               Download full-precision model.onnx instead of int8.
#   GUAXI_ASR_MODEL_ID=<id>        ModelScope model id. Defaults to csukuangfj/<model>.
#   GUAXI_ASR_MODEL_FILE=<name>    Model file inside the ModelScope repo.
#   GUAXI_MODELSCOPE_PYTHON=<path> Python executable with access to modelscope.
#   GUAXI_ASR_MODEL_URL=<url>      Direct URL for the GitHub-style .tar.bz2 archive.
#
# ASCII-only on purpose: Windows PowerShell 5.1 may mis-parse non-ASCII scripts.

$ErrorActionPreference = "Stop"

try { [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12 } catch {}

$RepoRoot  = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$TargetDir = Join-Path $RepoRoot "src-tauri/utils/Models/asr/sense-voice"
$ModelName = "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17"
$DefaultModelScopeModelId = "csukuangfj/test-sense-voice-2026-04-03"
$DefaultModelScopeModelFile = "model-5-seconds.onnx"
$ModelScopeModelId = if ($env:GUAXI_ASR_MODEL_ID) { $env:GUAXI_ASR_MODEL_ID } else { $DefaultModelScopeModelId }
$GithubArchive = if ($env:GUAXI_ASR_MODEL_URL) {
    $env:GUAXI_ASR_MODEL_URL
} else {
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/$ModelName.tar.bz2"
}

$curl = (Get-Command curl.exe -ErrorAction SilentlyContinue)

function Get-FileLength {
    param([string]$Path)
    if (-not (Test-Path $Path)) { return 0 }
    return (Get-Item $Path).Length
}

function Test-AsrModelReady {
    $int8Path = Join-Path $TargetDir "model.int8.onnx"
    $fullPath = Join-Path $TargetDir "model.onnx"
    $tokensPath = Join-Path $TargetDir "tokens.txt"

    $hasInt8 = (Get-FileLength $int8Path) -ge 52428800
    $hasFull = (Get-FileLength $fullPath) -ge 104857600
    $hasTokens = (Get-FileLength $tokensPath) -ge 1024

    if (($hasInt8 -or $hasFull) -and $hasTokens) {
        Write-Host "[ASR] Valid model already present: $TargetDir (skip)" -ForegroundColor Green
        return $true
    }

    Write-Host "[ASR] Model validation status: int8=$hasInt8 full=$hasFull tokens=$hasTokens" -ForegroundColor DarkGray
    return $false
}

function Find-ModelScopePython {
    if ($env:GUAXI_MODELSCOPE_PYTHON) {
        if (Test-Path $env:GUAXI_MODELSCOPE_PYTHON) { return (Resolve-Path $env:GUAXI_MODELSCOPE_PYTHON).Path }
        throw "GUAXI_MODELSCOPE_PYTHON does not exist: $env:GUAXI_MODELSCOPE_PYTHON"
    }

    $venvDir = Join-Path $RepoRoot "src-tauri/utils/MarkdownConvertor/marker-1.10.1/.venv"
    $candidates = @(
        (Join-Path $venvDir "Scripts/python.exe"),
        (Join-Path $venvDir "python.exe"),
        (Join-Path $venvDir "bin/python")
    )

    foreach ($p in $candidates) {
        if (Test-Path $p) { return (Resolve-Path $p).Path }
    }

    throw "Cannot find Marker virtual-env python under: $venvDir"
}

function Invoke-ModelScopeDownload {
    param([string]$SourceFileName, [string]$TargetFileName, [long]$MinBytes)

    $python = Find-ModelScopePython
    $script = Join-Path $RepoRoot "src-tauri/scripts/modelscope_download.py"
    if (-not (Test-Path $script)) { throw "modelscope helper script not found: $script" }

    New-Item -ItemType Directory -Force -Path $TargetDir | Out-Null
    $downloadedFile = Join-Path $TargetDir $SourceFileName
    $outFile = Join-Path $TargetDir $TargetFileName
    $cacheRoot = Join-Path $TargetDir "_modelscope_cache"
    if (Test-Path $outFile) { Remove-Item -Force $outFile -ErrorAction SilentlyContinue }
    if ($downloadedFile -ne $outFile -and (Test-Path $downloadedFile)) {
        Remove-Item -Force $downloadedFile -ErrorAction SilentlyContinue
    }
    New-Item -ItemType Directory -Force -Path $cacheRoot | Out-Null

    Write-Host "[ASR] ModelScope download start: model_id=$ModelScopeModelId file=$SourceFileName target=$TargetFileName" -ForegroundColor Cyan
    Write-Host "[ASR] Python: $python" -ForegroundColor DarkGray
    Write-Host "[ASR] MODELSCOPE_CACHE: $cacheRoot" -ForegroundColor DarkGray

    $oldModelScopeCache = $env:MODELSCOPE_CACHE
    $env:MODELSCOPE_CACHE = $cacheRoot
    try {
        & $python $script --model-id $ModelScopeModelId --filename $SourceFileName --local-dir $TargetDir
        if ($LASTEXITCODE -ne 0) {
            throw "modelscope download failed (exit=$LASTEXITCODE): $SourceFileName"
        }
    }
    finally {
        $env:MODELSCOPE_CACHE = $oldModelScopeCache
    }

    if ($downloadedFile -ne $outFile) {
        if (-not (Test-Path $downloadedFile)) {
            throw "Downloaded source file missing: $downloadedFile"
        }
        Move-Item -Force $downloadedFile $outFile
        Write-Host "[ASR] Normalized model filename: $SourceFileName -> $TargetFileName" -ForegroundColor DarkGray
    }

    if ((Get-FileLength $outFile) -lt $MinBytes) {
        throw "Downloaded file too small or missing: $outFile"
    }

    Write-Host "[ASR] ModelScope download ok: $outFile" -ForegroundColor Green
}

function Download-ArchiveFile {
    param([string]$Url, [string]$OutFile, [long]$MinBytes = 1048576)

    Write-Host "[ASR] Downloading archive: $Url" -ForegroundColor Cyan
    if (Test-Path $OutFile) { Remove-Item -Force $OutFile -ErrorAction SilentlyContinue }
    if ($curl) {
        & curl.exe -L --fail --retry 3 --retry-delay 2 -o $OutFile $Url
        if ($LASTEXITCODE -ne 0) { throw "curl exit=$LASTEXITCODE for $Url" }
    } else {
        Invoke-WebRequest -Uri $Url -OutFile $OutFile -UseBasicParsing
    }
    if ((Get-FileLength $OutFile) -lt $MinBytes) {
        throw "Downloaded archive too small or missing: $OutFile"
    }
}

function Install-FromGithubArchive {
    $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("guaxi_asr_" + [System.Guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Force -Path $tmp | Out-Null
    $archive = Join-Path $tmp "$ModelName.tar.bz2"
    try {
        Download-ArchiveFile -Url $GithubArchive -OutFile $archive
        Write-Host "[ASR] Extracting archive..." -ForegroundColor Cyan
        & tar -xf $archive -C $tmp
        if ($LASTEXITCODE -ne 0) { throw "tar failed (exit=$LASTEXITCODE). Ensure system tar supports bz2." }

        $extracted = Join-Path $tmp $ModelName
        if (-not (Test-Path $extracted)) { throw "Extracted directory not found: $extracted" }

        New-Item -ItemType Directory -Force -Path (Split-Path $TargetDir -Parent) | Out-Null
        if (Test-Path $TargetDir) { Remove-Item -Recurse -Force $TargetDir }
        Move-Item -Force $extracted $TargetDir
        Write-Host "[ASR] GitHub archive install ok: $TargetDir" -ForegroundColor Green
    }
    finally {
        if (Test-Path $tmp) { Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue }
    }
}

function Remove-ModelScopeCache {
    $cacheDirs = @(
        (Join-Path $TargetDir "_modelscope_download"),
        (Join-Path $TargetDir "_modelscope_cache")
    )
    foreach ($cacheDir in $cacheDirs) {
        if (Test-Path $cacheDir) {
            Remove-Item -Recurse -Force $cacheDir -ErrorAction SilentlyContinue
            Write-Host "[ASR] Removed ModelScope cache: $cacheDir" -ForegroundColor DarkGray
        }
    }
}

if (Test-AsrModelReady) { exit 0 }

if ($env:GUAXI_ASR_SOURCE -eq "github") {
    Write-Host "[ASR] Source selected: GitHub archive" -ForegroundColor Cyan
    Install-FromGithubArchive
} else {
    Write-Host "[ASR] Source selected: ModelScope Python SDK" -ForegroundColor Cyan
    $full = ($env:GUAXI_ASR_FULL -eq "1")
    $modelSourceFileName = if ($env:GUAXI_ASR_MODEL_FILE) {
        $env:GUAXI_ASR_MODEL_FILE
    } elseif ($ModelScopeModelId -eq $DefaultModelScopeModelId) {
        $DefaultModelScopeModelFile
    } elseif ($full) {
        "model.onnx"
    } else {
        "model.int8.onnx"
    }
    $modelTargetFileName = if ($modelSourceFileName -eq $DefaultModelScopeModelFile) { "model.onnx" } else { $modelSourceFileName }
    $modelMinBytes = if ($modelTargetFileName -eq "model.onnx") { 104857600 } else { 52428800 }

    Invoke-ModelScopeDownload -SourceFileName $modelSourceFileName -TargetFileName $modelTargetFileName -MinBytes $modelMinBytes
    Invoke-ModelScopeDownload -SourceFileName "tokens.txt" -TargetFileName "tokens.txt" -MinBytes 1024
    Remove-ModelScopeCache
}

if (-not (Test-AsrModelReady)) {
    throw "Validation failed: missing valid model(.int8).onnx or tokens.txt in $TargetDir"
}

Write-Host "[ASR] Done: $TargetDir" -ForegroundColor Green
