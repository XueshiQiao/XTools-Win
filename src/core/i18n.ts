import { EffectiveLanguage, RogLink } from './types';

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
  'rog.profile.mac': 'Mac 档',
  'rog.profile.default': '默认 配置文件',
  'rog.profile.named': '配置文件 {n}',
  'rog.action.switchToWindows': '切到 Windows 档',
  'rog.action.switchToMac': '切到 Mac 档',
  'rog.section.profiles.footer':
    '指定哪个板载档位代表哪台电脑。默认连上 Windows 时会自动切换到「Windows 档」（默认为 配置文件 1）。托盘右键菜单里也有同样的两个快捷开关，方便你在切回 Mac 之前手动切好。',

  'rog.section.automation': '自动化',
  'rog.auto.title': '连上 Windows 时自动切换',
  'rog.auto.subtitle': '检测到键盘重新连上这台 Windows（USB 有线或 2.4G）以及软件启动时，自动切到 Windows 档。',
  'rog.sound.title': '通知带提示音',
  'rog.notify.denied':
    'XTools 的系统通知被关掉了，切档时不会有横幅提示。可在「Windows 设置 → 系统 → 通知」里打开。',
  'rog.section.automation.footer':
    '键盘连上 Windows（无论 USB 拔插还是拨回 2.4G 接收器）都会被自动识别并还原为 Windows 档；若要切换回 Mac 档，建议在拨动键盘开关离开 Windows 前，点击上方按钮或系统托盘菜单切换。',

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
  'rog.profile.mac': 'Mac Profile',
  'rog.profile.default': 'Default Profile',
  'rog.profile.named': 'Profile {n}',
  'rog.action.switchToWindows': 'Switch to Windows',
  'rog.action.switchToMac': 'Switch to Mac',
  'rog.section.profiles.footer':
    'Choose which on-board slot belongs to each computer. When the keyboard connects to Windows, it switches to the Windows slot (default: Profile 1). Both switches also live in the system tray menu so you can switch back before flipping the keyboard switch to Mac.',

  'rog.section.automation': 'Automation',
  'rog.auto.title': 'Auto-Switch on Windows Connect',
  'rog.auto.subtitle':
    'Automatically switch to the Windows profile on launch and whenever the keyboard reconnects over USB or 2.4G.',
  'rog.sound.title': 'Notification Sound',
  'rog.notify.denied':
    'Notifications for XTools are turned off in Windows. Enable them in Windows Settings → System → Notifications.',
  'rog.section.automation.footer':
    'XTools detects both USB arrival and 2.4G receiver reconnection and applies your Windows profile automatically. To switch back to your Mac profile before leaving Windows, use the button above or the tray menu.',

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
}
