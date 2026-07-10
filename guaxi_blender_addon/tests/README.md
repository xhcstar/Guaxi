# Guaxi Blender Bridge 测试

`fixtures/smoke_story_ir.json` 是不依赖大模型和外部资产的确定性烟雾测试，用于验证：

- Blender 版本与 Python 启动。
- Story IR 解析。
- 场景、摄影机、灯光、材质和关键帧创建。
- `.blend` 保存。
- Eevee PNG 图片序列输出。
- 配置 `GUAXI_FFMPEG_PATH` 后的 H.264 MP4 输出。

在工程根目录执行：

```powershell
& 'D:\blender-5.2.0\blender.exe' `
  --background `
  --factory-startup `
  --python-exit-code 17 `
  --python .\guaxi_blender_addon\guaxi_blender_bridge\headless_runner.py `
  -- `
  --project-dir .\.tmp\story-studio-smoke `
  --story-ir .\guaxi_blender_addon\tests\fixtures\smoke_story_ir.json `
  --mode preview
```

成功时应生成：

```text
.tmp/story-studio-smoke/blender/preview.blend
.tmp/story-studio-smoke/renders/preview/frames/frame_000001.png
.tmp/story-studio-smoke/renders/preview/preview.mp4
```

如果未安装独立 FFmpeg，`.blend` 与完整 PNG 序列仍应成功，日志会显示 `awaiting-encoder`，MP4 留待编码器就绪后生成。
