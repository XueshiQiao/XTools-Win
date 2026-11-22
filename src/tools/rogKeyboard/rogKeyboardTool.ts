import React from 'react';
import { Keyboard } from 'lucide-react';
import { ToolModule, ToolRenderProps } from '../../core/toolRegistry';
import { RogKeyboardView } from './RogKeyboardView';

export const rogKeyboardTool: ToolModule = {
  id: 'rog-keyboard',
  group: 'devices',
  titleKey: 'tool.rogkeyboard.title',
  color: '#5856D6',
  renderIcon: (size: number) => React.createElement(Keyboard, { size, strokeWidth: 2.1 }),
  renderView: (props: ToolRenderProps) => React.createElement(RogKeyboardView, props),
};
