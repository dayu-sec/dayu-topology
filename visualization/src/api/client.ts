import type { ApiResponse, HostProcessTopologyGraph } from '../types/domain';

const BASE = '/api';

async function get<T>(path: string, params?: Record<string, string>): Promise<ApiResponse<T>> {
  try {
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
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { status: 'error', code: 'fetch_failed', message };
  }
}

// V1 required
export async function fetchHostTopology(hostId: string): Promise<ApiResponse<HostProcessTopologyGraph>> {
  return get<HostProcessTopologyGraph>(`/topology/host/${hostId}`);
}

// V1 optional enhancement
export async function fetchNetworkTopology(networkId: string): Promise<ApiResponse<HostProcessTopologyGraph>> {
  return get<HostProcessTopologyGraph>(`/topology/network/${networkId}`);
}

// Dev-only mock (against Rust mock endpoint)
export async function fetchMockHostTopology(): Promise<ApiResponse<HostProcessTopologyGraph>> {
  return get<HostProcessTopologyGraph>('/__mock/topology/host');
}

// Static fixture loader (zero backend dependency)
export async function loadFixture(name: string): Promise<ApiResponse<HostProcessTopologyGraph>> {
  try {
    const res = await fetch(`/fixtures/${name}.json`);
    if (!res.ok) {
      return { status: 'error', code: String(res.status), message: res.statusText };
    }
    return res.json() as Promise<ApiResponse<HostProcessTopologyGraph>>;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { status: 'error', code: 'fetch_failed', message };
  }
}

export async function fetchHostProcessTopology(hostId: string): Promise<ApiResponse<HostProcessTopologyGraph>> {
  return get<HostProcessTopologyGraph>(`/topology/host/${hostId}/processes`);
}

export async function fetchFirstHostProcessTopology(): Promise<ApiResponse<HostProcessTopologyGraph>> {
  return get<HostProcessTopologyGraph>('/topology/host/first/processes');
}
