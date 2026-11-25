import { Mouse } from 'lucide-react';
import { ToolModule } from '../../core/toolRegistry';
import { RazerNagaView } from './RazerNagaView';

export const razerNagaTool: ToolModule = {
  id: 'razer-naga',
  group: 'devices',
  titleKey: 'tool.razernaga.title',
  color: '#44D62C',
  renderIcon: (size) => <Mouse size={size} strokeWidth={2.1} />,
  renderView: (props) => <RazerNagaView {...props} />,
};
