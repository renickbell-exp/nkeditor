/**
 * Help View Component
 *
 * Comprehensive documentation and help for nkEditor3.
 * Covers all features: Editor, Librarian, Settings, presets, and keyboard shortcuts.
 */

import React, { useState } from 'react';

type HelpSection = 'getting-started' | 'editor' | 'librarian' | 'settings' | 'presets' | 'shortcuts' | 'troubleshooting';

interface SectionNavProps {
  activeSection: HelpSection;
  onSectionChange: (section: HelpSection) => void;
}

function SectionNav({ activeSection, onSectionChange }: SectionNavProps): React.ReactElement {
  const sections: { id: HelpSection; label: string }[] = [
    { id: 'getting-started', label: 'Getting Started' },
    { id: 'editor', label: 'Editor Tab' },
    { id: 'librarian', label: 'Librarian Tab' },
    { id: 'settings', label: 'Settings Tab' },
    { id: 'presets', label: 'Understanding Presets' },
    { id: 'shortcuts', label: 'Keyboard Shortcuts' },
    { id: 'troubleshooting', label: 'Troubleshooting' },
  ];

  return (
    <nav className="mb-6">
      <ul className="flex flex-wrap gap-2">
        {sections.map((section) => (
          <li key={section.id}>
            <button
              onClick={() => onSectionChange(section.id)}
              className={`px-3 py-1.5 text-sm rounded transition-colors ${
                activeSection === section.id
                  ? 'bg-nk-accent text-white'
                  : 'bg-nk-dark text-gray-400 hover:bg-nk-light hover:text-gray-200'
              }`}
            >
              {section.label}
            </button>
          </li>
        ))}
      </ul>
    </nav>
  );
}

