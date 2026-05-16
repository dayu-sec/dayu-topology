import { useEffect, useRef } from 'react';
import cytoscape, { type Core } from 'cytoscape';
import dagre from 'cytoscape-dagre';
import coseBilkent from 'cytoscape-cose-bilkent';
import type { HostProcessTopologyGraph, LayoutName } from '../types/domain';
import { graphToCyElements } from '../utils/graph-to-cy';
import { LAYER_COLORS, NODE_SHAPES } from '../utils/colors';

cytoscape.use(dagre);
cytoscape.use(coseBilkent);

type CytoscapeNodeShape =
  | 'rectangle'
  | 'roundrectangle'
  | 'ellipse'
  | 'triangle'
  | 'pentagon'
  | 'hexagon'
  | 'heptagon'
  | 'octagon'
  | 'star'
  | 'barrel'
  | 'diamond'
  | 'vee'
  | 'rhomboid'
  | 'polygon'
  | 'tag'
  | 'round-rectangle'
  | 'round-triangle'
  | 'round-diamond'
  | 'round-pentagon'
  | 'round-hexagon'
  | 'round-heptagon'
  | 'round-octagon'
  | 'round-tag'
  | 'cut-rectangle'
  | 'bottom-round-rectangle'
  | 'concave-hexagon';

type Props = {
  graph: HostProcessTopologyGraph | null;
  selectedNodeId: string | null;
  onSelectNode: (nodeId: string | null) => void;
  searchQuery: string;
  layout: LayoutName;
};

const LAYOUTS: Record<LayoutName, cytoscape.LayoutOptions> = {
  concentric: {
    name: 'concentric',
    minNodeSpacing: 12,
    spacingFactor: 1.1,
    concentric: (node: cytoscape.NodeSingular) => {
      const kind = node.data('objectKind') as string;
      return kind === 'HostInventory' ? 2 : 1;
    },
    levelWidth: () => 1,
  } as cytoscape.LayoutOptions,
  dagre: { name: 'dagre', rankDir: 'LR' } as cytoscape.LayoutOptions,
  'cose-bilkent': { name: 'cose-bilkent', idealEdgeLength: 100, nodeRepulsion: 4000 } as cytoscape.LayoutOptions,
};

