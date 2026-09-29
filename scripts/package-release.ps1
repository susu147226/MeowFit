# ============================================================
# 喵尺 MeowFit — 发布产物打包
#
# 前置：先执行 `npm run tauri build`（安装版由同一个命令产出）。
# 产物（都放在项目根目录的 artifacts/ 下，便于一起找到与上传）：
#   artifacts/MeowFit-portable-v<版本>.zip           Portable 绿色版，解压即用
#   artifacts/MeowFit_<版本>_x64-setup.exe           安装版（NSIS）
#
# 用法（在项目根目录）：
#   powershell -ExecutionPolicy Bypass -File scripts/package-release.ps1
# ============================================================

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$release = Join-Path $root "src-tauri\target\release"
$exe = Join-Path $release "MeowFit.exe"

if (-not (Test-Path $exe)) {
    Write-Error "找不到 $exe，请先执行 npm run tauri build"
}

$version = (Get-Content (Join-Path $root "package.json") -Raw -Encoding UTF8 | ConvertFrom-Json).version
$artifacts = Join-Path $root "artifacts"
New-Item -ItemType Directory -Force $artifacts | Out-Null

# ---------- 1) 安装版：从打包目录收集到 artifacts/ ----------
Write-Host "==> 收集安装包（v$version）"
$setup = Get-ChildItem -Path (Join-Path $release "bundle\nsis") -Filter "*setup.exe" |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $setup) {
    Write-Error "找不到 NSIS 安装包，请确认 npm run tauri build 已成功完成"
}
$setupTarget = Join-Path $artifacts $setup.Name
Copy-Item $setup.FullName $setupTarget -Force
Write-Host "    $($setup.Name)"

# ---------- 2) Portable 绿色版 ----------
Write-Host "==> 整理 Portable 目录"
$stage = Join-Path $artifacts "MeowFit"
$zip = Join-Path $artifacts "MeowFit-portable-v$version.zip"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force $stage | Out-Null

Copy-Item $exe $stage

# 随包资源：FFmpeg（规范 4：Portable 版运行期依赖全部内置）
$resources = Join-Path $release "resources"
if (-not (Test-Path $resources)) {
    Write-Error "找不到 $resources，打包前请确认 FFmpeg 已放入 src-tauri/resources/ffmpeg/win-x64/"
}
Copy-Item -Recurse $resources (Join-Path $stage "resources")

Copy-Item (Join-Path $root "LICENSE") $stage
Copy-Item (Join-Path $root "README.md") $stage

# Portable 版首次运行会在程序目录下建 config/，这里预置一个让用户知道它是什么
New-Item -ItemType Directory -Force (Join-Path $stage "config") | Out-Null
Set-Content -Encoding UTF8 (Join-Path $stage "config\说明.txt") @"
本目录存放喵尺 MeowFit 的配置（settings.json / presets.json / incremental-index.json）。
日志写在同级的 logs/ 目录，按日期滚动，保留 30 天。
删除整个程序目录即可彻底清除，不写注册表。

注意：MeowFit.exe 不能单独拷出来运行，它需要同级的 resources/ 目录（内含 FFmpeg）。
"@

Write-Host "==> 压缩"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zip -CompressionLevel Optimal

Write-Host ""
Write-Host "==> 完成，artifacts/ 下："
Get-ChildItem $artifacts -File | ForEach-Object {
    Write-Host ("    {0}  {1} MB" -f $_.Name, [math]::Round($_.Length / 1MB, 1))
}
