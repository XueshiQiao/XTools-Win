import {
  EffectiveLanguage,
  KeyShortcut,
  NagaHardwareAction,
  NagaLink,
  NagaPlateLayout,
  RogLink,
} from './types';

const ZH_STRINGS: Record<string, string> = {
  'nav.toggleSidebar': '切换侧栏',
  'status.ready': 'XTools 已就绪',
  'sidebar.general': '通用',
  'sidebar.about': '关于',

  'tool.rogkeyboard.title': 'ROG 键盘',
  'rog.action.refresh': '刷新',
  'rog.section.status': '键盘',
  'rog.status.connected': '已连接',
  'rog.status.disconnected': '未连接',
  'rog.status.disconnected.hint':
    '现在通过 USB 或 2.4G 接收器联系不上这把键盘——它多半被切到了 Mac、蓝牙，或者线拔了。',
  'rog.status.currentProfile': '当前档位',
  'rog.status.battery': '电池电量',
  'rog.battery.charging': '充电中',
  'rog.battery.full': '已充满',
  'rog.battery.split': '左 {left}% · 右 {right}%',
  'rog.status.link': '连接方式',
  'rog.status.firmware': '固件版本',
  'rog.link.wired': 'USB 有线',
  'rog.link.wireless24g': '2.4G 无线',
  'rog.link.bluetooth': '蓝牙',
  'rog.gearlink.title': '打开奥创极速网页版 (Gear Link)',
  'rog.gearlink.subtitle': '在浏览器中配置按键、灯效与板载配置文件',

  'rog.section.profiles': '档位分配',
  'rog.profile.windows': 'Windows 档',
  'rog.profile.default': '默认 配置文件',
  'rog.profile.named': '配置文件 {n}',
  'rog.action.switchToWindows': '切到 Windows 档',
  'rog.section.profiles.footer':
    '指定键盘连上这台 Windows 电脑时要自动激活的板载配置文件（默认为 配置文件 1）。托盘右键菜单里也可以随时一键切换。',

  'rog.section.automation': '自动化',
  'rog.auto.title': '连上 Windows 时自动切换',
  'rog.auto.subtitle':
    '检测到键盘重新连上这台 Windows（USB 有线或 2.4G）以及软件启动时，自动切到 Windows 档。',
  'rog.sound.title': '通知带提示音',
  'rog.notify.denied':
    'XTools 的系统通知被关掉了，切档时不会有横幅提示。可在「Windows 设置 → 系统 → 通知」里打开。',
  'rog.section.automation.footer':
    '键盘连上 Windows（无论 USB 拔插还是拨回 2.4G 接收器）都会被自动识别并还原为 Windows 档。',

  'tool.razernaga.title': 'Razer Naga 鼠标',
  'naga.section.status': '鼠标状态',
  'naga.status.connected': '已连接',
  'naga.status.disconnected': '未连接',
  'naga.status.disconnected.hint':
    '未检测到 Razer Naga V3 Pro（USB 有线或 2.4G 接收器）。读取和修改板载配置需要通过 USB 或 2.4G 连接；一旦写入板载内存，在蓝牙模式、其他电脑或反作弊游戏中都能原生生效。',
  'naga.status.battery': '电池电量',
  'naga.battery.charging': '充电中',
  'naga.battery.full': '已充满',
  'naga.status.link': '连接方式',
  'naga.status.activeProfile': '当前板载档',
  'naga.status.dpi': '当前 DPI',
  'naga.status.pollingRate': '轮询率',
  'naga.status.firmware': '固件版本',
  'naga.link.wired': 'USB 有线',
  'naga.link.wireless24g': '2.4G 无线',

  'naga.section.profileDpi': '板载配置文件与灵敏度',
  'naga.profile.title': '板载配置槽位',
  'naga.profile.slot': '配置 {n}',
  'naga.profile.deleteHint': '删除配置槽位 {n}',
  'naga.dpi.title': 'DPI 灵敏度档位',
  'naga.dpi.stage': '档位 {n}',
  'naga.dpi.addStage': '新增档位',
  'naga.dpi.removeStage': '删除档位',
  'naga.dpi.apply': '应用 DPI',
  'naga.polling.title': '回报率 / 轮询率',
  'naga.profileDpi.footer':
    '支持 1–5 号板载配置槽位（圆点颜色对应鼠标底部指示灯）。点击未创建的槽位会自动创建并切换；DPI 与轮询率修改后直接写入鼠标硬件板载内存。',

  'naga.section.mappings': '侧面板硬件按键映射',
  'naga.plate.title': '侧面板按键布局',
  'naga.plate.two_button': '2 键面板',
  'naga.plate.six_button': '6 键面板',
  'naga.plate.twelve_button': '12 键面板',
  'naga.plate.reset': '恢复出厂默认',
  'naga.bind.recording': '请按下键盘按键或组合键 (Esc 取消)…',
  'naga.bind.empty': '未连接',
  'naga.bind.unsupportedKey': '鼠标板载内存暂不支持映射该按键。',
  'naga.action.disabled': '已禁用',
  'naga.action.hypershift': 'Razer HyperShift',
  'naga.action.profile': '切换配置文件',
  'naga.action.mouse.left': '鼠标左键',
  'naga.action.mouse.right': '鼠标右键',
  'naga.action.mouse.middle': '鼠标中键',
  'naga.action.mouse.back': '鼠标后退 (X1)',
  'naga.action.mouse.forward': '鼠标前进 (X2)',
  'naga.action.mouse.scroll_up': '滚轮向上',
  'naga.action.mouse.scroll_down': '滚轮向下',
  'naga.action.dpi.up': 'DPI 升高',
  'naga.action.dpi.down': 'DPI 降低',
  'naga.action.dpi.cycle_up': 'DPI 循环切换 (+)',
  'naga.action.dpi.cycle_down': 'DPI 循环切换 (-)',
  'naga.action.type.keyboard': '键盘按键…',
  'naga.action.type.disabled': '禁用按键',
  'naga.action.type.other': '其他硬件动作',
  'naga.mappings.footer':
    '点击左侧映射标签后按下任意键盘按键（支持 Ctrl / Shift / Alt / Win 组合键），会通过 Razer HID 协议直接写入鼠标板载内存。写入后可完全退出雷云和 XTools，无后台键盘钩子、支持长按、且兼容反作弊游戏。',

  'naga.section.test': '按键实测区',
  'naga.test.placeholder': '点击此输入框，再按下鼠标侧键测试硬件实际输出…',
  'naga.test.lastKey': '最近输入',
  'naga.test.clear': '清空',

  'tool.hypercapslock.title': 'HyperCapslock',
  'hyper.section.service': '服务控制',
  'hyper.status.running': '运行中',
  'hyper.status.paused': '已暂停',
  'hyper.status.subtitle':
    '将 CapsLock 变成 Hyper 修饰键（短按 200ms 内仍可切换大小写；按住不放搭配其他键位可高效导航与执行操作）。',
  'hyper.action.pause': '暂停',
  'hyper.action.resume': '恢复',
  'hyper.action.reset': '恢复默认',
  'hyper.action.import': '导入',
  'hyper.action.export': '导出',
  'hyper.action.add': '添加映射',
  'hyper.section.mappings': '按键映射规则',
  'hyper.mappings.empty': '当前没有任何映射规则，点击右上角「添加映射」或「恢复默认」。',
  'hyper.group.directional': '方向移动',
  'hyper.group.jump': '快速跳转',
  'hyper.group.independent': '独立动作',
  'hyper.group.command': '系统命令',
  'hyper.group.key_combo': '组合快捷键',
  'hyper.action.left': '左移 (Left)',
  'hyper.action.right': '右移 (Right)',
  'hyper.action.up': '上移 (Up)',
  'hyper.action.down': '下移 (Down)',
  'hyper.action.word_forward': '下一词 (Ctrl+Right)',
  'hyper.action.word_back': '上一词 (Ctrl+Left)',
  'hyper.action.home': '行首 (Home)',
  'hyper.action.end': '行尾 (End)',
  'hyper.action.jump_up': '向上跳转 {count} 行',
  'hyper.action.jump_down': '向下跳转 {count} 行',
  'hyper.action.backspace': '退格 (Backspace)',
  'hyper.action.next_line': '换行到下一行 (End+Enter)',
  'hyper.action.insert_quotes': '插入三引号 ("""""" 并居中)',
  'hyper.action.toggle_caps_lock': '切换大小写锁定 (CapsLock)',
  'hyper.action.switch_input_source': '切换输入法 (Win+Space)',
  'hyper.trigger.single_tap': 'Caps ×1',
  'hyper.trigger.double_tap': 'Caps ×2',
  'hyper.trigger.caps': 'Caps',
  'hyper.trigger.caps_shift': 'Caps + Shift',
  'hyper.trigger.dtm': '双击修饰键',
  'hyper.mod.left_shift': '左 Shift ×2',
  'hyper.mod.right_shift': '右 Shift ×2',
  'hyper.mod.left_control': '左 Ctrl ×2',
  'hyper.mod.right_control': '右 Ctrl ×2',
  'hyper.mod.left_alt': '左 Alt ×2',
  'hyper.mod.right_alt': '右 Alt ×2',
  'hyper.mod.left_win': '左 Win ×2',
  'hyper.mod.right_win': '右 Win ×2',
  'hyper.modal.addTitle': '添加映射',
  'hyper.modal.editTitle': '编辑映射',
  'hyper.modal.triggerType': '触发方式',
  'hyper.modal.triggerKey': '按下触发键',
  'hyper.modal.actionType': '执行动作',
  'hyper.modal.targetKey': '按下目标键',
  'hyper.modal.commandPlaceholder': '例如：notepad.exe 或 explorer.exe',
  'hyper.modal.jumpCount': '行数',
  'hyper.modal.save': '保存',
  'hyper.modal.cancel': '取消',
  'hyper.importSuccess': '已导入 {count} 条映射规则。',
  'hyper.importFailed': '导入失败：{error}',
  'hyper.status.autoPaused': '已自动暂停',
  'hyper.status.autoPausedBy': '检测到进程 {processes} 正在运行，HyperCapslock 已自动暂停（已卸载底层键盘钩子）；这些进程全部退出后会自动恢复。',
  'hyper.autoPause.section': '进程自动暂停',
  'hyper.autoPause.footer':
    '列表中的任一进程运行时，HyperCapslock 会自动暂停并完全卸载键盘钩子，避免与游戏或反作弊冲突；所有进程都退出后自动恢复。若你本来就手动暂停了它，则不会被自动恢复。',
  'hyper.autoPause.enable.title': '启用进程自动暂停',
  'hyper.autoPause.enable.subtitle': '检测到下列进程运行时自动暂停并卸载键盘钩子，进程结束后自动恢复。',
  'hyper.autoPause.sound.title': '通知带提示音',
  'hyper.autoPause.input.placeholder': '进程名，例如 steam.exe 或 cs2',
  'hyper.autoPause.add': '添加',
  'hyper.autoPause.pick': '从运行中选择',
  'hyper.autoPause.empty': '尚未添加任何进程。可以手动输入进程名，或从运行中的进程里选择。',
  'hyper.autoPause.running': '运行中',
  'hyper.autoPause.remove': '移除',
  'hyper.autoPause.picker.title': '选择运行中的进程',
  'hyper.autoPause.picker.search': '搜索进程…',
  'hyper.autoPause.picker.refresh': '刷新',
  'hyper.autoPause.picker.added': '已添加',
  'hyper.autoPause.picker.empty': '没有匹配的进程。',
  'hyper.autoPause.picker.close': '关闭',
  'hyper.autoPause.picker.loading': '正在读取运行中的进程…',
  'hyper.autoPause.picker.selectedCount': '已选 {count} 个进程',
  'hyper.autoPause.picker.done': '完成',

  'tool.usbinspector.title': 'USB 端口',
  'usb.section.overview': '概览与控制',
  'usb.overview.summary': '已连接 {devices} 台设备 · {controllers} 个控制器',
  'usb.overview.healthy': '一切正常',
  'usb.overview.issues': '{count} 项提示',
  'usb.overview.scanning': '扫描中…',
  'usb.overview.lastScan': '更新于 {time}',
  'usb.showEmpty.title': '显示空闲端口',
  'usb.showEmpty.subtitle': '显示尚未插设备的物理 USB 接口，方便查看空闲口规格与位置备注',
  'usb.section.diagnostics': '诊断与建议',
  'usb.healthy.banner': '当前所有 USB 控制器与已连接端口状态正常，未检测到异常掉线或报错设备。',
  'usb.role.cpu_direct': 'CPU 直连',
  'usb.role.chipset': '主板芯片组',
  'usb.role.discrete': '独立扩展芯片',
  'usb.role.unknown': 'USB 控制器',
  'usb.speed.low': 'USB 1.0 (1.5M)',
  'usb.speed.full': 'USB 1.1 (12M)',
  'usb.speed.high': 'USB 2.0 (480M)',
  'usb.speed.super': 'USB 3.x (5G)',
  'usb.speed.super_plus': 'USB 3.2 (10G+)',
  'usb.speed.unknown': '未知速率',
  'usb.cap.low': '1.5M',
  'usb.cap.full': '1.1',
  'usb.cap.high': '2.0',
  'usb.cap.super': '5G',
  'usb.cap.super_plus': '10G',
  'usb.cap.unknown': 'USB',
  'usb.port.name': 'Port {n}',
  'usb.port.companionHint': '复合物理接口（USB 3 + USB 2 配对端口 #{companion}）',
  'usb.port.empty': '空闲端口',
  'usb.port.noVisiblePorts': '当前控制器下没有已连接设备（打开上方「显示空闲端口」可查看全部接口）。',
  'usb.port.errorCode': 'Error {code}',
  'usb.port.disconnects24h': '24h 断连 {count} 次',
  'usb.port.changed': '刚刚变动',
  'usb.port.hubTag': 'USB Hub',
  'usb.port.labelAdd': '备注位置',
  'usb.port.labelPlaceholder': '例如：背面蓝口 #1 / 前面板左口',
  'usb.port.labelSave': '保存',
  'usb.port.labelCancel': '取消',
  'usb.section.events': '近期插拔与异常掉线记录',
  'usb.events.footer': '实时追踪应用运行期间的 USB 插拔，并自动汇总 Windows Kernel-PnP 近 24 小时意外断开记录。',
  'usb.events.empty': '暂无近期插拔或异常掉线记录。随意插拔一根 USB 线即可实时看到对应端口高亮。',
  'usb.event.arrived': '已插入',
  'usb.event.removed': '已拔出',
  'usb.event.surprise_removed': '异常掉线',
  'usb.empty.controllers': '正在扫描系统 USB 拓扑…',

  'general.title': '通用',
  'general.launchAtLogin': '登录时启动（最小化到托盘）',
  'general.language': '语言',
  'general.language.followSystem': '跟随系统',
  'general.language.zhHans': '简体中文',
  'general.language.en': 'English',
  'general.diagnostics': '诊断',
  'general.logTitle': '日志文件',
  'general.revealLog': '在资源管理器中显示',
  'general.logSubtitle': '遇到连接或切档问题时，可以把这个日志文件发给开发者定位原因。',

  'about.title': '关于',
  'about.versionFormat': '版本 {version}',
  'about.github': 'GitHub 仓库',
  'about.websiteDesc': '作者网站 —',
  'about.copyright': '© 2026 乔学诗。保留所有权利。',
};