export default function TopologyCanvas({ graph, selectedNodeId, onSelectNode, searchQuery, layout }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const cyRef = useRef<Core | null>(null);
  const onSelectNodeRef = useRef(onSelectNode);

  useEffect(() => {
    onSelectNodeRef.current = onSelectNode;
  }, [onSelectNode]);

  // Initialize Cytoscape instance once.
  useEffect(() => {
    if (!containerRef.current || cyRef.current) return;

    const cy = cytoscape({
      container: containerRef.current,
      style: [
        {
          selector: 'node',
          style: {
            'background-color': (el) => {
              const layer = el.data('layer') as string;
              return LAYER_COLORS[layer as keyof typeof LAYER_COLORS]?.bg ?? '#eee';
            },
            'border-color': (el) => {
              const layer = el.data('layer') as string;
              return LAYER_COLORS[layer as keyof typeof LAYER_COLORS]?.border ?? '#999';
            },
            'border-width': 2,
            'shape': (el) => {
              const objectKind = el.data('objectKind') as keyof typeof NODE_SHAPES;
              return (NODE_SHAPES[objectKind] ?? 'ellipse') as CytoscapeNodeShape;
            },
            'width': (el: cytoscape.NodeSingular) => {
              const objectKind = el.data('objectKind') as string;
              if (objectKind === 'HostInventory') return 72;
              if (objectKind === 'ProcessSummary') return 48;
              if (objectKind === 'ProcessGroup') return 42;
              return 26;
            },
            'height': (el: cytoscape.NodeSingular) => {
              const objectKind = el.data('objectKind') as string;
              if (objectKind === 'HostInventory') return 72;
              if (objectKind === 'ProcessSummary') return 48;
              if (objectKind === 'ProcessGroup') return 42;
              return 26;
            },
            'label': 'data(label)',
            'text-valign': 'bottom',
            'text-halign': 'center',
            'font-size': (el: cytoscape.NodeSingular) => {
              const objectKind = el.data('objectKind') as string;
              if (objectKind === 'HostInventory') return '14px';
              if (objectKind === 'ProcessSummary') return '11px';
              if (objectKind === 'ProcessGroup') return '10px';
              return '9px';
            },
            'color': '#333',
            'text-wrap': 'wrap',
            'text-max-width': (el: cytoscape.NodeSingular) => {
              const objectKind = el.data('objectKind') as string;
              if (objectKind === 'HostInventory') return '140px';
              if (objectKind === 'ProcessSummary') return '120px';
              if (objectKind === 'ProcessGroup') return '110px';
              return '72px';
            },
          },
        },
        {
          selector: 'edge',
          style: {
            'width': 1.5,
            'line-color': '#bbb',
            'target-arrow-color': '#bbb',
            'target-arrow-shape': 'triangle',
            'curve-style': 'bezier',
            'label': '',
            'font-size': '9px',
            'color': '#999',
          },
        },
        {
          selector: 'edge[edgeKind = "host_process_assoc"]',
          style: {
            'line-color': '#60a5fa',
            'target-arrow-color': '#60a5fa',
            'target-arrow-shape': 'none',
            'curve-style': 'straight',
            'width': 1,
          },
        },
        {
          selector: 'node[objectKind = "ProcessSummary"]',
          style: {
            'background-color': '#dbeafe',
            'border-color': '#2563eb',
            'border-width': 3,
          },
        },
        {
          selector: 'node[objectKind = "ProcessGroup"]',
          style: {
            'background-color': '#e0f2fe',
            'border-color': '#0284c7',
            'border-width': 2,
          },
        },
        {
          selector: 'node[objectKind = "HostInventory"]',
          style: {
            'background-color': '#fef3c7',
            'border-color': '#f59e0b',
            'border-width': 4,
          },
        },
        {
          selector: 'node:selected',
          style: {
            'border-width': 3,
            'border-color': '#f59e0b',
          },
        },
        {
          selector: '.dimmed',
          style: {
            'opacity': 0.15,
          },
        },
      ],
      layout: LAYOUTS.dagre,
      wheelSensitivity: 0.3,
    });

    cy.on('tap', (evt: cytoscape.EventObject) => {
      if (evt.target === cy) {
        onSelectNodeRef.current(null);
      }
    });

    cy.on('tap', 'node', (evt: cytoscape.EventObject) => {
      onSelectNodeRef.current(evt.target.id());
    });

    cyRef.current = cy;

    return () => {
      cy.destroy();
      cyRef.current = null;
    };
  }, []);

  // Update elements when graph data changes.
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy || !graph) return;
    cy.elements().remove();
    cy.add(graphToCyElements(graph));
    const effectiveLayout = chooseEffectiveLayout(graph, layout);
    cy.layout(LAYOUTS[effectiveLayout]).run();
    cy.fit(undefined, 50);
  }, [graph, layout]);

  // Highlight selected node and its neighborhood.
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy) return;
    cy.elements().removeClass('highlighted');
    if (selectedNodeId) {
      const node = cy.getElementById(selectedNodeId);
      if (node.length) {
        node.addClass('highlighted');
        node.neighborhood().addClass('highlighted');
      }
    }
  }, [selectedNodeId]);

  // Search highlight: dim nodes not matching the query.
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy || !graph) return;

    const q = searchQuery.trim().toLowerCase();
    if (!q) {
      cy.elements().removeClass('dimmed');
      return;
    }

    cy.nodes().forEach((node: cytoscape.NodeSingular) => {
      const label = (node.data('label') ?? '') as string;
      const props = (node.data('properties') ?? {}) as Record<string, unknown>;
      const values = [label, ...Object.values(props).map(String)];
      const match = values.some((v) => v.toLowerCase().includes(q));
      if (!match) node.addClass('dimmed');
      else node.removeClass('dimmed');
    });
  }, [searchQuery, graph]);

  return (
    <div className="relative w-full h-full">
      <div ref={containerRef} className="w-full h-full" />
      {!graph && (
        <div className="absolute inset-0 flex items-center justify-center text-gray-400 pointer-events-none">
          Load topology data to view hosts and aggregated process summaries.
        </div>
      )}
    </div>
  );
}

function chooseEffectiveLayout(
  graph: HostProcessTopologyGraph,
  requestedLayout: LayoutName,
): LayoutName {
  if (requestedLayout !== 'dagre') {
    return requestedLayout;
  }

  const processSummaryCount = graph.nodes.filter((node) => node.objectKind === 'ProcessSummary').length;
  const processGroupCount = graph.nodes.filter((node) => node.objectKind === 'ProcessGroup').length;
  const processRuntimeCount = graph.nodes.filter((node) => node.objectKind === 'ProcessRuntime').length;

  const looksLikeFullGroupsFanout =
    processSummaryCount >= 1 &&
    processGroupCount >= 16 &&
    processRuntimeCount === 0;

  if (looksLikeFullGroupsFanout) {
    return 'cose-bilkent';
  }

  return requestedLayout;
}
