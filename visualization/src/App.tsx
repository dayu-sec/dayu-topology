import { useEffect, useState, useCallback } from 'react';
import { useAppState } from './hooks/useAppState';
import TopologyCanvas from './components/TopologyCanvas';
import FilterBar from './components/FilterBar';
import LayerLegend from './components/LayerLegend';
import NodeDetailPanel from './components/NodeDetailPanel';
import { fetchHostTopology, fetchNetworkTopology, loadFixture } from './api/client';

export default function App() {
  const [state, dispatch] = useAppState();
  const [hostId, setHostId] = useState('');
  const [networkId, setNetworkId] = useState('');

  const loadData = useCallback(() => {
    dispatch({ type: 'FETCH_GRAPH_START' });
    loadFixture('host-topology').then((res) => {
      if (res.status === 'ok') {
        dispatch({ type: 'FETCH_GRAPH_SUCCESS', graph: res.data });
      } else {
        dispatch({ type: 'FETCH_GRAPH_ERROR', error: res.message });
      }
    });
  }, []);

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
        dispatch({ type: 'FETCH_GRAPH_SUCCESS', graph: res.data });
      } else {
        dispatch({ type: 'FETCH_GRAPH_ERROR', error: `[${res.code}] ${res.message}` });
      }
    });
  }, [networkId, dispatch]);

  const selectedNode =
    state.graph?.nodes.find((n) => n.id === state.selectedNodeId) ?? null;

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
            graph={state.graph}
            selectedNodeId={state.selectedNodeId}
            onSelectNode={(id) => dispatch({ type: 'SELECT_NODE', nodeId: id })}
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
