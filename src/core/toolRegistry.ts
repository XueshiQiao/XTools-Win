import React from 'react';
import { I18nManager } from './i18n';
import {
  HyperCapslockSnapshot,
  NagaSnapshot,
  RogKeyboardSnapshot,
} from './types';

export type ToolGroup = 'devices' | 'utilities';

export interface ToolRenderProps {
  i18n: I18nManager;
  rogState: RogKeyboardSnapshot;
  onUpdateRogState: (next: RogKeyboardSnapshot) => void;
  nagaState: NagaSnapshot;
  onUpdateNagaState: (next: NagaSnapshot) => void;
  hyperState: HyperCapslockSnapshot;
  onUpdateHyperState: (next: HyperCapslockSnapshot) => void;
}

export interface ToolModule {
  id: string;
  group: ToolGroup;
  titleKey: string;
  color: string;
  renderIcon: (size: number) => React.ReactNode;
  renderView: (props: ToolRenderProps) => React.ReactNode;
}

export class ToolRegistry {
  private readonly tools: ToolModule[];

  constructor(tools: ToolModule[]) {
    this.tools = tools;
  }

  public getAll(): ToolModule[] {
    return this.tools;
  }

  public getById(id: string): ToolModule | undefined {
    return this.tools.find((t) => t.id === id);
  }

  public getGroups(): Array<{ group: ToolGroup; items: ToolModule[] }> {
    const order: ToolGroup[] = ['devices', 'utilities'];
    return order
      .map((group) => ({
        group,
        items: this.tools.filter((t) => t.group === group),
      }))
      .filter((entry) => entry.items.length > 0);
  }
}
