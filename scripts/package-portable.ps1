# ============================================================
# 喵尺 MeowFit — Portable 绿色版打包
#
# 前置：先执行 `npm run tauri build`（安装版由同一个命令产出）。
# 产物：artifacts/MeowFit-portable-v<版本>.zip，解压即用。
#
# 用法（在项目根目录）：
#   powershell -ExecutionPolicy Bypass -File scripts/package-portable.ps1
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
$stage = Join-Path $artifacts "MeowFit"
$zip = Join-Path $artifacts "MeowFit-portable-v$version.zip"

Write-Host "==> 整理 Portable 目录（v$version）"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force $stage | Out-Null

# 可执行文件
Copy-Item $exe $stage

# 随包资源：FFmpeg（规范 4：Portable 版运行期依赖全部内置）
$resources = Join-Path $release "resources"
if (-not (Test-Path $resources)) {
    Write-Error "找不到 $resources，打包前请确认 FFmpeg 已放入 src-tauri/resources/ffmpeg/win-x64/"
}
Copy-Item -Recurse $resources (Join-Path $stage "resources")

# 许可与说明
Copy-Item (Join-Path $root "LICENSE") $stage
Copy-Item (Join-Path $root "README.md") $stage

# Portable 版首次运行会在程序目录下建 config/，这里预置一个让用户知道它是什么
New-Item -ItemType Directory -Force (Join-Path $stage "config") | Out-Null
Set-Content -Encoding UTF8 (Join-Path $stage "config\说明.txt") @"
本目录存放喵尺 MeowFit 的配置（settings.json / presets.json / incremental-index.json）。
日志写在同级的 logs/ 目录，按日期滚动，保留 30 天。
删除整个程序目录即可彻底清除，不写注册表。
"@

Write-Host "==> 压缩"
New-Item -ItemType Directory -Force $artifacts | Out-Null
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zip -CompressionLevel Optimal

$sizeMb = [math]::Round((Get-Item $zip).Length / 1MB, 1)
Write-Host ""
Write-Host "==> 完成：$zip（$sizeMb MB）"
Write-Host "解压后目录结构："
Get-ChildItem $stage | ForEach-Object { Write-Host ("    " + $_.Name) }
