/**
 * Librarian View Component
 *
 * Preset management view for browsing, loading, and organizing presets.
 */

import React, { useState, useEffect, useCallback, useMemo } from 'react';
import { PresetList } from './PresetList';
import { QuickAccess } from './QuickAccess';
import { NewPresetDialog } from './NewPresetDialog';
import { EditPresetDialog } from './EditPresetDialog';
import { EditControlValuesDialog } from './EditControlValuesDialog';
import { DuplicatePresetDialog } from './DuplicatePresetDialog';
import { usePresetStore } from '../../stores/preset';
import type { PresetMetadata, Preset } from '@shared/ipc-protocol';

/** Sort field options */
export type SortField = 'name' | 'createdAt' | 'modifiedAt';
/** Sort direction */
export type SortDirection = 'asc' | 'desc';

export interface SortConfig {
  field: SortField;
  direction: SortDirection;
}

/** Sort presets by the given config */
export function sortPresets(presets: PresetMetadata[], config: SortConfig): PresetMetadata[] {
  return [...presets].sort((a, b) => {
    let cmp: number;
    if (config.field === 'name') {
      cmp = a.name.localeCompare(b.name, undefined, { sensitivity: 'base' });
    } else {
      cmp = a[config.field].localeCompare(b[config.field]);
    }
    return config.direction === 'desc' ? -cmp : cmp;
  });
}

/** Special constant for filtering ungrouped presets */
const UNGROUPED_FILTER = '__ungrouped__';

interface LibrarianViewProps {
  compact?: boolean;
}