const EN_STRINGS: Record<string, string> = {
  'nav.toggleSidebar': 'Toggle Sidebar',
  'status.ready': 'XTools Ready',
  'sidebar.general': 'General',
  'sidebar.about': 'About',

  'tool.rogkeyboard.title': 'ROG Keyboard',
  'rog.action.refresh': 'Refresh',
  'rog.section.status': 'Keyboard',
  'rog.status.connected': 'Connected',
  'rog.status.disconnected': 'Not on this PC',
  'rog.status.disconnected.hint':
    'Cannot reach the keyboard over USB or 2.4G right now — it is likely switched to Mac, Bluetooth, or unplugged.',
  'rog.status.currentProfile': 'Current Profile',
  'rog.status.battery': 'Battery',
  'rog.battery.charging': 'Charging',
  'rog.battery.full': 'Full',
  'rog.battery.split': 'L {left}% · R {right}%',
  'rog.status.link': 'Connection',
  'rog.status.firmware': 'Firmware',
  'rog.link.wired': 'USB Wired',
  'rog.link.wireless24g': '2.4G Wireless',
  'rog.link.bluetooth': 'Bluetooth',
  'rog.gearlink.title': 'Open ASUS Gear Link Web Driver',
  'rog.gearlink.subtitle': 'Configure keys, lighting, and on-board profiles in browser',

  'rog.section.profiles': 'Profile Assignment',
  'rog.profile.windows': 'Windows Profile',
  'rog.profile.default': 'Default Profile',
  'rog.profile.named': 'Profile {n}',
  'rog.action.switchToWindows': 'Switch to Windows',
  'rog.section.profiles.footer':
    'Choose the on-board profile to activate when the keyboard connects to Windows (default: Profile 1). You can also switch anytime from the system tray menu.',

  'rog.section.automation': 'Automation',
  'rog.auto.title': 'Auto-Switch on Windows Connect',
  'rog.auto.subtitle':
    'Automatically switch to the Windows profile on launch and whenever the keyboard reconnects over USB or 2.4G.',
  'rog.sound.title': 'Notification Sound',
  'rog.notify.denied':
    'Notifications for XTools are turned off in Windows. Enable them in Windows Settings → System → Notifications.',
  'rog.section.automation.footer':
    'XTools detects both USB arrival and 2.4G receiver reconnection and restores your Windows profile automatically.',

  'tool.razernaga.title': 'Razer Naga',
  'naga.section.status': 'Mouse Status',
  'naga.status.connected': 'Connected',
  'naga.status.disconnected': 'Not Detected',
  'naga.status.disconnected.hint':
    'Cannot reach Razer Naga V3 Pro over USB cable or 2.4G HyperSpeed dongle right now. Editing onboard memory requires USB or 2.4G, and saved bindings keep working over Bluetooth or without Synapse.',
  'naga.status.battery': 'Battery',
  'naga.battery.charging': 'Charging',
  'naga.battery.full': 'Full',
  'naga.status.link': 'Connection',
  'naga.status.activeProfile': 'Active Profile',
  'naga.status.dpi': 'Active DPI',
  'naga.status.pollingRate': 'Polling Rate',
  'naga.status.firmware': 'Firmware',
  'naga.link.wired': 'USB Wired',
  'naga.link.wireless24g': '2.4G Wireless',

  'naga.section.profileDpi': 'Onboard Profile & Sensitivity',
  'naga.profile.title': 'Onboard Profile Slot',
  'naga.profile.slot': 'Profile {n}',
  'naga.profile.deleteHint': 'Delete Profile {n}',
  'naga.dpi.title': 'DPI Sensitivity Stages',
  'naga.dpi.stage': 'Stage {n}',
  'naga.dpi.addStage': 'Add Stage',
  'naga.dpi.removeStage': 'Remove Stage',
  'naga.dpi.apply': 'Apply DPI',
  'naga.polling.title': 'Polling Rate',
  'naga.profileDpi.footer':
    'Supports onboard Profile 1–5 (matching the bottom LED colors). Clicking an unused slot creates and activates it; DPI and polling rate changes write straight to mouse hardware memory.',

  'naga.section.mappings': 'Side-Plate Onboard Keymap',
  'naga.plate.title': 'Side Plate Layout',
  'naga.plate.two_button': '2-Button Plate',
  'naga.plate.six_button': '6-Button Plate',
  'naga.plate.twelve_button': '12-Button Plate',
  'naga.plate.reset': 'Reset Defaults',
  'naga.bind.recording': 'Press key or combo (Esc cancels)…',
  'naga.bind.empty': 'Disconnected',
  'naga.bind.unsupportedKey': 'That key is not supported for mouse onboard mapping.',
  'naga.action.disabled': 'Disabled',
  'naga.action.hypershift': 'Razer HyperShift',
  'naga.action.profile': 'Switch Profile',
  'naga.action.mouse.left': 'Left Click',
  'naga.action.mouse.right': 'Right Click',
  'naga.action.mouse.middle': 'Middle Click',
  'naga.action.mouse.back': 'Mouse Back (X1)',
  'naga.action.mouse.forward': 'Mouse Forward (X2)',
  'naga.action.mouse.scroll_up': 'Scroll Up',
  'naga.action.mouse.scroll_down': 'Scroll Down',
  'naga.action.dpi.up': 'DPI Up',
  'naga.action.dpi.down': 'DPI Down',
  'naga.action.dpi.cycle_up': 'DPI Cycle (+)',
  'naga.action.dpi.cycle_down': 'DPI Cycle (-)',
  'naga.action.type.keyboard': 'Keyboard Key…',
  'naga.action.type.disabled': 'Disable Button',
  'naga.action.type.other': 'Other Action',
  'naga.mappings.footer':
    'Click a button label and press any key or shortcut to flash it directly into the mouse onboard memory via Razer HID protocol. Once written, mappings work natively without Synapse, XTools, or background hooks.',

  'naga.section.test': 'Local Key Test Box',
  'naga.test.placeholder': 'Click here and press any side button to verify hardware output…',
  'naga.test.lastKey': 'Last input',
  'naga.test.clear': 'Clear',

  'tool.hypercapslock.title': 'HyperCapslock',
  'hyper.section.service': 'Service',
  'hyper.status.running': 'Running',
  'hyper.status.paused': 'Paused',
  'hyper.status.subtitle':
    'Turns CapsLock into a Hyper modifier key (short tap ≤200ms still toggles CapsLock; hold with keys to navigate & trigger actions).',
  'hyper.action.pause': 'Pause',
  'hyper.action.resume': 'Resume',
  'hyper.action.reset': 'Reset Defaults',
  'hyper.action.import': 'Import',
  'hyper.action.export': 'Export',
  'hyper.action.add': 'Add Mapping',
  'hyper.section.mappings': 'Action Mappings',
  'hyper.mappings.empty': 'No action mappings configured yet.',
  'hyper.group.directional': 'Directional',
  'hyper.group.jump': 'Jump',
  'hyper.group.independent': 'Independent',
  'hyper.group.command': 'Command',
  'hyper.group.key_combo': 'Key Combo',
  'hyper.action.left': 'Move Left',
  'hyper.action.right': 'Move Right',
  'hyper.action.up': 'Move Up',
  'hyper.action.down': 'Move Down',
  'hyper.action.word_forward': 'Word Forward (Ctrl+Right)',
  'hyper.action.word_back': 'Word Back (Ctrl+Left)',
  'hyper.action.home': 'Line Start (Home)',
  'hyper.action.end': 'Line End (End)',
  'hyper.action.jump_up': 'Jump Up ×{count}',
  'hyper.action.jump_down': 'Jump Down ×{count}',
  'hyper.action.backspace': 'Backspace',
  'hyper.action.next_line': 'Next Line (End+Enter)',
  'hyper.action.insert_quotes': 'Insert Triple Quotes ("""""")',
  'hyper.action.toggle_caps_lock': 'Toggle CapsLock',
  'hyper.action.switch_input_source': 'Switch Input Method (Win+Space)',
  'hyper.trigger.single_tap': 'Caps ×1',
  'hyper.trigger.double_tap': 'Caps ×2',
  'hyper.trigger.caps': 'Caps',
  'hyper.trigger.caps_shift': 'Caps + Shift',
  'hyper.trigger.dtm': 'Double-tap Modifier',
  'hyper.mod.left_shift': 'L-Shift ×2',
  'hyper.mod.right_shift': 'R-Shift ×2',
  'hyper.mod.left_control': 'L-Ctrl ×2',
  'hyper.mod.right_control': 'R-Ctrl ×2',
  'hyper.mod.left_alt': 'L-Alt ×2',
  'hyper.mod.right_alt': 'R-Alt ×2',
  'hyper.mod.left_win': 'L-Win ×2',
  'hyper.mod.right_win': 'R-Win ×2',
  'hyper.modal.addTitle': 'Add Mapping',
  'hyper.modal.editTitle': 'Edit Mapping',
  'hyper.modal.triggerType': 'Trigger',
  'hyper.modal.triggerKey': 'Press trigger key',
  'hyper.modal.actionType': 'Action',
  'hyper.modal.targetKey': 'Press target key',
  'hyper.modal.commandPlaceholder': 'e.g. notepad.exe or explorer.exe',
  'hyper.modal.jumpCount': 'Lines',
  'hyper.modal.save': 'Save',
  'hyper.modal.cancel': 'Cancel',
  'hyper.importSuccess': 'Imported {count} mapping(s).',
  'hyper.importFailed': 'Import failed: {error}',
  'hyper.status.autoPaused': 'Auto-paused',
  'hyper.status.autoPausedBy': 'HyperCapslock is auto-paused (low-level keyboard hook uninstalled) because {processes} is running. It resumes automatically once all of them exit.',
  'hyper.autoPause.section': 'Auto-Pause on Processes',
  'hyper.autoPause.footer':
    'While any listed process is running, HyperCapslock pauses itself and uninstalls the keyboard hook to stay anti-cheat safe; it resumes once all of them exit. If you paused it manually, it will not be auto-resumed.',
  'hyper.autoPause.enable.title': 'Enable Process Auto-Pause',
  'hyper.autoPause.enable.subtitle': 'Pause automatically and uninstall the keyboard hook while the processes below are running.',
  'hyper.autoPause.sound.title': 'Notification Sound',
  'hyper.autoPause.input.placeholder': 'Process name, e.g. steam.exe or cs2',
  'hyper.autoPause.add': 'Add',
  'hyper.autoPause.pick': 'Pick Running',
  'hyper.autoPause.empty': 'No processes yet. Type a process name or pick one from the running processes.',
  'hyper.autoPause.running': 'Running',
  'hyper.autoPause.remove': 'Remove',
  'hyper.autoPause.picker.title': 'Select a Running Process',
  'hyper.autoPause.picker.search': 'Search processes…',
  'hyper.autoPause.picker.refresh': 'Refresh',
  'hyper.autoPause.picker.added': 'Added',
  'hyper.autoPause.picker.empty': 'No matching processes.',
  'hyper.autoPause.picker.close': 'Close',
  'hyper.autoPause.picker.loading': 'Loading running processes…',
  'hyper.autoPause.picker.selectedCount': '{count} selected',
  'hyper.autoPause.picker.done': 'Done',

  'tool.usbinspector.title': 'USB Ports',
  'usb.section.overview': 'Overview & Controls',
  'usb.overview.summary': '{devices} connected · {controllers} controllers',
  'usb.overview.healthy': 'Healthy',
  'usb.overview.issues': '{count} notice(s)',
  'usb.overview.scanning': 'Scanning…',
  'usb.overview.lastScan': 'Updated {time}',
  'usb.showEmpty.title': 'Show Empty Ports',
  'usb.showEmpty.subtitle': 'Show unoccupied physical USB sockets to inspect speeds and set location labels',
  'usb.section.diagnostics': 'Diagnostics & Tips',
  'usb.healthy.banner': 'All USB controllers and connected ports look healthy with no recent surprise-disconnect spikes.',
  'usb.role.cpu_direct': 'CPU Direct',
  'usb.role.chipset': 'Motherboard Chipset',
  'usb.role.discrete': 'Discrete Controller',
  'usb.role.unknown': 'USB Controller',
  'usb.speed.low': 'USB 1.0 (1.5M)',
  'usb.speed.full': 'USB 1.1 (12M)',
  'usb.speed.high': 'USB 2.0 (480M)',
  'usb.speed.super': 'USB 3.x (5G)',
  'usb.speed.super_plus': 'USB 3.2 (10G+)',
  'usb.speed.unknown': 'Unknown',
  'usb.cap.low': '1.5M',
  'usb.cap.full': '1.1',
  'usb.cap.high': '2.0',
  'usb.cap.super': '5G',
  'usb.cap.super_plus': '10G',
  'usb.cap.unknown': 'USB',
  'usb.port.name': 'Port {n}',
  'usb.port.companionHint': 'Combined physical socket (paired with companion port #{companion})',
  'usb.port.empty': 'Empty Port',
  'usb.port.noVisiblePorts': 'No occupied ports under this controller (enable "Show Empty Ports" above to view free sockets).',
  'usb.port.errorCode': 'Error {code}',
  'usb.port.disconnects24h': '{count} drops / 24h',
  'usb.port.changed': 'Just Changed',
  'usb.port.hubTag': 'USB Hub',
  'usb.port.labelAdd': 'Label',
  'usb.port.labelPlaceholder': 'e.g. Rear Blue #1 / Front Left',
  'usb.port.labelSave': 'Save',
  'usb.port.labelCancel': 'Cancel',
  'usb.section.events': 'Recent Hotplug & Surprise Disconnects',
  'usb.events.footer': 'Tracks live USB hotplugs while XTools runs and pulls 24h surprise removals from Windows Kernel-PnP.',
  'usb.events.empty': 'No recent hotplug or disconnect events yet. Plug or unplug any USB cable to highlight its port live.',
  'usb.event.arrived': 'Plugged In',
  'usb.event.removed': 'Unplugged',
  'usb.event.surprise_removed': 'Dropped',
  'usb.empty.controllers': 'Scanning Windows USB topology…',

  'general.title': 'General',
  'general.launchAtLogin': 'Launch at Login (Minimized to Tray)',
  'general.language': 'Language',
  'general.language.followSystem': 'Follow System',
  'general.language.zhHans': '简体中文',
  'general.language.en': 'English',
  'general.diagnostics': 'Diagnostics',
  'general.logTitle': 'Log File',
  'general.revealLog': 'Reveal in Explorer',
  'general.logSubtitle': 'Share this log file if you run into connection or profile switching issues.',

  'about.title': 'About',
  'about.versionFormat': 'Version {version}',
  'about.github': 'GitHub Repository',
  'about.websiteDesc': 'Author website —',
  'about.copyright': '© 2026 Xueshi Qiao. All rights reserved.',
};

