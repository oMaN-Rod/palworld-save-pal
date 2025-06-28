// Map of Pal Preset property names to their labels and descriptions
// Extend as needed for all properties used in PalPresetConfig

export type PalPresetPropertyNames =
  | 'lock'
  | 'lock_element'
  | 'some_other_property'; // Add all relevant property keys here

export interface PalPresetNameDescription {
  label: string;
  description: string;
}

export const palPresetNameDescriptionMap: Record<PalPresetPropertyNames, PalPresetNameDescription> = {
  lock: {
    label: 'Lock',
    description: 'Prevents changes to this preset.'
  },
  lock_element: {
    label: 'Lock Element',
    description: 'Prevents changes to the element type.'
  },
  some_other_property: {
    label: 'Other Property',
    description: 'Description for other property.'
  }
  // Add more properties as needed
};

// Define the PalPresetConfig type for use in defaultPresetConfig and elsewhere
export type PalPresetConfig = Record<PalPresetPropertyNames, boolean>;
