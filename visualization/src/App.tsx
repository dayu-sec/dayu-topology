import { useEffect, useState, useCallback } from 'react';
import { useAppState } from './hooks/useAppState';
import TopologyCanvas from './components/TopologyCanvas';
import FilterBar from './components/FilterBar';
import LayerLegend from './components/LayerLegend';
import NodeDetailPanel from './components/NodeDetailPanel';
import {
  fetchFirstHostProcessTopology,
  fetchHostProcessTopology,
  fetchHostTopology,
  fetchNetworkTopology,
  loadFixture,
} from './api/client';
import type {
  HostProcessTopologyEdge,
  HostProcessTopologyGraph,
  HostProcessTopologyNode,
} from './types/domain';

const PROCESS_SUMMARY_PREFIX = 'process-summary:';
const PROCESS_GROUP_PREFIX = 'process-group:';
const PROCESS_EXPAND_LIMIT = 20;
const DEFAULT_HOST_ID = import.meta.env.VITE_DEFAULT_HOST_ID as string | undefined;
const USE_FIXTURE = import.meta.env.VITE_USE_FIXTURE === '1';

export default function App() {
  const [state, dispatch] = useAppState();
  const [hostId, setHostId] = useState('');
  const [networkId, setNetworkId] = useState('');
  const [expandedHosts, setExpandedHosts] = useState<Set<string>>(new Set());
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());

  const loadData = useCallback(() => {
    dispatch({ type: 'FETCH_GRAPH_START' });
    const done = (res: Awaited<ReturnType<typeof fetchHostProcessTopology>>) => {
      if (res.status === 'ok') {
        setExpandedHosts(new Set());
        setExpandedGroups(new Set());
        dispatch({ type: 'FETCH_GRAPH_SUCCESS', graph: res.data });
      } else {
        dispatch({ type: 'FETCH_GRAPH_ERROR', error: res.message });
      }
    };

    if (USE_FIXTURE) {
      loadFixture('live-topology').then(done);
      return;
    }

    const host = hostId.trim() || DEFAULT_HOST_ID;
    if (!host) {
      fetchFirstHostProcessTopology().then(done);
      return;
    }
    fetchHostProcessTopology(host).then(done);
  }, [dispatch, hostId]);

  // Load fixture data on first mount so there is something to render.
  useEffect(() => {
    loadData();
  }, [loadData]);

  const queryHost = useCallback(() => {
    const trimmed = hostId.trim();
    if (!trimmed) return;
    dispatch({ type: 'FETCH_GRAPH_START' });
    fetchHostTopology(trimmed).then((res) => {
      if (res.status === 'ok') {
        setExpandedHosts(new Set());
        setExpandedGroups(new Set());
        dispatch({ type: 'FETCH_GRAPH_SUCCESS', graph: res.data });
      } else {
        dispatch({ type: 'FETCH_GRAPH_ERROR', error: `[${res.code}] ${res.message}` });
      }
    });
  }, [hostId, dispatch]);

  const queryNetwork = useCallback(() => {
    const trimmed = networkId.trim();
    if (!trimmed) return;
    dispatch({ type: 'FETCH_GRAPH_START' });
    fetchNetworkTopology(trimmed).then((res) => {
      if (res.status === 'ok') {
        setExpandedHosts(new Set());
        setExpandedGroups(new Set());
        dispatch({ type: 'FETCH_GRAPH_SUCCESS', graph: res.data });
      } else {
        dispatch({ type: 'FETCH_GRAPH_ERROR', error: `[${res.code}] ${res.message}` });
      }
    });
  }, [networkId, dispatch]);

  const displayGraph = buildDisplayGraph(state.graph, expandedHosts, expandedGroups);
  const selectedNode =
    displayGraph?.nodes.find((n) => n.id === state.selectedNodeId) ?? null;

  const handleSelectNode = useCallback((nodeId: string | null) => {
    if (nodeId && nodeId.startsWith(PROCESS_SUMMARY_PREFIX)) {
      const hostNodeId = nodeId.slice(PROCESS_SUMMARY_PREFIX.length);
      setExpandedHosts((current) => {
        const next = new Set(current);
        if (next.has(hostNodeId)) {
          next.delete(hostNodeId);
        } else {
          next.add(hostNodeId);
        }
        return next;
      });
      dispatch({ type: 'SELECT_NODE', nodeId: null });
      return;
    }
    if (nodeId && nodeId.startsWith(PROCESS_GROUP_PREFIX)) {
      setExpandedGroups((current) => {
        const next = new Set(current);
        if (next.has(nodeId)) {
          next.delete(nodeId);
        } else {
          next.add(nodeId);
        }
        return next;
      });
      dispatch({ type: 'SELECT_NODE', nodeId: null });
      return;
    }
    dispatch({ type: 'SELECT_NODE', nodeId });
  }, [dispatch]);

  return (
    <div className="h-screen flex flex-col bg-gray-50">
      {/* Header */}
      <header className="flex items-center gap-4 px-4 py-2 bg-white border-b border-gray-200 shrink-0">
        <h1 className="text-sm font-semibold text-gray-700 tracking-wide shrink-0">
          dayu-topology
        </h1>

        {/* Host ID query input */}
        <form
          onSubmit={(e) => { e.preventDefault(); queryHost(); }}
          className="flex items-center gap-1"
        >
          <input
            type="text"
            placeholder="Host object ID..."
            value={hostId}
            onChange={(e) => setHostId(e.target.value)}
            className="px-2 py-1 text-xs border border-gray-300 rounded w-56 focus:outline-none focus:ring-2 focus:ring-blue-400"
          />
          <button
            type="submit"
            className="px-3 py-1 text-xs bg-blue-500 text-white rounded hover:bg-blue-600"
          >
            Query
          </button>
        </form>

        {/* Network ID query input */}
        <form
          onSubmit={(e) => { e.preventDefault(); queryNetwork(); }}
          className="flex items-center gap-1"
        >
          <input
            type="text"
            placeholder="Network object ID..."
            value={networkId}
            onChange={(e) => setNetworkId(e.target.value)}
            className="px-2 py-1 text-xs border border-gray-300 rounded w-56 focus:outline-none focus:ring-2 focus:ring-blue-400"
          />
          <button
            type="submit"
            className="px-3 py-1 text-xs bg-green-500 text-white rounded hover:bg-green-600"
          >
            Network
          </button>
        </form>

        <div className="flex-1" />

        <button
          onClick={loadData}
          disabled={state.loading}
          className="px-3 py-1 text-xs bg-gray-100 text-gray-600 rounded hover:bg-gray-200 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          Refresh
        </button>

        {state.graph?.metadata?.queryTime && (
          <span className="text-xs text-gray-400">
            Updated: {state.graph.metadata.queryTime}
          </span>
        )}
      </header>

      {/* Main area */}
      <div className="flex-1 relative overflow-hidden">
        <FilterBar
          searchQuery={state.searchQuery}
          onSearchChange={(q) => dispatch({ type: 'SET_SEARCH', query: q })}
          layerVisibility={state.layerVisibility}
          onToggleLayer={(l) => dispatch({ type: 'TOGGLE_LAYER', layer: l })}
          layout={state.layout}
          onLayoutChange={(l) => dispatch({ type: 'SET_LAYOUT', layout: l })}
        />
        <div className="relative" style={{ height: 'calc(100% - 41px)' }}>
          <TopologyCanvas
            graph={displayGraph}
            selectedNodeId={state.selectedNodeId}
            onSelectNode={handleSelectNode}
            searchQuery={state.searchQuery}
            layout={state.layout}
          />
          <LayerLegend />
          <NodeDetailPanel
            node={selectedNode}
            onClose={() => dispatch({ type: 'SELECT_NODE', nodeId: null })}
          />
        </div>
      </div>

      {/* Loading bar */}
      <div
        className={`h-0.5 bg-blue-500 transition-all duration-300 ease-out ${state.loading ? 'opacity-100' : 'opacity-0'}`}
        style={{ width: state.loading ? '100%' : '0%' }}
      />

      {/* Error banner */}
      {state.error && (
        <div className="absolute bottom-4 right-4 bg-red-50 border border-red-200 rounded-lg px-4 py-2 text-xs text-red-700 z-20 max-w-md shadow">
          <span className="font-medium">Error</span>
          <span className="ml-1">{state.error}</span>
          <button
            onClick={() => dispatch({ type: 'FETCH_GRAPH_ERROR', error: '' })}
            className="ml-2 text-red-400 hover:text-red-600 font-bold"
          >
            &times;
          </button>
        </div>
      )}
    </div>
  );
}

