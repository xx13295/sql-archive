$ErrorActionPreference = "Stop"
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$engineDir = $env:SQL_ARCHIVE_ENGINE_DIR
if (-not $engineDir) {
  $engineDir = Join-Path $here "..\..\tool\java"
}
$engineJar = Get-ChildItem -Path (Join-Path $engineDir "sql-translate*.jar") | Select-Object -First 1
if (-not $engineJar) {
  throw "sql-translate*.jar not found in $engineDir"
}
$classes = Join-Path $here "dist\classes"
$dist = Join-Path $here "dist"
if (Test-Path $classes) { Remove-Item -Recurse -Force $classes }
New-Item -ItemType Directory -Force -Path $classes | Out-Null
$sources = Get-ChildItem -Path (Join-Path $here "src") -Recurse -Filter *.java | ForEach-Object { $_.FullName }
javac -encoding UTF-8 -cp $engineJar.FullName -d $classes $sources
if ($LASTEXITCODE -ne 0) { throw "javac failed" }
Push-Location $classes
try {
  jar cf (Join-Path $dist "sql-archive-bridge.jar") cn
  if ($LASTEXITCODE -ne 0) { throw "jar failed" }
} finally {
  Pop-Location
}
Write-Output ("built " + (Join-Path $dist "sql-archive-bridge.jar"))