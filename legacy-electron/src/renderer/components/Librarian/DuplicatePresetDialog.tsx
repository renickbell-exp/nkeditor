/**
 * Duplicate Preset Dialog Component
 *
 * Modal dialog for duplicating a preset with a new name.
 * Generates a default name based on the original preset name.
 */

import React, { useState, useCallback, useEffect } from 'react';
import { usePresetStore } from '../../stores/preset';
import { generateDuplicateName } from '@shared/schemas';
import type { PresetMetadata } from '@shared/ipc-protocol';

interface DuplicatePresetDialogProps {
  isOpen: boolean;
  onClose: () => void;
  preset: PresetMetadata | null;
}

export function DuplicatePresetDialog({ isOpen, onClose, preset }: DuplicatePresetDialogProps): React.ReactElement | null {
  const [name, setName] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const presets = usePresetStore((state) => state.presets);
  const duplicatePreset = usePresetStore((state) => state.duplicatePreset);
  const loadPresets = usePresetStore((state) => state.loadPresets);

  // Generate default name when preset changes or dialog opens
  useEffect(() => {
    if (preset && isOpen) {
      const existingNames = presets.map(p => p.name);
      const defaultName = generateDuplicateName(preset.name, existingNames);
      setName(defaultName);
      setError(null);
    }
  }, [preset, isOpen, presets]);

  // Handle Escape key to close dialog
  useEffect(() => {
    if (!isOpen) return;

    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !isSubmitting) {
        onClose();
      }
    };

    document.addEventListener('keydown', handleEscape);
    return () => document.removeEventListener('keydown', handleEscape);
  }, [isOpen, isSubmitting, onClose]);

  const handleSubmit = useCallback(async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (!preset) {
      setError('No preset selected');
      return;
    }

    const trimmedName = name.trim();
    if (!trimmedName) {
      setError('Name is required');
      return;
    }

    // Check if name already exists
    const existingNames = presets.map(p => p.name.toLowerCase());
    if (existingNames.includes(trimmedName.toLowerCase())) {
      setError('A preset with this name already exists');
      return;
    }

    setIsSubmitting(true);

    try {
      const newId = await duplicatePreset(preset.id, trimmedName);
      if (newId) {
        // Reload presets to show the new duplicate
        await loadPresets();
        onClose();
      } else {
        setError('Failed to duplicate preset');
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(message);
    } finally {
      setIsSubmitting(false);
    }
  }, [preset, name, presets, duplicatePreset, loadPresets, onClose]);

  const handleClose = useCallback(() => {
    if (!isSubmitting) {
      setError(null);
      onClose();
    }
  }, [isSubmitting, onClose]);

  // Do not render if dialog is closed or no preset
  if (!isOpen || !preset) {
    return null;
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/60"
        onClick={handleClose}
      />

      {/* Dialog */}
      <div className="relative bg-nk-dark border border-nk-border rounded-lg shadow-xl w-full max-w-md mx-4">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-nk-border">
          <h2 className="text-lg font-semibold text-gray-200">Duplicate Preset</h2>
          <button
            className="text-gray-500 hover:text-gray-300 text-xl leading-none"
            onClick={handleClose}
            disabled={isSubmitting}
            aria-label="Close dialog"
          >
            x
          </button>
        </div>

        {/* Content */}
        <form onSubmit={handleSubmit} className="p-4 space-y-4">
          {/* Error message */}
          {error && (
            <div className="text-red-400 text-sm bg-red-900/20 border border-red-800 rounded px-3 py-2">
              {error}
            </div>
          )}

          {/* Original preset info */}
          <div className="text-sm text-gray-500">
            Duplicating: <span className="text-gray-300">{preset.name}</span>
          </div>

          {/* Name input */}
          <div>
            <label htmlFor="duplicate-name" className="block text-sm font-medium text-gray-300 mb-1">
              New preset name
            </label>
            <input
              id="duplicate-name"
              type="text"
              className="input w-full"
              value={name}
              onChange={(e) => setName(e.target.value)}
              disabled={isSubmitting}
              autoFocus
              maxLength={100}
            />
          </div>

          {/* Actions */}
          <div className="flex justify-end gap-2 pt-2">
            <button
              type="button"
              className="btn btn-ghost"
              onClick={handleClose}
              disabled={isSubmitting}
            >
              Cancel
            </button>
            <button
              type="submit"
              className="btn btn-primary"
              disabled={isSubmitting || !name.trim()}
            >
              {isSubmitting ? 'Duplicating...' : 'Duplicate'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