const SPECIAL_VK_NAMES: Record<number, string> = {
  8: 'Backspace',
  9: 'Tab',
  13: 'Enter',
  19: 'Pause',
  20: 'CapsLock',
  27: 'Esc',
  32: 'Space',
  33: 'PgUp',
  34: 'PgDn',
  35: 'End',
  36: 'Home',
  37: '←',
  38: '↑',
  39: '→',
  40: '↓',
  44: 'PrtSc',
  45: 'Ins',
  46: 'Del',
  91: 'L-Win',
  92: 'R-Win',
  106: 'Num *',
  107: 'Num +',
  109: 'Num -',
  110: 'Num .',
  111: 'Num /',
  144: 'NumLock',
  145: 'ScrollLock',
  160: 'L-Shift',
  161: 'R-Shift',
  162: 'L-Ctrl',
  163: 'R-Ctrl',
  164: 'L-Alt',
  165: 'R-Alt',
  186: ';',
  187: '=',
  188: ',',
  189: '-',
  190: '.',
  191: '/',
  192: '`',
  219: '[',
  220: '\\',
  221: ']',
  222: "'",
};

export class I18nManager {
  private language: EffectiveLanguage;

  constructor(language: EffectiveLanguage = 'zh-Hans') {
    this.language = language;
  }

