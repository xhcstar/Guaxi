param(
  [Parameter(Mandatory = $true)]
  [string]$BundleRoot,

  [Parameter(Mandatory = $true)]
  [string]$Destination
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path -LiteralPath $BundleRoot)) {
  throw "utils_bundles directory not found: $BundleRoot"
}

New-Item -ItemType Directory -Force -Path $Destination | Out-Null

$bundles = Get-ChildItem -LiteralPath $BundleRoot -Filter *.zip -File | Sort-Object Name
if ($bundles.Count -eq 0) {
  throw "utils bundle archive not found: $BundleRoot"
}

foreach ($bundle in $bundles) {
  Expand-Archive -LiteralPath $bundle.FullName -DestinationPath $Destination -Force
}

Set-Content -LiteralPath (Join-Path $Destination '.guaxi_utils_extract_complete') -Value 'ok' -Encoding ascii
