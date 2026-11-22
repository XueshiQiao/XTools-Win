# XTools for Windows

Windows 版本的 [XTools](https://github.com/XueshiQiao/XTools)，基于 **Rust + Tauri v2 + Windows Win32/WinRT API** 构建，延续 macOS 版轻量实用工具箱的交互与视觉风格。

## 当前功能

### ROG 键盘（ROG Falcata）

配合 macOS 版 XTools 的跨设备工作流：当你把 ROG Falcata 通过 USB 有线或 2.4G 无线重新连回 Windows 电脑时，自动将其切换回指定的 Windows 板载配置文件（默认为 **配置文件 1**）。

- **直接驱动 HID 协议**：通过 `windows` crate 直接读写华硕官方外设控制通道（`VID 0x0B05`、`UsagePage 0xFF00 / Usage 0x0001`），无需独占硬件、无需安装额外驱动。
- **USB + 2.4G 双模自动切档**：
  - USB 有线（`PID 0x1C2F`）：通过 `WM_DEVICECHANGE` 事件零轮询瞬时触发。
  - 2.4G 接收器（`PID 0x1C31`）：通过低频握手探测离线到在线跳变。
  - 软件启动时也会自动校验当前档位并纠偏。
- **双托盘图标 + 实时分体电量显示**：
  - 保留 XTools 主托盘图标与右键快捷菜单。
  - 键盘在线时自动展示第二个专用托盘数字电量图标（支持总电量、充电/充满状态与低电量颜色告警，悬浮查看左/右分体独立电量）。
- **奥创极速网页版入口**：一键直达 [Gear Link](https://gearlink.asus.com/zh-cn)。
- **Windows 原生 Toast 通知与开机自启**：自动切档结果即时通知（支持关闭提示音），支持登录时最小化到托盘自启。

## 技术栈

- **后端**：Rust、`tauri` v2（`tray-icon`）、`windows` crate（Win32 SetupAPI / HID Overlapped I/O / `WM_DEVICECHANGE` / Registry / WinRT ToastNotifications）
- **前端**：React 19 + TypeScript + Vite + Tailwind CSS + Lucide Icons

## 本地构建与运行

环境要求：Rust toolchain、Node.js、`pnpm`、Windows 10/11（WebView2）。

```powershell
# 安装前端依赖
pnpm install

# 运行开发模式
pnpm tauri dev

# 运行 Rust 单元测试
cd src-tauri
cargo test

# 运行真机只读硬件检测（需已连接 ROG Falcata）
cargo test -- --ignored live_read_rog_falcata --nocapture

# 前端类型检查与构建发布版
cd ..
pnpm typecheck
pnpm tauri build
```

## 相关项目

- macOS 版 XTools：[XueshiQiao/XTools](https://github.com/XueshiQiao/XTools)
