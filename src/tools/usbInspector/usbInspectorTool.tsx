import React from 'react';
import { Usb } from 'lucide-react';
import { ToolModule, ToolRenderProps } from '../../core/toolRegistry';
import { UsbInspectorView } from './UsbInspectorView';

export const usbInspectorTool: ToolModule = {
  id: 'usb-inspector',
  group: 'utilities',
  titleKey: 'tool.usbinspector.title',
  color: '#30B0C7',
  renderIcon: (size: number) => React.createElement(Usb, { size, strokeWidth: 2.1 }),
  renderView: (props: ToolRenderProps) => React.createElement(UsbInspectorView, props),
};
