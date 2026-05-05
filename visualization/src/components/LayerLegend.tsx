import { LAYER_COLORS } from '../utils/colors';
import type { LayerKind } from '../types/domain';

const ALL_LAYERS: LayerKind[] = ['resource', 'governance'];

export default function LayerLegend() {
  return (
    <div className="absolute bottom-4 left-4 bg-white/90 backdrop-blur rounded-lg shadow border border-gray-200 px-3 py-2 text-xs">
      <div className="font-medium text-gray-500 mb-1">Layers</div>
      {ALL_LAYERS.map((layer) => {
        const c = LAYER_COLORS[layer];
        return (
          <div key={layer} className="flex items-center gap-2 py-0.5">
            <span
              className="inline-block w-3 h-3 rounded-sm"
              style={{ backgroundColor: c.bg, border: `1.5px solid ${c.border}` }}
            />
            <span className="text-gray-600">{c.label}</span>
          </div>
        );
      })}
    </div>
  );
}
