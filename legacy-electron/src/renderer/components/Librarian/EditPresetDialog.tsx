/**
 * Edit Preset Dialog Component
 *
 * Modal dialog for editing preset metadata: name, description, tags, and group.
 * Does not modify the mapping configuration - only metadata fields.
 */

import React, { useState, useCallback, useEffect } from 'react';
import { usePresetStore } from '../../stores/preset';
import type { PresetMetadata } from '@shared/ipc-protocol';

interface EditPresetDialogProps {
  isOpen: boolean;
  onClose: () => void;
  /** The preset metadata to edit. Dialog will not render if null. */
  preset: PresetMetadata | null;
}

export function EditPresetDialog({ isOpen, onClose, preset }: EditPresetDialogProps): React.ReactElement | null {
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [group, setGroup] = useState('');
  const [tagInput, setTagInput] = useState('');
  const [tags, setTags] = useState<string[]>([]);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const updatePresetMetadata = usePresetStore((state) => state.updatePresetMetadata);

  // Initialize form when preset changes or dialog opens
  useEffect(() => {
    if (preset && isOpen) {
      setName(preset.name);
      setDescription(preset.description ?? '');
      setGroup(preset.group ?? '');
      setTags([...preset.tags]);
      setTagInput('');
      setError(null);
    }
  }, [preset, isOpen]);

  const handleAddTag = useCallback(() => {
    const trimmedTag = tagInput.trim().toLowerCase();
    if (trimmedTag && !tags.includes(trimmedTag) && tags.length < 10) {
      setTags([...tags, trimmedTag]);
      setTagInput('');
    }
  }, [tagInput, tags]);

  const handleRemoveTag = useCallback((tagToRemove: string) => {
    setTags(tags.filter((tag) => tag !== tagToRemove));
  }, [tags]);

  const handleTagInputKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      handleAddTag();
    }
  }, [handleAddTag]);

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

    setIsSubmitting(true);

    try {
      const success = await updatePresetMetadata(preset.id, {
        name: trimmedName,
        description: description.trim() || undefined,
        group: group.trim() || undefined,
        tags,
      });

      if (success) {
        onClose();
      } else {
        setError('Failed to update preset');
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(message);
    } finally {
      setIsSubmitting(false);
    }
  }, [preset, name, description, group, tags, updatePresetMetadata, onClose]);

  const handleClose = useCallback(() => {
    if (!isSubmitting) {
      setError(null);
      onClose();
    }
  }, [isSubmitting, onClose]);

  // Do not render if dialog is closed or no preset provided
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
          <h2 className="text-lg font-semibold text-gray-200">Edit Preset</h2>
          <button
            className="text-gray-500 hover:text-gray-300 text-xl leading-none"
            onClick={handleClose}
            disabled={isSubmitting}
            aria-label="Close dialog"
          >
            x
          </button>
        </div>

        {/* Form */}
        <form onSubmit={handleSubmit} className="p-4 space-y-4">
          {/* Error message */}
          {error && (
            <div className="text-red-400 text-sm bg-red-900/20 border border-red-800 rounded px-3 py-2">
              {error}
            </div>
          )}

          {/* Name field */}
          <div>
            <label htmlFor="edit-preset-name" className="block text-sm font-medium text-gray-400 mb-1">
              Name *
            </label>
            <input
              id="edit-preset-name"
              type="text"
              className="input w-full"
              placeholder="My Preset"
              value={name}
              onChange={(e) => setName(e.target.value)}
              disabled={isSubmitting}
              autoFocus
              maxLength={100}
            />
          </div>

          {/* Description field */}
          <div>
            <label htmlFor="edit-preset-description" className="block text-sm font-medium text-gray-400 mb-1">
              Description
            </label>
            <textarea
              id="edit-preset-description"
              className="input w-full resize-none"
              placeholder="Optional description for this preset..."
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              disabled={isSubmitting}
              rows={3}
              maxLength={500}
            />
          </div>

          {/* Group field */}
          <div>
            <label htmlFor="edit-preset-group" className="block text-sm font-medium text-gray-400 mb-1">
              Group / Folder
            </label>
            <input
              id="edit-preset-group"
              type="text"
              className="input w-full"
              placeholder="e.g., Factory, User, Live Sets..."
              value={group}
              onChange={(e) => setGroup(e.target.value)}
              disabled={isSubmitting}
              maxLength={50}
            />
            <p className="text-xs text-gray-600 mt-1">
              Optional folder for organizing presets
            </p>
          </div>

          {/* Tags field */}
          <div>
            <label htmlFor="edit-preset-tags" className="block text-sm font-medium text-gray-400 mb-1">
              Tags
            </label>
            <div className="flex gap-2">
              <input
                id="edit-preset-tags"
                type="text"
                className="input flex-1"
                placeholder="Add a tag..."
                value={tagInput}
                onChange={(e) => setTagInput(e.target.value)}
                onKeyDown={handleTagInputKeyDown}
                disabled={isSubmitting || tags.length >= 10}
                maxLength={30}
              />
              <button
                type="button"
                className="btn btn-ghost"
                onClick={handleAddTag}
                disabled={isSubmitting || !tagInput.trim() || tags.length >= 10}
              >
                Add
              </button>
            </div>
            {tags.length > 0 && (
              <div className="flex flex-wrap gap-1 mt-2">
                {tags.map((tag) => (
                  <span
                    key={tag}
                    className="inline-flex items-center gap-1 px-2 py-0.5 bg-nk-border rounded text-sm text-gray-300"
                  >
                    {tag}
                    <button
                      type="button"
                      className="text-gray-500 hover:text-gray-300 leading-none"
                      onClick={() => handleRemoveTag(tag)}
                      disabled={isSubmitting}
                      aria-label={`Remove tag ${tag}`}
                    >
                      x
                    </button>
                  </span>
                ))}
              </div>
            )}
            <p className="text-xs text-gray-600 mt-1">
              {tags.length}/10 tags - Press Enter or click Add
            </p>
          </div>

          {/* Metadata info (read-only) */}
          <div className="text-xs text-gray-600 border-t border-nk-border pt-3 space-y-1">
            <p>Created: {new Date(preset.createdAt).toLocaleString()}</p>
            <p>Modified: {new Date(preset.modifiedAt).toLocaleString()}</p>
            <p className="text-gray-700 truncate">ID: {preset.id}</p>
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
              {isSubmitting ? 'Saving...' : 'Save Changes'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
