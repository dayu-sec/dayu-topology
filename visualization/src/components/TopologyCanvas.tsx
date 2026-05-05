import { useEffect, useRef } from 'react';
import cytoscape, { type Core } from 'cytoscape';
import dagre from 'cytoscape-dagre';
import coseBilkent from 'cytoscape-cose-bilkent';
import type { TopologyGraph, LayoutName } from '../types/domain';
import { graphToCyElements } from '../utils/graph-to-cy';
import { LAYER_COLORS } from '../utils/colors';

cytoscape.use(dagre);
cytoscape.use(coseBilkent);

type Props = {
  graph: TopologyGraph | null;
  selectedNodeId: string | null;
  onSelectNode: (nodeId: string | null) => void;
  searchQuery: string;
  layout: LayoutName;
};

const LAYOUTS: Record<LayoutName, cytoscape.LayoutOptions> = {
  dagre: { name: 'dagre', rankDir: 'LR' } as cytoscape.LayoutOptions,
  'cose-bilkent': { name: 'cose-bilkent', idealEdgeLength: 100, nodeRepulsion: 4000 } as cytoscape.LayoutOptions,
};

export default function TopologyCanvas({ graph, selectedNodeId, onSelectNode, searchQuery, layout }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const cyRef = useRef<Core | null>(null);

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
            'label': 'data(label)',
            'text-valign': 'bottom',
            'text-halign': 'center',
            'font-size': '11px',
            'color': '#333',
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
            'label': 'data(label)',
            'font-size': '9px',
            'color': '#999',
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

    cy.on('tap', (evt) => {
      if (evt.target === cy) {
        onSelectNode(null);
      }
    });

    cy.on('tap', 'node', (evt) => {
      onSelectNode(evt.target.id());
    });

    cyRef.current = cy;

    return () => {
      cy.destroy();
      cyRef.current = null;
    };
  }, [onSelectNode]);

  // Update elements when graph data changes.
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy || !graph) return;
    cy.elements().remove();
    cy.add(graphToCyElements(graph));
    cy.layout(LAYOUTS[layout]).run();
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

    cy.nodes().forEach((node) => {
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
          Query a host to load its topology graph.
        </div>
      )}
    </div>
  );
}
