// ============================================================
// dayu-topology visualization — V1 domain types
// Aligned with topology-visualization-frontend-architecture.md
// V1 scope: host + network + responsibility only
// ============================================================

// ---- Layer classification (V1: only 2 layers) ----
export type LayerKind = 'resource' | 'governance';

// ---- Node kind (V1: only 3 object types) ----
export type NodeKind =
  | 'HostInventory'
  | 'NetworkSegment'
  | 'Subject';

// ---- Edge kind (V1: only 2 relation types) ----
export type EdgeKind =
  | 'host_network_assoc'
  | 'responsibility_assignment';

// ---- Layer → NodeKind mapping ----
export const LAYER_NODE_KINDS: Record<LayerKind, readonly NodeKind[]> = {
  resource:   ['HostInventory', 'NetworkSegment'],
  governance: ['Subject'],
};

// ---- Core DTOs ----
export type TopologyNode = {
  id: string;                     // visual element ID (not necessarily the backend UUID)
  objectKind: NodeKind;           // backend object type
  objectId: string;               // backend object UUID
  layer: LayerKind;
  label: string;                  // display name
  properties: Record<string, unknown>;
};

export type TopologyEdge = {
  id: string;
  edgeKind: EdgeKind;
  source: string;                 // node id
  target: string;                 // node id
  label?: string;
  properties?: Record<string, unknown>;
};

export type TopologyGraph = {
  nodes: TopologyNode[];
  edges: TopologyEdge[];
  metadata?: {
    queryTime: string;
    tenantId?: string;
    focusObjectKind?: NodeKind;
    focusObjectId?: string;
    truncated?: boolean;
  };
};

// ---- API response wrapper ----
export type ApiResponse<T> =
  | { status: 'ok'; data: T }
  | { status: 'error'; code: string; message: string };

// ---- App state (single graph view, no multi-view routing in V1) ----
export type AppState = {
  graph: TopologyGraph | null;
  loading: boolean;
  error: string | null;

  selectedNodeId: string | null;
  layerVisibility: Record<LayerKind, boolean>;
  searchQuery: string;
  layout: LayoutName;
};

export type AppAction =
  | { type: 'FETCH_GRAPH_START' }
  | { type: 'FETCH_GRAPH_SUCCESS'; graph: TopologyGraph }
  | { type: 'FETCH_GRAPH_ERROR'; error: string }
  | { type: 'SELECT_NODE'; nodeId: string | null }
  | { type: 'TOGGLE_LAYER'; layer: LayerKind }
  | { type: 'SET_SEARCH'; query: string }
  | { type: 'SET_LAYOUT'; layout: LayoutName };

export type LayoutName = 'dagre' | 'cose-bilkent';

// ---- Cytoscape element data extensions ----
export type CyNodeData = {
  id: string;
  objectKind: NodeKind;
  layer: LayerKind;
  label: string;
  properties: Record<string, unknown>;
};

export type CyEdgeData = {
  id: string;
  source: string;
  target: string;
  edgeKind: EdgeKind;
  label: string;
  properties?: Record<string, unknown>;
};
