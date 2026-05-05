import type { TopologyNode } from '../types/domain';
import { getLayerStyle } from '../utils/colors';

type Props = {
  node: TopologyNode | null;
  onClose: () => void;
};

export default function NodeDetailPanel({ node, onClose }: Props) {
  if (!node) return null;

  const style = getLayerStyle(node.objectKind);

  return (
    <div className="absolute right-0 top-0 h-full w-72 bg-white border-l border-gray-200 shadow-lg overflow-y-auto z-10">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-gray-100">
        <div className="flex items-center gap-2">
          <span
            className="inline-block w-3 h-3 rounded-sm"
            style={{ backgroundColor: style.bg, border: `1.5px solid ${style.border}` }}
          />
          <span className="text-sm font-medium text-gray-800">{node.objectKind}</span>
        </div>
        <button
          onClick={onClose}
          className="text-gray-400 hover:text-gray-600 text-lg leading-none"
        >
          &times;
        </button>
      </div>

      {/* Body */}
      <div className="p-4 space-y-3">
        <div>
          <span className="text-xs text-gray-400 block">Label</span>
          <span className="text-sm text-gray-800 font-medium">{node.label}</span>
        </div>
        <div>
          <span className="text-xs text-gray-400 block">Graph ID</span>
          <span className="text-sm text-gray-600 font-mono text-xs break-all">{node.id}</span>
        </div>
        <div>
          <span className="text-xs text-gray-400 block">Object ID</span>
          <span className="text-sm text-gray-600 font-mono text-xs break-all">{node.objectId}</span>
        </div>
        <div>
          <span className="text-xs text-gray-400 block">Layer</span>
          <span className="text-sm text-gray-600">{node.layer}</span>
        </div>

        {Object.keys(node.properties).length > 0 && (
          <div className="pt-2 border-t border-gray-100">
            <span className="text-xs text-gray-400 block mb-1">Properties</span>
            {Object.entries(node.properties).map(([key, value]) => (
              <div key={key} className="flex justify-between py-0.5">
                <span className="text-xs text-gray-500">{key}</span>
                <span className="text-xs text-gray-700 font-mono">
                  {String(value)}
                </span>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
