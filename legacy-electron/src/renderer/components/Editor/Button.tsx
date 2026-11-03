/**
 * Button Component
 *
 * Visual representation of a button control with LED indicator.
 * Supports both click-to-toggle and right-click/Ctrl+click to edit mapping.
 *
 * - Left-click: Toggles the button state
 * - Right-click or Ctrl+click: Opens the mapping editor popover
 */

import React, { useState, useCallback } from 'react';
import { ControlPopover } from './ControlPopover';

type ButtonType = 'solo' | 'mute' | 'rec' | 'transport';

interface ButtonProps {
  type: ButtonType;
  active: boolean;
  label?: string;
  controlId: string;
  onClick?: () => void;
  compact?: boolean;
}

export function Button({ type, active, label, controlId, onClick, compact = false }: ButtonProps): React.ReactElement {
  const [popoverOpen, setPopoverOpen] = useState(false);
  const [popoverPosition, setPopoverPosition] = useState({ x: 0, y: 0 });

  const openPopover = useCallback((x: number, y: number) => {
    setPopoverPosition({ x, y });
    setPopoverOpen(true);
  }, []);

  // Get LED color class based on type
  const getLedClass = () => {
    if (!active) return 'led-off';
    switch (type) {
      case 'solo':
        return 'led-solo';
      case 'mute':
        return 'led-mute';
      case 'rec':
        return 'led-rec';
      case 'transport':
        return 'led-solo'; // Use yellow for transport
      default:
        return 'led-off';
    }
  };

  // Get button background based on type
  const getButtonClass = () => {
    const base = 'relative flex items-center justify-center rounded transition-all';
    const size = compact ? 'w-8 h-8' : 'w-10 h-8';
    const bg = active
      ? 'bg-gray-600 shadow-inner'
      : 'bg-gray-700 hover:bg-gray-600 shadow';

    return `${base} ${size} ${bg}`;
  };

  // Get label color based on type
  const getLabelClass = () => {
    switch (type) {
      case 'solo':
        return 'text-yellow-500';
      case 'mute':
        return 'text-green-500';
      case 'rec':
        return 'text-red-500';
      default:
        return 'text-gray-400';
    }
  };

  const handleClick = (e: React.MouseEvent) => {
    // Ctrl+click or meta+click opens the popover instead of toggling
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      openPopover(e.clientX, e.clientY);
      return;
    }

    // Regular click toggles the button
    if (onClick) {
      onClick();
    }
  };

  // Right-click opens the popover
  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    openPopover(e.clientX, e.clientY);
  };

  return (
    <>
      <div
        className="flex flex-col items-center gap-1 cursor-pointer"
        title={`${controlId}: ${active ? 'ON' : 'OFF'} (right-click to edit mapping)`}
        onClick={handleClick}
        onContextMenu={handleContextMenu}
      >
        {/* LED indicator */}
        <div className={`led ${getLedClass()}`} />

        {/* Button */}
        <button
          className={getButtonClass()}
          type="button"
        >
          <span className={`text-xs font-medium ${getLabelClass()}`}>
            {label || type.charAt(0).toUpperCase()}
          </span>
        </button>
      </div>

      <ControlPopover
        isOpen={popoverOpen}
        onClose={() => setPopoverOpen(false)}
        controlId={controlId}
        controlKind="button"
        anchorPosition={popoverPosition}
      />
    </>
  );
}
