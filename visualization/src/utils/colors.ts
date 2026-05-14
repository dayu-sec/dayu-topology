import type { LayerKind, NodeKind } from '../types/domain';
import { LAYER_NODE_KINDS } from '../types/domain';

type LayerStyle = {
  bg: string;
  border: string;
  label: string;
};

export const LAYER_COLORS: Record<LayerKind, LayerStyle> = {
  resource:   { bg: '#D1FAE5', border: '#10B981', label: '资源目录' },
  governance: { bg: '#EDE9FE', border: '#8B5CF6', label: '责任治理' },
};

export const NODE_SHAPES: Record<NodeKind, string> = {
  HostInventory:  'diamond',
  NetworkSegment: 'hexagon',
  ProcessRuntime: 'round-rectangle',
  ProcessSummary: 'round-hexagon',
  ProcessGroup:   'ellipse',
  Subject:        'ellipse',
};

export function nodeKindToLayer(kind: NodeKind): LayerKind {
  for (const [layer, kinds] of Object.entries(LAYER_NODE_KINDS)) {
    if ((kinds as readonly NodeKind[]).includes(kind)) return layer as LayerKind;
  }
  return 'resource';
}

export function getLayerStyle(kind: NodeKind): LayerStyle {
  return LAYER_COLORS[nodeKindToLayer(kind)];
}
