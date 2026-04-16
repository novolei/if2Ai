/**
 * Onboarding module — 6-step state machine driven onboarding flow.
 *
 * Entry point: `OnboardingApp` component.
 * State management: `useOnboarding` hook.
 * Types: `types.ts`.
 */

// Root component
export { OnboardingApp } from './OnboardingApp';

// Hook
export { useOnboarding } from './hooks/useOnboarding';

// Types (re-export all)
export type {
  // App state
  AppState,
  OnboardingState,
  // System check
  CheckStatus,
  CpuInfo,
  GpuInfo,
  NodeJsInfo,
  EmbeddedModelStatus,
  SystemReport,
  // Provider
  ProviderCategory,
  ProviderStatus,
  Provider,
  ModelModality,
  Model,
  ProviderConfig,
  ModelSelection,
  // Channel
  ChannelCategory,
  Channel,
  ChannelConfig,
  ChannelConfigRedacted,
  // Test
  TestResult,
  // Activation
  ActivationChecklist,
  ActivationResult,
  // Config
  AppConfig,
  // Hook
  UseOnboardingReturn,
} from './types';

// Components
export { OnboardingLayout } from './components/OnboardingLayout';
export { StepHeader } from './components/StepHeader';
export { StepNavigation } from './components/StepNavigation';
export { StepProgress } from './components/StepProgress';
export { InfoPanel, InfoCard } from './components/InfoPanel';
export { FeatureCard } from './components/FeatureCard';
export { RiskItem } from './components/RiskItem';
export { CheckItem } from './components/CheckItem';
export { DownloadProgress } from './components/DownloadProgress';
export { ProviderCard } from './components/ProviderCard';
export { ModelSelector } from './components/ModelSelector';

// Step pages
export { WelcomeStep } from './steps/WelcomeStep';
export { SecurityConfirmStep } from './steps/SecurityConfirmStep';
export { SystemCheckStep } from './steps/SystemCheckStep';
export { ProviderSetupStep } from './steps/ProviderSetupStep';