export function HelpView(): React.ReactElement {
  const [activeSection, setActiveSection] = useState<HelpSection>('getting-started');

  return (
    <div className="h-full p-4 overflow-auto">
      <div className="max-w-3xl mx-auto">
        <div className="card p-6">
          <h1 className="text-2xl font-bold text-gray-200 mb-6">nkEditor3 Help</h1>

          <SectionNav activeSection={activeSection} onSectionChange={setActiveSection} />

          {/* Getting Started */}
          {activeSection === 'getting-started' && (
            <section>
              <h2 className="text-lg font-semibold text-gray-300 mb-4">Getting Started</h2>

              <div className="text-gray-400 space-y-4">
                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Connecting Your Hardware</h3>
                  <ol className="list-decimal list-inside space-y-2 ml-2">
                    <li>Connect your Korg nanoKONTROL2 via USB</li>
                    <li>The app will automatically detect and connect to the device (if auto-connect is enabled)</li>
                    <li>You can verify the connection status in the Settings tab or the header status indicator</li>
                    <li>If auto-connect is disabled, click "Connect" in Settings to manually connect</li>
                  </ol>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Basic Workflow</h3>
                  <ol className="list-decimal list-inside space-y-2 ml-2">
                    <li>Go to the <strong>Librarian</strong> tab and load a preset (or create a new one)</li>
                    <li>Switch to the <strong>Editor</strong> tab to see the visual controller layout</li>
                    <li>Move any physical control on your nanoKONTROL2 - you will see it reflected on screen</li>
                    <li>Click or right-click any control to edit its CC mapping</li>
                    <li>Save your changes when done</li>
                  </ol>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">What Does nkEditor3 Do?</h3>
                  <p>
                    nkEditor3 is a <strong>MIDI CC remapper</strong>. It sits between your nanoKONTROL2 and your DAW/software,
                    allowing you to remap which CC messages are sent when you move controls. For example, you can make
                    slider 1 send CC 74 (filter cutoff) instead of its default CC 0.
                  </p>
                </div>
              </div>
            </section>
          )}

          {/* Editor Tab */}
          {activeSection === 'editor' && (
            <section>
              <h2 className="text-lg font-semibold text-gray-300 mb-4">Editor Tab</h2>

              <div className="text-gray-400 space-y-4">
                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Visual Controller Representation</h3>
                  <p>
                    The Editor tab displays a visual representation of the Korg nanoKONTROL2. You will see:
                  </p>
                  <ul className="list-disc list-inside space-y-1 ml-2 mt-2">
                    <li><strong>8 Track Strips</strong> - Each with a knob, slider, and three buttons (Solo, Mute, Rec)</li>
                    <li><strong>Transport Section</strong> - Rewind, Forward, Stop, Play, Record, Cycle, and navigation buttons</li>
                  </ul>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Real-Time Feedback</h3>
                  <p>
                    When you move a physical control on the nanoKONTROL2, the on-screen representation updates in real-time.
                    This allows you to verify your hardware is connected and working properly. The current value (0-127)
                    is displayed below each control.
                  </p>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Editing CC Mappings</h3>
                  <p>
                    To edit a control's mapping, use any of these methods:
                  </p>
                  <ul className="list-disc list-inside space-y-1 ml-2 mt-2">
                    <li><strong>Click</strong> on any control to open the mapping editor</li>
                    <li><strong>Right-click</strong> on any control for the context menu</li>
                    <li><strong>Ctrl+Click</strong> (or Cmd+Click on Mac) for quick access</li>
                  </ul>
                  <p className="mt-2">
                    The mapping editor allows you to set:
                  </p>
                  <ul className="list-disc list-inside space-y-1 ml-2 mt-2">
                    <li><strong>Output CC</strong> - The CC number to send to your DAW (0-127)</li>
                    <li><strong>MIDI Channel</strong> - Which channel to send on (1-16)</li>
                    <li><strong>Label</strong> - A custom name for the control (optional)</li>
                    <li><strong>Behavior</strong> - For buttons: Toggle or Momentary</li>
                  </ul>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Saving Changes</h3>
                  <p>
                    When you modify a mapping, an asterisk (*) appears next to the preset name indicating unsaved changes.
                    Click <strong>Save Preset</strong> (or press Ctrl+S) to save changes to the current preset.
                  </p>
                  <p className="mt-2 text-yellow-400">
                    Note: If you want to create a new preset instead of overwriting the current one, go to the Librarian
                    tab and use the "+ New" button, then configure your mappings.
                  </p>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Selection and Randomization</h3>
                  <p>
                    The Editor provides tools for quickly modifying multiple control values:
                  </p>
                  <ul className="list-disc list-inside space-y-1 ml-2 mt-2">
                    <li><strong>Selection checkboxes</strong> - Each knob and slider has a selection checkbox in the corner.
                        Click to toggle selection for batch operations.</li>
                    <li><strong>Select All / Deselect</strong> - Buttons to quickly select or deselect all knobs and sliders.</li>
                    <li><strong>Randomize All</strong> - Randomizes all knob and slider values (0-127). Useful for creating
                        random sound variations or generating starting points for sound design.</li>
                    <li><strong>Randomize Selected</strong> - Only randomizes the selected controls. Allows targeted
                        randomization while preserving other values.</li>
                  </ul>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Undo History</h3>
                  <p>
                    nkEditor3 maintains an undo history of up to <strong>20 steps</strong> for control value changes.
                    This allows you to revert randomizations or accidental modifications.
                  </p>
                  <ul className="list-disc list-inside space-y-1 ml-2 mt-2">
                    <li>Click the <strong>Undo</strong> button or press <strong>Ctrl+Z</strong> to revert the last change</li>
                    <li>The button shows the number of available undo steps</li>
                    <li>Undo history is preserved until you load a different preset</li>
                  </ul>
                </div>
              </div>
            </section>
          )}

          {/* Librarian Tab */}
          {activeSection === 'librarian' && (
            <section>
              <h2 className="text-lg font-semibold text-gray-300 mb-4">Librarian Tab</h2>

              <div className="text-gray-400 space-y-4">
                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Overview</h3>
                  <p>
                    The Librarian is your preset management hub. Here you can browse, create, edit, organize, and delete
                    presets. It also provides Quick Access slots for fast preset switching.
                  </p>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Creating New Presets</h3>
                  <p>
                    Click the <strong>"+ New"</strong> button in the top-right corner to create a new preset. You will be
                    prompted to enter:
                  </p>
                  <ul className="list-disc list-inside space-y-1 ml-2 mt-2">
                    <li><strong>Name</strong> - A unique name for your preset (required)</li>
                    <li><strong>Description</strong> - What this preset is for (optional)</li>
                    <li><strong>Group</strong> - A folder to organize this preset (optional)</li>
                    <li><strong>Tags</strong> - Keywords for searching (optional)</li>
                  </ul>
                  <p className="mt-2">
                    New presets start with the default "identity" mapping (input CCs pass through unchanged).
                    Load the preset and edit it in the Editor tab to customize.
                  </p>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Loading and Selecting Presets</h3>
                  <p>
                    Click on any preset in the list to select and load it. The loaded preset becomes active in the Editor
                    tab, ready for use and editing.
                  </p>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Editing Preset Metadata</h3>
                  <p>
                    Click the edit (pencil) icon on any preset to modify its metadata:
                  </p>
                  <ul className="list-disc list-inside space-y-1 ml-2 mt-2">
                    <li><strong>Name</strong> - Rename the preset</li>
                    <li><strong>Description</strong> - Update the description</li>
                    <li><strong>Group</strong> - Move to a different folder/group</li>
                    <li><strong>Tags</strong> - Add or remove tags</li>
                  </ul>
                  <p className="mt-2">
                    Note: Editing metadata does not change the CC mappings. To edit mappings, load the preset
                    and use the Editor tab.
                  </p>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Duplicating Presets</h3>
                  <p>
                    Click the <strong>Duplicate</strong> button on any preset to create a copy. You will be prompted
                    to enter a name for the new preset. The default name follows a smart naming convention:
                  </p>
                  <ul className="list-disc list-inside space-y-1 ml-2 mt-2">
                    <li>"My Preset" becomes "My Preset copy"</li>
                    <li>"My Preset copy" becomes "My Preset copy 2"</li>
                    <li>"My Preset copy 2" becomes "My Preset copy 3"</li>
                  </ul>
                  <p className="mt-2">
                    Duplicating is useful for creating variations of existing presets while preserving the original.
                  </p>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Deleting Presets</h3>
                  <p>
                    Click the delete (trash) icon on any preset to remove it. You will be asked to confirm
                    before the preset is permanently deleted.
                  </p>
                  <p className="mt-2 text-yellow-400">
                    Warning: Deleted presets cannot be recovered.
                  </p>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Folder/Group Organization</h3>
                  <p>
                    Presets can be organized into groups (like folders). Groups appear as collapsible sections
                    in the preset list. To use groups:
                  </p>
                  <ul className="list-disc list-inside space-y-1 ml-2 mt-2">
                    <li>Assign a group when creating a new preset</li>
                    <li>Edit an existing preset's metadata to change its group</li>
                    <li>Use the group filter dropdown to show only presets in a specific group</li>
                    <li>Select "Ungrouped" to see presets without a group assignment</li>
                  </ul>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Searching and Filtering</h3>
                  <p>
                    Use the search box to find presets by name or description. Use the dropdown filters to
                    narrow by group or tag.
                  </p>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Quick Access Slots</h3>
                  <p>
                    The Quick Access bar provides <strong>5 slots</strong> for your most-used presets. To use:
                  </p>
                  <ul className="list-disc list-inside space-y-1 ml-2 mt-2">
                    <li><strong>Assign:</strong> Select a preset in the list, then right-click a slot to assign it</li>
                    <li><strong>Load:</strong> Click on any occupied slot to instantly load that preset</li>
                    <li><strong>Clear:</strong> Click the "x" button on a slot to remove the assignment</li>
                  </ul>
                  <p className="mt-2">
                    Quick Access slots persist across sessions, so your favorite presets are always ready.
                  </p>
                  <p className="mt-2">
                    <strong>Hardware Integration:</strong> Use the Track Left/Right buttons on the nanoKONTROL2
                    to cycle through your Quick Access presets without touching the computer.
                  </p>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Sample Presets</h3>
                  <p>
                    nkEditor3 includes several sample presets in the "Factory" group to help you get started:
                  </p>
                  <ul className="list-disc list-inside space-y-1 ml-2 mt-2">
                    <li><strong>Default</strong> - Identity mapping (CCs pass through unchanged). Use as a starting point.</li>
                    <li><strong>Synth Lead</strong> - Optimized for synthesizer control with filter, envelope, and modulation mappings.</li>
                    <li><strong>Drums</strong> - Configured for drum machines with tuning, decay, and level controls.</li>
                    <li><strong>DAW Transport</strong> - Standard DAW transport mappings compatible with most DAWs.</li>
                    <li><strong>DJ Mixer</strong> - DJ-style layout with EQ, faders, and cue controls.</li>
                  </ul>
                  <p className="mt-2">
                    These presets are read-only templates. To customize one, load it and use "Save As New" to create your own version.
                  </p>
                </div>
              </div>
            </section>
          )}

          {/* Settings Tab */}
          {activeSection === 'settings' && (
            <section>
              <h2 className="text-lg font-semibold text-gray-300 mb-4">Settings Tab</h2>

              <div className="text-gray-400 space-y-4">
                <div>
                  <h3 className="font-medium text-gray-300 mb-2">MIDI Connection</h3>
                  <ul className="list-disc list-inside space-y-1 ml-2">
                    <li><strong>Status</strong> - Shows whether the nanoKONTROL2 is connected</li>
                    <li><strong>Connect/Disconnect</strong> - Manually control the MIDI connection</li>
                    <li><strong>Auto-connect</strong> - Automatically connect when the app starts</li>
                  </ul>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Soft Takeover</h3>
                  <p>
                    Soft takeover prevents sudden value jumps when switching presets. Because physical knob/slider
                    positions do not change when you load a new preset, the software value may not match the hardware.
                    Soft takeover handles this gracefully.
                  </p>
                  <p className="mt-2"><strong>Available Modes:</strong></p>
                  <ul className="list-disc list-inside space-y-2 ml-2 mt-2">
                    <li>
                      <strong>Catch</strong> - The control only outputs when the physical position matches the
                      software value. Move the control until it "catches" the target.
                    </li>
                    <li>
                      <strong>Jump</strong> - No soft takeover. The control immediately outputs its physical value.
                      This may cause audible parameter jumps but is most responsive.
                    </li>
                    <li>
                      <strong>Pickup</strong> - The control outputs when the physical position crosses the software
                      value. Smoother than jump but more responsive than catch.
                    </li>
                  </ul>
                  <p className="mt-2">
                    <strong>Threshold:</strong> For catch and pickup modes, this sets how close the physical value
                    must be to the target before it "catches." Lower values require more precision.
                  </p>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Appearance</h3>
                  <ul className="list-disc list-inside space-y-1 ml-2">
                    <li><strong>Theme</strong> - Choose Dark, Light, or System (follows OS setting)</li>
                    <li><strong>Value Display</strong> - Show MIDI values as Decimal (0-127) or Hexadecimal (00-7F)</li>
                  </ul>
                </div>
              </div>
            </section>
          )}

          {/* Understanding Presets */}
          {activeSection === 'presets' && (
            <section>
              <h2 className="text-lg font-semibold text-gray-300 mb-4">Understanding Presets</h2>

              <div className="text-gray-400 space-y-4">
                <div className="p-4 bg-nk-darker rounded border border-nk-border">
                  <h3 className="font-medium text-nk-accent mb-2">Key Concept</h3>
                  <p>
                    <strong>Presets store MAPPINGS, not control values.</strong>
                  </p>
                  <p className="mt-2">
                    A preset defines <em>how</em> your controls are routed (which CC numbers they send), not the current
                    positions of your knobs and sliders. When you load a preset, your physical controls remain where they are -
                    only the CC routing changes.
                  </p>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">What a Preset Contains</h3>
                  <p>Each preset stores:</p>
                  <ul className="list-disc list-inside space-y-1 ml-2 mt-2">
                    <li><strong>Metadata</strong> - Name, description, group, tags, timestamps</li>
                    <li><strong>Mappings</strong> - For each control: output CC, MIDI channel, label, button behavior</li>
                  </ul>
                  <p className="mt-2">
                    When you move a slider, nkEditor3 looks up its mapping and sends the configured output CC
                    on the configured channel - regardless of what the hardware's "native" CC would be.
                  </p>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Save Preset vs Save As New</h3>
                  <table className="w-full mt-2 text-sm">
                    <thead>
                      <tr className="border-b border-nk-border">
                        <th className="text-left py-2 text-gray-300">Action</th>
                        <th className="text-left py-2 text-gray-300">What It Does</th>
                        <th className="text-left py-2 text-gray-300">When to Use</th>
                      </tr>
                    </thead>
                    <tbody>
                      <tr className="border-b border-nk-border">
                        <td className="py-2 font-medium">Save Preset</td>
                        <td className="py-2">Overwrites the currently loaded preset with your changes</td>
                        <td className="py-2">Updating an existing preset you created</td>
                      </tr>
                      <tr className="border-b border-nk-border">
                        <td className="py-2 font-medium">Save As New</td>
                        <td className="py-2">Creates a new preset, leaving the original unchanged</td>
                        <td className="py-2">Creating a variation or starting from a template</td>
                      </tr>
                    </tbody>
                  </table>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Example Use Case</h3>
                  <p>
                    Suppose you have multiple hardware synthesizers like an ASM Hydrasynth and a Novation BassStation II.
                    You might create:
                  </p>
                  <ul className="list-disc list-inside space-y-1 ml-2 mt-2">
                    <li>"Hydrasynth Control" preset - Knobs mapped to the Hydrasynth's filter, oscillator mix, and modulation CCs</li>
                    <li>"BassStation II" preset - Knobs mapped to the BassStation's filter frequency (CC74), resonance (CC71), and LFO rate</li>
                  </ul>
                  <p className="mt-2">
                    Switching presets instantly reconfigures your nanoKONTROL2 for each synth.
                    Your physical slider positions stay the same - only the CC routing changes.
                  </p>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Identity Mapping</h3>
                  <p>
                    The default "identity" mapping passes CC values through unchanged - input CC equals output CC.
                    This is useful as a starting point or when you want the nanoKONTROL2's standard behavior.
                  </p>
                </div>
              </div>
            </section>
          )}

          {/* Keyboard Shortcuts */}
          {activeSection === 'shortcuts' && (
            <section>
              <h2 className="text-lg font-semibold text-gray-300 mb-4">Keyboard Shortcuts</h2>

              <div className="text-gray-400">
                <table className="w-full">
                  <thead>
                    <tr className="border-b border-nk-border">
                      <th className="text-left py-2 text-gray-300 w-40">Shortcut</th>
                      <th className="text-left py-2 text-gray-300">Action</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr className="border-b border-nk-border">
                      <td className="py-2 font-mono text-sm">Ctrl+S</td>
                      <td className="py-2">Save current preset</td>
                    </tr>
                    <tr className="border-b border-nk-border">
                      <td className="py-2 font-mono text-sm">Ctrl+Z</td>
                      <td className="py-2">Undo last change (up to 20 steps)</td>
                    </tr>
                    <tr className="border-b border-nk-border">
                      <td className="py-2 font-mono text-sm">1 - 8</td>
                      <td className="py-2">Select track 1-8 in the editor</td>
                    </tr>
                    <tr className="border-b border-nk-border">
                      <td className="py-2 font-mono text-sm">A</td>
                      <td className="py-2">Toggle A/B comparison mode</td>
                    </tr>
                    <tr className="border-b border-nk-border">
                      <td className="py-2 font-mono text-sm">Escape</td>
                      <td className="py-2">Close popover/dialog</td>
                    </tr>
                  </tbody>
                </table>

                <div className="mt-6">
                  <h3 className="font-medium text-gray-300 mb-2">Hardware Shortcuts (nanoKONTROL2)</h3>
                  <table className="w-full">
                    <thead>
                      <tr className="border-b border-nk-border">
                        <th className="text-left py-2 text-gray-300 w-40">Button</th>
                        <th className="text-left py-2 text-gray-300">Action</th>
                      </tr>
                    </thead>
                    <tbody>
                      <tr className="border-b border-nk-border">
                        <td className="py-2 font-medium">Track Left</td>
                        <td className="py-2">Switch to previous Quick Access preset</td>
                      </tr>
                      <tr className="border-b border-nk-border">
                        <td className="py-2 font-medium">Track Right</td>
                        <td className="py-2">Switch to next Quick Access preset</td>
                      </tr>
                    </tbody>
                  </table>
                </div>

                <div className="mt-6">
                  <h3 className="font-medium text-gray-300 mb-2">Mouse Actions</h3>
                  <table className="w-full">
                    <thead>
                      <tr className="border-b border-nk-border">
                        <th className="text-left py-2 text-gray-300 w-40">Action</th>
                        <th className="text-left py-2 text-gray-300">Result</th>
                      </tr>
                    </thead>
                    <tbody>
                      <tr className="border-b border-nk-border">
                        <td className="py-2 font-medium">Click control</td>
                        <td className="py-2">Open mapping editor for that control</td>
                      </tr>
                      <tr className="border-b border-nk-border">
                        <td className="py-2 font-medium">Right-click control</td>
                        <td className="py-2">Open mapping editor (alternative)</td>
                      </tr>
                      <tr className="border-b border-nk-border">
                        <td className="py-2 font-medium">Ctrl+Click control</td>
                        <td className="py-2">Open mapping editor (alternative)</td>
                      </tr>
                      <tr className="border-b border-nk-border">
                        <td className="py-2 font-medium">Drag knob up/down</td>
                        <td className="py-2">Adjust the control value</td>
                      </tr>
                      <tr className="border-b border-nk-border">
                        <td className="py-2 font-medium">Right-click Quick Access slot</td>
                        <td className="py-2">Assign selected preset to that slot</td>
                      </tr>
                    </tbody>
                  </table>
                </div>
              </div>
            </section>
          )}

          {/* Troubleshooting */}
          {activeSection === 'troubleshooting' && (
            <section>
              <h2 className="text-lg font-semibold text-gray-300 mb-4">Troubleshooting</h2>

              <div className="text-gray-400 space-y-4">
                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Device Not Detected</h3>
                  <ul className="list-disc list-inside space-y-1 ml-2">
                    <li>Ensure the nanoKONTROL2 is connected via USB and powered on</li>
                    <li>Close any other applications that might be using the device (DAWs, other MIDI utilities)</li>
                    <li>Try unplugging and reconnecting the USB cable</li>
                    <li>Check your operating system's MIDI settings to confirm the device is recognized</li>
                    <li>On Linux, ensure your user has permission to access MIDI devices (audio group)</li>
                  </ul>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">LEDs Not Responding</h3>
                  <ul className="list-disc list-inside space-y-1 ml-2">
                    <li>Ensure the nanoKONTROL2 is in CC mode, not MIDI note mode</li>
                    <li>Use the Korg Kontrol Editor software to check/change the device mode</li>
                    <li>The LED control uses SysEx messages - verify SysEx is enabled if using MIDI routing</li>
                  </ul>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Controls Jumping or Skipping Values</h3>
                  <ul className="list-disc list-inside space-y-1 ml-2">
                    <li>Enable soft takeover in Settings (use "Catch" or "Pickup" mode)</li>
                    <li>This happens when the physical control position does not match the software value</li>
                    <li>After loading a preset, slowly move controls until they "catch" the target</li>
                    <li>If jumps are acceptable, use "Jump" mode for most responsive feel</li>
                  </ul>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">MIDI Not Reaching DAW</h3>
                  <ul className="list-disc list-inside space-y-1 ml-2">
                    <li>Ensure your DAW is configured to receive from nkEditor3's virtual MIDI output</li>
                    <li>Check that you have a preset loaded with valid mappings</li>
                    <li>Verify the MIDI channel in your preset matches what your DAW expects</li>
                    <li>Look at the editor - controls should update when you move physical knobs/sliders</li>
                  </ul>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">MIDI Not Reaching Hardware Synth</h3>
                  <p className="mb-2">
                    If you're using nkEditor3 to control external hardware synthesizers (like an ASM Hydrasynth or
                    Novation BassStation II) and MIDI messages aren't reaching your synth:
                  </p>
                  <ul className="list-disc list-inside space-y-1 ml-2">
                    <li><strong>Check MIDI routing:</strong> Ensure nkEditor3's virtual MIDI output is routed to your
                        hardware synth's MIDI input. You may need a MIDI interface or DAW to route the signal.</li>
                    <li><strong>Verify MIDI channel:</strong> Most hardware synths listen on a specific MIDI channel.
                        Check your synth's settings and ensure your nkEditor3 preset uses the same channel.</li>
                    <li><strong>Check CC numbers:</strong> Verify that the CC numbers in your preset match what your
                        synth expects. Consult your synth's MIDI implementation chart for correct CC assignments.</li>
                    <li><strong>Test with MIDI monitor:</strong> Use a MIDI monitor app to verify that nkEditor3 is
                        actually sending MIDI data. If you see data in the monitor but not on the synth, the issue
                        is in the routing between the monitor output and the synth.</li>
                    <li><strong>Physical connections:</strong> If using DIN MIDI cables, ensure they're firmly connected.
                        Try swapping cables to rule out cable faults. Remember: MIDI Out from your interface goes to
                        MIDI In on your synth.</li>
                    <li><strong>USB MIDI:</strong> If your synth has USB MIDI, try connecting it directly to your
                        computer and selecting it as an output device in your MIDI routing software.</li>
                    <li><strong>Synth MIDI settings:</strong> Some synths have local control settings or require
                        enabling MIDI CC reception. Check your synth's global settings menu.</li>
                  </ul>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Preset Changes Not Saving</h3>
                  <ul className="list-disc list-inside space-y-1 ml-2">
                    <li>Check for the asterisk (*) next to the preset name indicating unsaved changes</li>
                    <li>Click "Save Preset" or press Ctrl+S to save</li>
                    <li>Factory presets may be read-only - use "Save As New" to create an editable copy</li>
                    <li>Ensure you have write permissions to the presets directory</li>
                  </ul>
                </div>

                <div>
                  <h3 className="font-medium text-gray-300 mb-2">Application Crashes or Freezes</h3>
                  <ul className="list-disc list-inside space-y-1 ml-2">
                    <li>Check the application logs for error messages</li>
                    <li>Try restarting the application</li>
                    <li>Reset settings by deleting the config file (back up your presets first)</li>
                    <li>Update to the latest version of nkEditor3</li>
                  </ul>
                </div>

                <div className="mt-6 p-4 bg-nk-darker rounded border border-nk-border">
                  <h3 className="font-medium text-gray-300 mb-2">Still Having Issues?</h3>
                  <p>
                    If you continue to experience problems, please report an issue on the project's GitHub page
                    with details about your operating system, nkEditor3 version, and steps to reproduce the problem.
                  </p>
                </div>
              </div>
            </section>
          )}
        </div>
      </div>
    </div>
  );
}
