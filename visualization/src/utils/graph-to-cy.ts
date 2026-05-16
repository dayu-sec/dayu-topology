import type { ElementDefinition } from 'cytoscape';
import type { HostProcessTopologyGraph, CyNodeData, CyEdgeData } from '../types/domain';

export function graphToCyElements(graph: HostProcessTopologyGraph): ElementDefinition[] {
  const nodes: ElementDefinition[] = graph.nodes.map((n) => ({
    data: {
      id: n.id,
      objectKind: n.objectKind,
      layer: n.layer,
      label: n.label,
      properties: n.properties,
    } satisfies CyNodeData,
    classes: `layer-${n.layer}`,
  }));

  const edges: ElementDefinition[] = graph.edges.map((e) => ({
    data: {
      id: e.id,
      source: e.source,
      target: e.target,
      edgeKind: e.edgeKind,
      label: e.label ?? '',
      properties: e.properties ?? {},
    } satisfies CyEdgeData,
  }));

  return [...nodes, ...edges];
}
