// This file provides a TypeScript enum for TargetType for use in the frontend.
// This should match the target types used in your app logic and backend.

export enum TargetType {
  ToSelf = 'ToSelf',
  ToTrainer = 'ToTrainer',
  ToSelfAndTrainer = 'ToSelfAndTrainer',
  ToBaseCampPal = 'ToBaseCampPal',
  ToBuildObject = 'ToBuildObject',
  EPalPassiveSkillEffectTargetType_MAX = 'EPalPassiveSkillEffectTargetType_MAX',
  NONE = 'None'
}