function buildDisplayGraph(
  source: HostProcessTopologyGraph | null,
  expandedHosts: Set<string>,
  expandedGroups: Set<string>,
): HostProcessTopologyGraph | null {
  if (!source) return null;

  const nodesById = new Map(source.nodes.map((node) => [node.id, node]));
  const processSummaryEdgesByHost = new Map<string, HostProcessTopologyEdge[]>();
  const processGroupEdgesBySummary = new Map<string, HostProcessTopologyEdge[]>();
  const processEdgesByGroup = new Map<string, HostProcessTopologyEdge[]>();

  for (const edge of source.edges) {
    if (edge.edgeKind !== 'host_process_assoc') continue;
    if (edge.target.startsWith(PROCESS_SUMMARY_PREFIX)) {
      const edges = processSummaryEdgesByHost.get(edge.source) ?? [];
      edges.push(edge);
      processSummaryEdgesByHost.set(edge.source, edges);
      continue;
    }
    if (edge.source.startsWith(PROCESS_SUMMARY_PREFIX) && edge.target.startsWith(PROCESS_GROUP_PREFIX)) {
      const edges = processGroupEdgesBySummary.get(edge.source) ?? [];
      edges.push(edge);
      processGroupEdgesBySummary.set(edge.source, edges);
      continue;
    }
    if (edge.source.startsWith(PROCESS_GROUP_PREFIX)) {
      const edges = processEdgesByGroup.get(edge.source) ?? [];
      edges.push(edge);
      processEdgesByGroup.set(edge.source, edges);
    }
  }

  const displayNodes: HostProcessTopologyNode[] = [];
  const displayEdges: HostProcessTopologyEdge[] = [];
  const includedNodeIds = new Set<string>();

  for (const node of source.nodes) {
    if (node.objectKind !== 'HostInventory' && node.objectKind !== 'NetworkSegment' && node.objectKind !== 'Subject') {
      continue;
    }
    displayNodes.push(node);
    includedNodeIds.add(node.id);
  }

  for (const edge of source.edges) {
    if (edge.edgeKind === 'host_process_assoc') continue;
    if (includedNodeIds.has(edge.source) && includedNodeIds.has(edge.target)) {
      displayEdges.push(edge);
    }
  }

  for (const [hostNodeId, summaryEdges] of processSummaryEdgesByHost.entries()) {
    for (const summaryEdge of summaryEdges) {
      const summaryNode = nodesById.get(summaryEdge.target);
      if (!summaryNode) continue;

      displayNodes.push(summaryNode);
      displayEdges.push(summaryEdge);

      const isExpanded = expandedHosts.has(hostNodeId);
      if (!isExpanded) continue;

      const groupEdges = processGroupEdgesBySummary.get(summaryNode.id) ?? [];
      for (const groupEdge of groupEdges) {
        const groupNode = nodesById.get(groupEdge.target);
        if (!groupNode) continue;

        displayNodes.push(groupNode);
        displayEdges.push(groupEdge);

        const isGroupExpanded = expandedGroups.has(groupNode.id);
        if (!isGroupExpanded) continue;

        const processEdges = processEdgesByGroup.get(groupNode.id) ?? [];
        for (const processEdge of processEdges.slice(0, PROCESS_EXPAND_LIMIT)) {
          const processNode = nodesById.get(processEdge.target);
          if (!processNode) continue;
          displayNodes.push(processNode);
          displayEdges.push(processEdge);
        }
      }
    }
  }

  return {
    nodes: displayNodes,
    edges: displayEdges,
    metadata: source.metadata,
  };
}