export function LibrarianView({ compact = false }: LibrarianViewProps): React.ReactElement {
  const [searchQuery, setSearchQuery] = useState('');
  const [filterTag, setFilterTag] = useState<string | null>(null);
  const [filterGroup, setFilterGroup] = useState<string | null>(null);
  const [sortConfig, setSortConfig] = useState<SortConfig>({ field: 'name', direction: 'asc' });

  const [isNewPresetDialogOpen, setIsNewPresetDialogOpen] = useState(false);
  const [editingPreset, setEditingPreset] = useState<PresetMetadata | null>(null);
  const [editingControlValuesPreset, setEditingControlValuesPreset] = useState<Preset | null>(null);
  const [duplicatingPreset, setDuplicatingPreset] = useState<PresetMetadata | null>(null);

  const presets = usePresetStore((state) => state.presets);
  const loadPresets = usePresetStore((state) => state.loadPresets);
  const allTags = usePresetStore((state) => state.getAllTags());
  const allGroups = usePresetStore((state) => state.getAllGroups());

  const handleOpenNewPresetDialog = useCallback(() => {
    setIsNewPresetDialogOpen(true);
  }, []);

  const handleCloseNewPresetDialog = useCallback(() => {
    setIsNewPresetDialogOpen(false);
  }, []);

  const handleOpenEditPresetDialog = useCallback((preset: PresetMetadata) => {
    setEditingPreset(preset);
  }, []);

  const handleCloseEditPresetDialog = useCallback(() => {
    setEditingPreset(null);
  }, []);

  const handleOpenEditControlValuesDialog = useCallback((preset: Preset) => {
    setEditingControlValuesPreset(preset);
  }, []);

  const handleCloseEditControlValuesDialog = useCallback(() => {
    setEditingControlValuesPreset(null);
    // Reload presets to reflect any changes
    loadPresets();
  }, [loadPresets]);

  const handleOpenDuplicateDialog = useCallback((preset: PresetMetadata) => {
    setDuplicatingPreset(preset);
  }, []);

  const handleCloseDuplicateDialog = useCallback(() => {
    setDuplicatingPreset(null);
  }, []);

  // Load presets on mount
  useEffect(() => {
    loadPresets();
  }, [loadPresets]);

  // Filter presets by search, tag, and group, then sort
  const filteredPresets = useMemo(() => {
    const filtered = presets.filter((preset) => {
      const matchesSearch =
        !searchQuery ||
        preset.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        preset.description?.toLowerCase().includes(searchQuery.toLowerCase());

      const matchesTag = !filterTag || preset.tags.includes(filterTag);

      const matchesGroup =
        !filterGroup ||
        (filterGroup === UNGROUPED_FILTER ? !preset.group : preset.group === filterGroup);

      return matchesSearch && matchesTag && matchesGroup;
    });
    return sortPresets(filtered, sortConfig);
  }, [presets, searchQuery, filterTag, filterGroup, sortConfig]);

  // Group presets by their group field for hierarchical display
  const groupedPresets = useMemo(() => {
    const groups: Record<string, PresetMetadata[]> = {};
    const ungrouped: PresetMetadata[] = [];

    for (const preset of filteredPresets) {
      if (preset.group) {
        const groupName = preset.group;
        if (!groups[groupName]) {
          groups[groupName] = [];
        }
        // TypeScript now knows groups[groupName] is defined due to the check above
        const groupArray = groups[groupName];
        if (groupArray) {
          groupArray.push(preset);
        }
      } else {
        ungrouped.push(preset);
      }
    }

    return { groups, ungrouped };
  }, [filteredPresets]);

  // Determine if we should show grouped view (when not filtering by a specific group)
  const showGroupedView = !filterGroup && !compact && Object.keys(groupedPresets.groups).length > 0;

  return (
    <div className={`h-full flex flex-col ${compact ? 'p-2' : 'p-4'}`}>
      <div className={`card flex-1 flex flex-col ${compact ? 'p-2' : 'p-4'}`}>
        {/* Header */}
        <div className="flex items-center justify-between mb-4">
          <h2 className={`font-semibold text-gray-200 ${compact ? 'text-base' : 'text-lg'}`}>
            Preset Library
          </h2>
          <button
            className="btn btn-primary text-sm"
            onClick={handleOpenNewPresetDialog}
          >
            + New
          </button>
        </div>

        {/* Quick Access */}
        {!compact && <QuickAccess />}

        {/* Search and filters */}
        <div className={`flex gap-2 flex-wrap ${compact ? 'mb-2' : 'mb-4'}`}>
          <input
            type="text"
            placeholder="Search presets..."
            className="input flex-1 text-sm min-w-[150px]"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          />
          {!compact && (
            <>
              {/* Group filter */}
              <select
                className="input text-sm w-36"
                value={filterGroup ?? ''}
                onChange={(e) => setFilterGroup(e.target.value || null)}
                title="Filter by group"
              >
                <option value="">All Groups</option>
                <option value={UNGROUPED_FILTER}>Ungrouped</option>
                {allGroups.map((group) => (
                  <option key={group} value={group}>
                    {group}
                  </option>
                ))}
              </select>
              {/* Tag filter */}
              <select
                className="input text-sm w-32"
                value={filterTag ?? ''}
                onChange={(e) => setFilterTag(e.target.value || null)}
                title="Filter by tag"
              >
                <option value="">All Tags</option>
                {allTags.map((tag) => (
                  <option key={tag} value={tag}>
                    {tag}
                  </option>
                ))}
              </select>
            </>
          )}
        </div>

        {/* Sort controls */}
        {!compact && (
          <SortControls sortConfig={sortConfig} onSortChange={setSortConfig} />
        )}

        {/* Preset list - grouped or flat */}
        <div className="flex-1 overflow-auto">
          {showGroupedView ? (
            <GroupedPresetList
              groupedPresets={groupedPresets}
              sortConfig={sortConfig}
              onEditPreset={handleOpenEditPresetDialog}
              onEditControlValues={handleOpenEditControlValuesDialog}
              onDuplicatePreset={handleOpenDuplicateDialog}
            />
          ) : (
            <PresetList
              presets={filteredPresets}
              compact={compact}
              onEditPreset={compact ? undefined : handleOpenEditPresetDialog}
              onEditControlValues={compact ? undefined : handleOpenEditControlValuesDialog}
              onDuplicatePreset={compact ? undefined : handleOpenDuplicateDialog}
            />
          )}
        </div>

        {/* Footer stats */}
        <div className="mt-2 text-xs text-gray-500">
          {filteredPresets.length} of {presets.length} presets
          {filterGroup && filterGroup !== UNGROUPED_FILTER && (
            <span className="ml-2">in "{filterGroup}"</span>
          )}
          {filterGroup === UNGROUPED_FILTER && (
            <span className="ml-2">(ungrouped)</span>
          )}
        </div>
      </div>

      {/* New Preset Dialog */}
      <NewPresetDialog
        isOpen={isNewPresetDialogOpen}
        onClose={handleCloseNewPresetDialog}
      />

      {/* Edit Preset Dialog */}
      <EditPresetDialog
        isOpen={editingPreset !== null}
        onClose={handleCloseEditPresetDialog}
        preset={editingPreset}
      />

      {/* Edit Control Values Dialog */}
      <EditControlValuesDialog
        isOpen={editingControlValuesPreset !== null}
        onClose={handleCloseEditControlValuesDialog}
        preset={editingControlValuesPreset}
      />

      {/* Duplicate Preset Dialog */}
      <DuplicatePresetDialog
        isOpen={duplicatingPreset !== null}
        onClose={handleCloseDuplicateDialog}
        preset={duplicatingPreset}
      />
    </div>
  );
}

