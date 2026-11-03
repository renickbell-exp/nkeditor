/**
 * Quick Access Component
 *
 * 5-slot preset quick access bar for fast preset switching.
 */

import React from 'react';
import { usePresetStore } from '../../stores/preset';
import { useSettingsStore } from '../../stores/settings';

export function QuickAccess(): React.ReactElement {
  const quickAccessSlots = useSettingsStore((state) => state.config?.quickAccessSlots ?? []);
  const setQuickAccessSlot = useSettingsStore((state) => state.setQuickAccessSlot);
  const presets = usePresetStore((state) => state.presets);
  const selectedId = usePresetStore((state) => state.selectedId);
  const loadPreset = usePresetStore((state) => state.loadPreset);

  // Get preset name by ID
  const getPresetName = (id: string | null): string | null => {
    if (!id) return null;
    const preset = presets.find((p) => p.id === id);
    return preset?.name ?? null;
  };

  // Handle slot click
  const handleSlotClick = (index: number) => {
    const slotId = quickAccessSlots[index];
    if (slotId) {
      loadPreset(slotId);
    }
  };

  // Handle slot assignment (right-click or drag)
  const handleSlotAssign = (index: number) => {
    if (selectedId) {
      setQuickAccessSlot(index, selectedId);
    }
  };

  // Handle slot clear
  const handleSlotClear = (index: number, e: React.MouseEvent) => {
    e.stopPropagation();
    setQuickAccessSlot(index, null);
  };

  return (
    <div className="mb-4">
      <div className="flex items-center justify-between mb-2">
        <h3 className="text-sm font-medium text-gray-400">Quick Access</h3>
        <span className="text-xs text-gray-600">
          Right-click to assign selected preset
        </span>
      </div>
      <div className="flex gap-2">
        {[0, 1, 2, 3, 4].map((index) => {
          const slotId = quickAccessSlots[index] ?? null;
          const presetName = getPresetName(slotId);

          return (
            <div
              key={index}
              className={`
                flex-1 h-12 rounded border-2 border-dashed flex items-center justify-center
                cursor-pointer transition-colors text-sm
                ${slotId
                  ? 'border-nk-border bg-nk-dark hover:bg-nk-light'
                  : 'border-nk-border/50 hover:border-nk-accent'
                }
              `}
              onClick={() => handleSlotClick(index)}
              onContextMenu={(e) => {
                e.preventDefault();
                handleSlotAssign(index);
              }}
              title={slotId ? `Load: ${presetName}` : 'Right-click to assign'}
            >
              {presetName ? (
                <div className="flex items-center gap-2 px-2">
                  <span className="text-nk-accent font-bold">{index + 1}</span>
                  <span className="truncate text-gray-300">{presetName}</span>
                  <button
                    className="text-gray-600 hover:text-gray-300"
                    onClick={(e) => handleSlotClear(index, e)}
                    title="Clear slot"
                  >
                    x
                  </button>
                </div>
              ) : (
                <span className="text-gray-600">{index + 1}</span>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
