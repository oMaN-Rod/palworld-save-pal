// This file provides a TypeScript enum for EntryState for use in the frontend.
// It should match the values in shared/models.py:EntryState.

export enum EntryState {
  NONE = 'None',
  MODIFIED = 'Modified',
  NEW = 'New',
  DELETED = 'Deleted'
}
