# Gera o pacote para distribuir: dist\HP12C-<versão>-windows.zip
# (executável portátil + manual + programas de exemplo + README). Uso: pwsh scripts\release.ps1
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

cargo test --workspace
if ($LASTEXITCODE -ne 0) { throw 'testes falharam' }
cargo build --release -p hp12c-desktop
if ($LASTEXITCODE -ne 0) { throw 'build falhou' }

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$stage = Join-Path $root "dist\HP12C-$version"
$zip = Join-Path $root "dist\HP12C-$version-windows.zip"

if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item target\release\hp12c.exe (Join-Path $stage 'HP12C.exe')
Copy-Item README.md, LICENSE $stage
# Mesma pasta que o F1 usa, para o manual já estar lá antes de abrir o programa.
$manual = New-Item -ItemType Directory -Force (Join-Path $stage 'manual')
Copy-Item docs\manual-usuario.html, docs\m2-visual.png $manual
$examples = New-Item -ItemType Directory -Force (Join-Path $stage 'exemplos')
Copy-Item examples\programas\* $examples
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path "$stage\*" -DestinationPath $zip

$size = (Get-Item $zip).Length / 1MB
"Pacote pronto: $zip ({0:N1} MB)" -f $size
