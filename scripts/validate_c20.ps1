$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
$vcvars = Join-Path $vs 'VC/Auxiliary/Build/vcvars64.bat'
$out = Join-Path $root 'artifacts/cabi-2.0'
New-Item -ItemType Directory -Force $out | Out-Null
$batch = Join-Path $out 'compile.cmd'
@"
@echo off
call "$vcvars" >nul
if errorlevel 1 exit /b 1
cl /nologo /W4 /WX /I"$root\OOS-Wollaston\include" "$root\OOS-Wollaston\examples\embed.c" /Fo"$out\embed.obj" /Fe"$out\embed.exe" /link "$root\OOS-Wollaston\target\release\wollaston_ffi.dll.lib"
if errorlevel 1 exit /b 1
cl /nologo /TP /W4 /WX /I"$root\OOS-Wollaston\include" "$root\OOS-Wollaston\examples\embed.c" /Fo"$out\embed_cpp.obj" /Fe"$out\embed_cpp.exe" /link "$root\OOS-Wollaston\target\release\wollaston_ffi.dll.lib"
exit /b %errorlevel%
"@ | Set-Content -LiteralPath $batch -Encoding utf8
& cmd.exe /d /c $batch
if ($LASTEXITCODE -ne 0) { throw 'C/C++ ABI compile failed' }
Copy-Item -LiteralPath (Join-Path $root 'OOS-Wollaston/target/release/wollaston_ffi.dll') -Destination $out -Force
$c = & (Join-Path $out 'embed.exe')
if ($LASTEXITCODE -ne 0 -or $c -notmatch 'ABI 1:') { throw 'C ABI runtime failed' }
$cpp = & (Join-Path $out 'embed_cpp.exe')
if ($LASTEXITCODE -ne 0 -or $cpp -notmatch 'ABI 1:') { throw 'C++ ABI runtime failed' }
@{ compiler='MSVC x64 /W4 /WX'; c_compiled_and_ran=$true; cpp_compiled_and_ran=$true; abi=1; dll_sha256=(Get-FileHash (Join-Path $out 'wollaston_ffi.dll')).Hash.ToLower() } | ConvertTo-Json | Set-Content (Join-Path $root 'docs/validation-c-2.0.json') -Encoding utf8
Write-Output 'C and C++ clients compiled, linked and ran against ABI 1.'
