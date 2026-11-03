/**
 * Knob Component
 *
 * Visual representation of a rotary knob control.
 * Supports both drag-to-change-value and click-to-edit-mapping interactions.
 *
 * - Drag up/down: Changes the control value
 * - Right-click or Ctrl+click: Opens the mapping editor popover
 */

import React, { useState, useRef, useCallback } from 'react';
import { ControlPopover } from './ControlPopover';

interface KnobProps {
  value: number;
  label?: string;
  controlId: string;
  onValueChange?: (value: number) => void;
  /** Whether this control is selected for batch operations */
  selected?: boolean;
  /** Callback when selection is toggled */
  onToggleSelection?: () => void;
}

/** Minimum drag distance (in pixels) to distinguish drag from click */
const DRAG_THRESHOLD = 3;

export function Knob({ value, label, controlId, onValueChange, selected = false, onToggleSelection }: KnobProps): React.ReactElement {
  const [isDragging, setIsDragging] = useState(false);
  const [popoverOpen, setPopoverOpen] = useState(false);
  const [popoverPosition, setPopoverPosition] = useState({ x: 0, y: 0 });
  const containerRef = useRef<HTMLDivElement>(null);

  const handleSelectionClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    onToggleSelection?.();
  };

  // Calculate rotation angle (0-127 maps to -135 to 135 degrees)
  const rotation = (value / 127) * 270 - 135;

  const openPopover = useCallback((x: number, y: number) => {
    setPopoverPosition({ x, y });
    setPopoverOpen(true);
  }, []);

  const handleMouseDown = (e: React.MouseEvent) => {
    // Right-click or Ctrl+click opens the popover
    if (e.button === 2 || e.ctrlKey || e.metaKey) {
      e.preventDefault();
      openPopover(e.clientX, e.clientY);
      return;
    }

    // Left-click initiates drag
    if (!onValueChange) return;

    const startY = e.clientY;
    const startX = e.clientX;
    const startValue = value;
    let hasDragged = false;

    const handleMouseMove = (moveEvent: MouseEvent) => {
      const deltaY = startY - moveEvent.clientY;
      const deltaX = moveEvent.clientX - startX;

      // Check if we've moved enough to count as a drag
      if (!hasDragged && (Math.abs(deltaY) > DRAG_THRESHOLD || Math.abs(deltaX) > DRAG_THRESHOLD)) {
        hasDragged = true;
        setIsDragging(true);
      }

      if (hasDragged) {
        const newValue = Math.max(0, Math.min(127, startValue + deltaY));
        onValueChange(newValue);
      }
    };

    const handleMouseUp = (upEvent: MouseEvent) => {
      setIsDragging(false);
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);

      // If we didn't drag, treat as a click to open popover
      if (!hasDragged) {
        openPopover(upEvent.clientX, upEvent.clientY);
      }
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
  };

  // Prevent context menu on right-click since we use it for popover
  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    openPopover(e.clientX, e.clientY);
  };

  return (
    <>
      <div
        ref={containerRef}
        className={`flex flex-col items-center gap-1 cursor-pointer relative ${selected ? 'ring-2 ring-nk-accent rounded-lg p-1' : ''}`}
        onMouseDown={handleMouseDown}
        onContextMenu={handleContextMenu}
        title={`${controlId}: ${value} (click to edit mapping)`}
      >
        {/* Selection checkbox */}
        {onToggleSelection && (
          <div
            className="absolute -top-1 -right-1 z-10"
            onClick={handleSelectionClick}
            onMouseDown={(e) => e.stopPropagation()}
          >
            <input
              type="checkbox"
              checked={selected}
              onChange={() => {}}
              className="w-3 h-3 rounded border-gray-500 text-blue-500 focus:ring-blue-500 focus:ring-offset-0 cursor-pointer"
              title={selected ? 'Click to deselect' : 'Click to select for randomization'}
            />
          </div>
        )}
        <span className="text-xs text-gray-500 truncate max-w-[60px]" title={label || controlId}>
          {label || controlId}
        </span>
        <div
          className={`knob ${isDragging ? 'ring-2 ring-nk-accent' : ''}`}
          style={{ transform: `rotate(${rotation}deg)` }}
        >
          <div className="knob-indicator" />
        </div>
        <span className="text-xs text-gray-600">{value}</span>
      </div>

      <ControlPopover
        isOpen={popoverOpen}
        onClose={() => setPopoverOpen(false)}
        controlId={controlId}
        controlKind="knob"
        anchorPosition={popoverPosition}
      />
    </>
  );
}