  public setLanguage(language: EffectiveLanguage): void {
    this.language = language;
  }

  public get isZh(): boolean {
    return this.language === 'zh-Hans';
  }

  public t(key: string, vars?: Record<string, string | number>): string {
    const dict = this.isZh ? ZH_STRINGS : EN_STRINGS;
    let text = dict[key] ?? key;
    if (vars) {
      for (const [k, v] of Object.entries(vars)) {
        text = text.replace(`{${k}}`, String(v));
      }
    }
    return text;
  }

  /** Ordered wire slots matching ASUS Gear Link: Default (last slot) first, then 1..N-1. */
  public orderedProfileSlots(count: number): number[] {
    const total = Math.max(count, 1);
    if (total <= 1) return [total];
    const slots = [total];
    for (let i = 1; i < total; i++) {
      slots.push(i);
    }
    return slots;
  }

  public profileLabel(wire: number, count: number): string {
    const defaultSlot = Math.max(count, 1);
    if (wire === defaultSlot) {
      return this.t('rog.profile.default');
    }
    return this.t('rog.profile.named', { n: wire });
  }

  public linkLabel(link: RogLink | null): string {
    if (!link) return '—';
    return this.t(`rog.link.${link}`);
  }

  public nagaLinkLabel(link: NagaLink | null): string {
    if (!link) return '—';
    return this.t(`naga.link.${link}`);
  }

