# Guaxi iOS 分享快捷指令

`Guaxi.shortcut` 是一个可导入的 iOS 快捷指令,用于把知乎/公众号/小红书/B站等 App 分享的
链接送进 Guaxi 采集队列(再由 PC 端抓取正文)。

## 它做了什么
分享面板输入(URL/文本) → URL 编码 → 打开 `guaxi-capture://capture?url=<编码后的链接>`。
Guaxi App(含 tauri-plugin-deep-link + Info.plist 里注册的 `guaxi-capture://` scheme)接收后
进入采集流程。

## 导入步骤
1. 先装好**带 URL scheme 的新版 Guaxi**(CI `unsigned-ipa` 产物 + Sideloadly)。
2. 把 `Guaxi.shortcut` 传到 iPhone(iCloud 云盘 / 发给自己的微信或邮件 / 隔空投送均可)。
3. iPhone 上开启:**设置 → 快捷指令 → 允许不受信任的快捷指令**
   (该开关需先随便运行过一次任意快捷指令后才会出现)。
4. 点开 `Guaxi.shortcut` → **添加快捷指令**。
5. 在知乎点分享 → 下滑找到 **Guaxi** → 点它即可。

## 自测
Safari 地址栏输入 `guaxi-capture://capture?url=https://example.com` 回车,
能唤起 Guaxi 就说明 scheme 正常。