// =============================================================================
// Grouped Preset List Component
// =============================================================================

// =============================================================================
// Sort Controls Component
// =============================================================================

interface SortControlsProps {
  sortConfig: SortConfig;
  onSortChange: (config: SortConfig) => void;
}

function SortControls({ sortConfig, onSortChange }: SortControlsProps): React.ReactElement {
  const handleFieldChange = (field: SortField) => {
    if (sortConfig.field === field) {
      // Toggle direction if same field
      onSortChange({ field, direction: sortConfig.direction === 'asc' ? 'desc' : 'asc' });
    } else {
      onSortChange({ field, direction: 'asc' });
    }
  };

  const fieldLabel = (field: SortField): string => {
    switch (field) {
      case 'name': return 'Name';
      case 'createdAt': return 'Created';
      case 'modifiedAt': return 'Modified';
    }
  };

  const arrow = sortConfig.direction === 'asc' ? '\u2191' : '\u2193';

  return (
    <div className="flex items-center gap-1 mb-2 text-xs">
      <span className="text-gray-500 mr-1">Sort:</span>
      {(['name', 'createdAt', 'modifiedAt'] as SortField[]).map((field) => (
        <button
          key={field}
          className={`px-2 py-0.5 rounded transition-colors ${
            sortConfig.field === field
              ? 'bg-nk-accent/20 text-nk-accent border border-nk-accent/40'
              : 'bg-nk-light text-gray-400 hover:text-gray-300 border border-transparent'
          }`}
          onClick={() => handleFieldChange(field)}
        >
          {fieldLabel(field)}{sortConfig.field === field ? ` ${arrow}` : ''}
        </button>
      ))}
    </div>
  );
}

interface GroupedPresetListProps {
  groupedPresets: {
    groups: Record<string, PresetMetadata[]>;
    ungrouped: PresetMetadata[];
  };
  sortConfig: SortConfig;
  onEditPreset?: (preset: PresetMetadata) => void;
  onEditControlValues?: (preset: Preset) => void;
  onDuplicatePreset?: (preset: PresetMetadata) => void;
}

/**
 * Displays presets organized by group with collapsible sections.
 */
function GroupedPresetList({ groupedPresets, sortConfig, onEditPreset, onEditControlValues, onDuplicatePreset }: GroupedPresetListProps): React.ReactElement {
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(new Set());

  const toggleGroupCollapse = useCallback((groupName: string) => {
    setCollapsedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(groupName)) {
        next.delete(groupName);
      } else {
        next.add(groupName);
      }
      return next;
    });
  }, []);

  const sortedGroupNames = Object.keys(groupedPresets.groups).sort();

  return (
    <div className="space-y-3">
      {/* Grouped presets */}
      {sortedGroupNames.map((groupName) => {
        const presetsInGroup = sortPresets(groupedPresets.groups[groupName] ?? [], sortConfig);
        const isCollapsed = collapsedGroups.has(groupName);

        return (
          <div key={groupName} className="border border-nk-border rounded">
            {/* Group header */}
            <button
              className="w-full flex items-center justify-between px-3 py-2 bg-nk-light hover:bg-nk-border transition-colors rounded-t"
              onClick={() => toggleGroupCollapse(groupName)}
            >
              <div className="flex items-center gap-2">
                <span className="text-gray-400 text-sm">{isCollapsed ? '\u25B6' : '\u25BC'}</span>
                <span className="font-medium text-gray-200">{groupName}</span>
                <span className="text-xs text-gray-500">({presetsInGroup.length})</span>
              </div>
            </button>

            {/* Group content */}
            {!isCollapsed && (
              <div className="p-2">
                <PresetList presets={presetsInGroup} compact={false} onEditPreset={onEditPreset} onEditControlValues={onEditControlValues} onDuplicatePreset={onDuplicatePreset} />
              </div>
            )}
          </div>
        );
      })}

      {/* Ungrouped presets */}
      {groupedPresets.ungrouped.length > 0 && (
        <div className="border border-nk-border rounded">
          <div className="px-3 py-2 bg-nk-light rounded-t">
            <div className="flex items-center gap-2">
              <span className="font-medium text-gray-400">Ungrouped</span>
              <span className="text-xs text-gray-500">({groupedPresets.ungrouped.length})</span>
            </div>
          </div>
          <div className="p-2">
            <PresetList presets={sortPresets(groupedPresets.ungrouped, sortConfig)} compact={false} onEditPreset={onEditPreset} onEditControlValues={onEditControlValues} onDuplicatePreset={onDuplicatePreset} />
          </div>
        </div>
      )}
    </div>
  );
}