  public nagaPlateLabel(layout: NagaPlateLayout): string {
    return this.t(`naga.plate.${layout}`);
  }

  public formatNagaAction(action: NagaHardwareAction): string {
    switch (action.kind) {
      case 'disabled':
        return this.t('naga.action.disabled');
      case 'mouse':
        return this.t(`naga.action.mouse.${action.button}`);
      case 'keyboard': {
        const parts: string[] = [];
        if (action.ctrl) parts.push('Ctrl');
        if (action.alt) parts.push('Alt');
        if (action.shift) parts.push('Shift');
        if (action.win) parts.push('Win');
        const keyLabel =
          action.vk > 0
            ? this.vkName(action.vk)
            : `HID 0x${action.hid_code.toString(16).toUpperCase().padStart(2, '0')}`;
        parts.push(keyLabel);
        return parts.join('+');
      }
      case 'dpi':
        return this.t(`naga.action.dpi.${action.op}`);
      case 'profile':
        return `${this.t('naga.action.profile')} (${action.op})`;
      case 'media':
        return `Media 0x${action.usage.toString(16).toUpperCase().padStart(4, '0')}`;
      case 'hypershift':
        return this.t('naga.action.hypershift');
      case 'raw':
        return `Raw 0x${action.action_type.toString(16).toUpperCase().padStart(2, '0')}`;
    }
  }

  public vkName(vk: number): string {
    if (SPECIAL_VK_NAMES[vk]) return SPECIAL_VK_NAMES[vk];
    if (vk >= 48 && vk <= 57) return String.fromCharCode(vk);
    if (vk >= 65 && vk <= 90) return String.fromCharCode(vk);
    if (vk >= 0x60 && vk <= 0x69) return `Num ${vk - 0x60}`;
    if (vk >= 0x70 && vk <= 0x87) return `F${vk - 0x6f}`;
    return `VK 0x${vk.toString(16).toUpperCase().padStart(2, '0')}`;
  }

  public formatShortcut(sc: KeyShortcut): string {
    const parts: string[] = [];
    if (sc.ctrl) parts.push('Ctrl');
    if (sc.alt) parts.push('Alt');
    if (sc.shift) parts.push('Shift');
    if (sc.win) parts.push('Win');
    parts.push(this.vkName(sc.vk));
    return parts.join('+');
  }
}
