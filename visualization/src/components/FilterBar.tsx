import type { LayerKind, LayoutName } from '../types/domain';
import { LAYER_COLORS } from '../utils/colors';

type Props = {
  searchQuery: string;
  onSearchChange: (query: string) => void;
  layerVisibility: Record<LayerKind, boolean>;
  onToggleLayer: (layer: LayerKind) => void;
  layout: LayoutName;
  onLayoutChange: (layout: LayoutName) => void;
};

const ALL_LAYERS: LayerKind[] = ['resource', 'governance'];

const LAYOUT_OPTIONS: { value: LayoutName; label: string }[] = [
  { value: 'concentric', label: '中心放射 (concentric)' },
  { value: 'dagre', label: '层次 (dagre)' },
  { value: 'cose-bilkent', label: '力导向 (cose)' },
];

export default function FilterBar({ searchQuery, onSearchChange, layerVisibility, onToggleLayer, layout, onLayoutChange }: Props) {
  return (
    <div className="flex items-center gap-3 px-4 py-2 bg-white border-b border-gray-200">
      <input
        type="text"
        placeholder="Search nodes..."
        value={searchQuery}
        onChange={(e) => onSearchChange(e.target.value)}
        className="px-3 py-1.5 text-sm border border-gray-300 rounded-md w-48 focus:outline-none focus:ring-2 focus:ring-blue-400"
      />

      <div className="flex items-center gap-2 ml-2">
        {ALL_LAYERS.map((layer) => (
          <label
            key={layer}
            className="flex items-center gap-1 text-xs cursor-pointer select-none"
          >
            <input
              type="checkbox"
              checked={layerVisibility[layer]}
              onChange={() => onToggleLayer(layer)}
              style={{ accentColor: LAYER_COLORS[layer].border }}
            />
            <span style={{ color: LAYER_COLORS[layer].border }}>
              {LAYER_COLORS[layer].label}
            </span>
          </label>
        ))}
      </div>

      <select
        value={layout}
        onChange={(e) => onLayoutChange(e.target.value as LayoutName)}
        className="ml-auto text-xs border border-gray-300 rounded px-2 py-1 focus:outline-none focus:ring-2 focus:ring-blue-400"
      >
        {LAYOUT_OPTIONS.map((opt) => (
          <option key={opt.value} value={opt.value}>{opt.label}</option>
        ))}
      </select>
    </div>
  );
}
