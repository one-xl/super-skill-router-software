param(
    [string]$Python = "tools\skillspector\.venv\Scripts\python.exe"
)

$ErrorActionPreference = "Stop"
$source = Join-Path $PSScriptRoot "..\tools\skillspector\SkillSpector-main\src"
$entryPoint = Join-Path $PSScriptRoot "skillspector_sidecar.py"
$binaryName = "skillspector-x86_64-pc-windows-msvc"

if (-not (Test-Path -LiteralPath $Python)) {
    throw "SkillSpector virtual environment was not found: $Python"
}
if (-not (Test-Path -LiteralPath $source)) {
    throw "SkillSpector source was not found: $source"
}

& $Python -m PyInstaller --noconfirm --clean --onefile --name $binaryName `
    --paths $source --collect-all skillspector `
    --distpath "src-tauri\binaries" --workpath "tmp\pyinstaller-work" `
    --specpath "tmp\pyinstaller-spec" $entryPoint

if ($LASTEXITCODE -ne 0) {
    throw "PyInstaller failed with exit code $LASTEXITCODE"
}
