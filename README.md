# XTools for Windows

Windows 版本的 [XTools](https://github.com/XueshiQiao/XTools)，基于 **Rust + Tauri v2 + Windows Win32/WinRT API** 构建，延续 macOS 版轻量实用工具箱的交互与视觉风格，专注于免驱直连的外设硬件控制与高效键盘工作流。

## 功能一览

### ROG 键盘（ROG Falcata）

配合 macOS 版 XTools 的跨设备工作流：当你把 ROG Falcata 通过 USB 有线或 2.4G 无线重新连回 Windows 电脑时，自动将其切换回指定的 Windows 板载配置文件（默认为 **配置文件 1**）。

- **直接驱动 HID 协议**：通过 `windows` crate 直接读写华硕外设控制通道（`VID 0x0B05`、`UsagePage 0xFF00 / Usage 0x0001`），无需安装奥创中心（Armoury Crate）或额外驱动。
- **USB + 2.4G 双模自动切档**：
  - USB 有线（`PID 0x1C2F`）：通过 `WM_DEVICECHANGE` 事件零轮询瞬时触发。
  - 2.4G 接收器（`PID 0x1C31`）：通过低频握手探测离线到在线跳变。
  - 软件启动时自动校验当前档位并纠偏，也支持在托盘右键菜单中一键切档。
- **独立托盘电量图标**：键盘在线时自动在通知区域展示专用数字电量图标，支持总电量、充电 / 充满状态与低电量颜色告警；悬浮提示可查看左 / 右分体独立电量及当前档位。
- **奥创极速网页版与原生通知**：一键直达 [Gear Link](https://gearlink.asus.com/zh-cn) 网页驱动，自动切档结果支持 Windows 原生 Toast 通知（可单独开关提示音）。

### Razer Naga 鼠标（Razer Naga V3 Pro）

通过 Razer HID 协议直接读写鼠标硬件板载内存，无需常驻 Razer Synapse（雷云）；写入板载内存后的按键映射与灵敏度配置在蓝牙模式、其他电脑或反作弊游戏中均可原生生效，无后台键盘钩子。

- **USB + 2.4G 直连硬件协议**：支持 USB 有线（`PID 0x00E7`）与 2.4G HyperSpeed 接收器（`PID 0x00E8`），通过 `VID 0x1532`（`UsagePage 0x0001 / Usage 0x0003`、91 字节 Feature Report）直写硬件。
- **板载配置槽位管理**：支持 **1–5 号** 板载配置槽位（圆点颜色对应鼠标底部指示灯），支持快速切换、点击未启用槽位自动创建，以及删除槽位。
- **DPI 与轮询率调节**：支持 `100–35000` DPI、最多 `5` 个板载灵敏度档位，以及 `125 / 500 / 1000 Hz` 回报率直接写入板载内存。
- **可换侧面板硬件按键映射**：
  - 支持 **2 键 / 6 键 / 12 键** 侧面板布局。
  - 支持映射为任意键盘按键及 `Ctrl / Shift / Alt / Win` 组合键、鼠标按键、DPI 调节、配置切换、Razer HyperShift 或禁用，并支持一键恢复出厂默认。
  - 内置本地按键实测区，可即时验证侧键写入后的硬件输出。
- **独立托盘电量图标**：鼠标在线时自动显示数字电量托盘图标，悬浮可查看电量百分比、充电状态、当前配置槽位与当前 DPI。

### HyperCapslock

将 `CapsLock` 改造为全局 **Hyper** 修饰键：短按（≤ 200ms）仍正常切换大小写，按住搭配其他按键即可高效光标导航与执行快捷操作。

- **灵活的触发方式**：支持 `Caps + 按键`、`Caps + Shift + 按键`、`Caps ×1`、`Caps ×2`，以及双击左右修饰键（`Shift / Ctrl / Alt / Win`）。
- **内置动作类型与默认键位**：
  - **方向移动**：默认 `H/J/K/L`（左 / 下 / 上 / 右）、`Y/P`（上一词 / 下一词）、`A/E`（行首 `Home` / 行尾 `End`）。
  - **快速跳转**：默认 `U/D` 向上 / 向下跳转 10 行（支持自定义行数）。
  - **独立动作**：默认 `I`（退格）、`O`（换行到下一行 `End+Enter`）、`N`（插入居中三引号 `""""""`），另支持切换大小写锁定与切换输入法（`Win+Space`）。
  - **自定义组合快捷键与系统命令**：可按需绑定任意热键或启动外部程序。
- **规则管理**：支持在界面内新增、编辑、删除映射规则，支持一键恢复默认与 JSON 导入 / 导出。
- **进程自动暂停（反作弊友好）**：支持维护监控进程列表（手动输入或从当前运行进程中选取）。当列表中任一进程运行时，HyperCapslock 会自动暂停并**完全卸载底层键盘钩子（`WH_KEYBOARD_LL`）**；相关进程全部退出后自动恢复（手动暂停状态不会被自动恢复），可选配 Windows Toast 提示音。

### 通用体验与系统托盘

- **多托盘协同**：保留 XTools 主托盘图标（汇总设备状态、快捷切档、刷新电量、快速暂停 / 恢复 HyperCapslock），同时在 ROG Falcata 与 Razer Naga V3 Pro 连接时各自显示独立的数字电量图标。
- **中英双语界面**：支持跟随系统或手动切换 `简体中文` / `English`，主界面与系统托盘菜单实时同步。
- **轻量常驻体验**：支持登录时最小化到托盘自启（`--minimized`）、单实例运行、关闭窗口隐藏到托盘，以及在资源管理器中一键定位诊断日志文件。

## 下载与安装

前往 [GitHub Releases](https://github.com/XueshiQiao/XTools-Win/releases) 下载对应架构的 Windows 安装包（支持 `x64` 与 `ARM64`）。

- **系统要求**：Windows 10 / 11（需 WebView2 运行时，Windows 11 默认自带）。

## 技术栈

- **后端**：Rust、`tauri` v2（`tray-icon`、`single-instance`、`opener`）、`windows` crate（Win32 SetupAPI / HID Overlapped I/O 与 Feature Report / `WM_DEVICECHANGE` / `WH_KEYBOARD_LL` / ToolHelp32 进程快照 / DWM 暗色圆角 / Registry / WinRT ToastNotifications）
- **前端**：React 19 + TypeScript + Vite + Tailwind CSS + Lucide Icons
- **CI / 发布**：GitHub Actions（自动类型检查、前端构建、Rust 单元测试及 Windows `x86_64` / `aarch64` 安装包构建发布）

## 本地构建与开发

环境要求：Rust stable toolchain、Node.js 22+、`pnpm`、Windows 10/11（WebView2）。

> 在 Windows PowerShell 中请分别执行或使用 `;` 分隔命令。

```powershell
# 安装前端依赖
pnpm install

# 运行开发模式
pnpm tauri dev

# 前端类型检查与构建
pnpm typecheck
pnpm build

# 运行 Rust 单元测试
cargo test --manifest-path src-tauri/Cargo.toml

# 构建独立发布版可执行文件（不打包安装器）
pnpm tauri build --no-bundle

# 构建完整 Windows 安装包
pnpm tauri build
```

### 真机硬件只读 / 往返测试（可选）

需将对应外设通过 USB 或 2.4G 接收器连接到当前 Windows 主机：

```powershell
# ROG Falcata 只读检测
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored live_read_rog_falcata --nocapture

# Razer Naga V3 Pro 只读检测
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored live_read_razer_naga_v3_pro --nocapture

# Razer Naga V3 Pro 板载配置写入与恢复往返测试
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored live_naga_onboard_roundtrip --nocapture
```

## 相关项目

- macOS 版 XTools：[XueshiQiao/XTools](https://github.com/XueshiQiao/XTools)
- Windows 版仓库：[XueshiQiao/XTools-Win](https://github.com/XueshiQiao/XTools-Win)
