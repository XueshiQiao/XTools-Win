import { Command } from 'lucide-react';
import { ToolModule } from '../../core/toolRegistry';
import { HyperCapslockView } from './HyperCapslockView';

export const hyperCapslockTool: ToolModule = {
  id: 'hyper-capslock',
  group: 'utilities',
  titleKey: 'tool.hypercapslock.title',
  color: '#0A84FF',
  renderIcon: (size) => <Command size={size} strokeWidth={2.1} />,
  renderView: (props) => <HyperCapslockView {...props} />,
};
