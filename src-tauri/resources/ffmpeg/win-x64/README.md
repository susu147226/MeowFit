# FFmpeg 二进制放置目录

把 **GPL 构建** 的 FFmpeg 可执行文件放到本目录（`src-tauri/resources/ffmpeg/win-x64/`）：

```
ffmpeg.exe
ffprobe.exe
```

- 只需要 **win-x64** 一份。
- 二进制**不纳入版本控制**（见仓库根 `.gitignore`），需自行获取后放入。
- 本项目以**独立进程**方式调用 FFmpeg，不链接、不修改其源码。
- 打包前须完成构建自检，确认包含 `libx264`、`libx265`、`libvpx-vp9`、`libaom-av1`
  编码器，`palettegen`、`paletteuse`、`zscale`、`tonemap` 滤镜，以及 `hevc` 解码器。

FFmpeg 适用 GPL 许可证，**不受本项目私有许可证约束**，详见仓库根 `README.md`
的「许可与致谢」一节。
