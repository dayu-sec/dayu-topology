import type { ApiResponse, TopologyGraph } from '../types/domain';

const BASE = '/api';

async function get<T>(path: string, params?: Record<string, string>): Promise<ApiResponse<T>> {
  const url = new URL(`${BASE}${path}`, window.location.origin);
  if (params) {
    for (const [k, v] of Object.entries(params)) {
      url.searchParams.set(k, v);
    }
  }
  const res = await fetch(url.toString());
  if (!res.ok) {
    return { status: 'error', code: String(res.status), message: res.statusText };
  }
  return res.json() as Promise<ApiResponse<T>>;
}

// V1 required
export async function fetchHostTopology(hostId: string): Promise<ApiResponse<TopologyGraph>> {
  return get<TopologyGraph>(`/topology/host/${hostId}`);
}

// V1 optional enhancement
export async function fetchNetworkTopology(networkId: string): Promise<ApiResponse<TopologyGraph>> {
  return get<TopologyGraph>(`/topology/network/${networkId}`);
}

// Dev-only mock (against Rust mock endpoint)
export async function fetchMockHostTopology(): Promise<ApiResponse<TopologyGraph>> {
  return get<TopologyGraph>('/__mock/topology/host');
}

// Static fixture loader (zero backend dependency)
export async function loadFixture(name: string): Promise<ApiResponse<TopologyGraph>> {
  const res = await fetch(`/fixtures/${name}.json`);
  if (!res.ok) {
    return { status: 'error', code: String(res.status), message: res.statusText };
  }
  return res.json() as Promise<ApiResponse<TopologyGraph>>;
}
