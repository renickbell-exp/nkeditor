/**
 * New Preset Dialog Component
 *
 * Modal dialog for creating a new preset with name, description, group, and tags.
 */

import React, { useState, useCallback } from 'react';
import { usePresetStore } from '../../stores/preset';

interface NewPresetDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

export function NewPresetDialog({ isOpen, onClose }: NewPresetDialogProps): React.ReactElement | null {
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [group, setGroup] = useState('');
  const [newGroupInput, setNewGroupInput] = useState('');
  const [isCreatingNewGroup, setIsCreatingNewGroup] = useState(false);
  const [tagInput, setTagInput] = useState('');
  const [tags, setTags] = useState<string[]>([]);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const createPreset = usePresetStore((state) => state.createPreset);
  const loadPreset = usePresetStore((state) => state.loadPreset);
  const allGroups = usePresetStore((state) => state.getAllGroups());

  const handleAddTag = useCallback(() => {
    const trimmedTag = tagInput.trim();
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

    const trimmedName = name.trim();
    if (!trimmedName) {
      setError('Name is required');
      return;
    }

    // Determine final group value
    const finalGroup = isCreatingNewGroup
      ? newGroupInput.trim() || undefined
      : group || undefined;

    setIsSubmitting(true);

    try {
      const presetId = await createPreset(trimmedName, description.trim() || undefined, tags, finalGroup);

      if (presetId) {
        // Load the newly created preset so it becomes the active preset
        await loadPreset(presetId);

        // Reset form and close dialog
        setName('');
        setDescription('');
        setGroup('');
        setNewGroupInput('');
        setIsCreatingNewGroup(false);
        setTags([]);
        setTagInput('');
        onClose();
      } else {
        setError('Failed to create preset');
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(message);
    } finally {
      setIsSubmitting(false);
    }
  }, [name, description, group, newGroupInput, isCreatingNewGroup, tags, createPreset, loadPreset, onClose]);

  const handleClose = useCallback(() => {
    if (!isSubmitting) {
      setName('');
      setDescription('');
      setGroup('');
      setNewGroupInput('');
      setIsCreatingNewGroup(false);
      setTags([]);
      setTagInput('');
      setError(null);
      onClose();
    }
  }, [isSubmitting, onClose]);

  if (!isOpen) {
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
          <h2 className="text-lg font-semibold text-gray-200">New Preset</h2>
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
            <label htmlFor="preset-name" className="block text-sm font-medium text-gray-400 mb-1">
              Name *
            </label>
            <input
              id="preset-name"
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
            <label htmlFor="preset-description" className="block text-sm font-medium text-gray-400 mb-1">
              Description
            </label>
            <textarea
              id="preset-description"
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
            <label htmlFor="preset-group" className="block text-sm font-medium text-gray-400 mb-1">
              Group
            </label>
            {!isCreatingNewGroup ? (
              <div className="flex gap-2">
                <select
                  id="preset-group"
                  className="input flex-1"
                  value={group}
                  onChange={(e) => setGroup(e.target.value)}
                  disabled={isSubmitting}
                >
                  <option value="">No Group</option>
                  {allGroups.map((g) => (
                    <option key={g} value={g}>
                      {g}
                    </option>
                  ))}
                </select>
                <button
                  type="button"
                  className="btn btn-ghost text-sm"
                  onClick={() => setIsCreatingNewGroup(true)}
                  disabled={isSubmitting}
                  title="Create new group"
                >
                  + New
                </button>
              </div>
            ) : (
              <div className="flex gap-2">
                <input
                  type="text"
                  className="input flex-1"
                  placeholder="New group name..."
                  value={newGroupInput}
                  onChange={(e) => setNewGroupInput(e.target.value)}
                  disabled={isSubmitting}
                  maxLength={50}
                  autoFocus
                />
                <button
                  type="button"
                  className="btn btn-ghost text-sm"
                  onClick={() => {
                    setIsCreatingNewGroup(false);
                    setNewGroupInput('');
                  }}
                  disabled={isSubmitting}
                >
                  Cancel
                </button>
              </div>
            )}
            <p className="text-xs text-gray-600 mt-1">
              Organize presets into folders for easier management
            </p>
          </div>

          {/* Tags field */}
          <div>
            <label htmlFor="preset-tags" className="block text-sm font-medium text-gray-400 mb-1">
              Tags
            </label>
            <div className="flex gap-2">
              <input
                id="preset-tags"
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
              {isSubmitting ? 'Creating...' : 'Create Preset'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
