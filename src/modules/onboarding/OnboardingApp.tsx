/**
 * OnboardingApp — root component for the Onboarding flow.
 *
 * Renders the OnboardingLayout with step-specific content based on
 * the backend AppState. When AppState is `Ready`, this component
 * should not be rendered (main app takes over).
 *
 * This component wires together:
 * - useOnboarding hook for state and operations
 * - OnboardingLayout for 70/30 split
 * - Step-specific page components
 */

import { WelcomeStep } from './steps/WelcomeStep';
import { SecurityConfirmStep } from './steps/SecurityConfirmStep';
import { StepHeader } from './components/StepHeader';
import { StepProgress } from './components/StepProgress';
import { InfoPanel } from './components/InfoPanel';
import { OnboardingLayout } from './components/OnboardingLayout';
import { useOnboarding } from './hooks/useOnboarding';

/** Step title mapping for the left panel header. */
const STEP_TITLES: Record<number, string> = {
  1: 'Welcome to if2AI',
  2: 'Complete system check',
  3: 'Confirm security notes',
  4: 'Select your model provider',
  5: 'Connect your channels',
  6: 'Activate if2AI Agent',
};

/** Step label for the right panel. */
const STEP_LABELS: Record<number, string> = {
  1: 'STEP 1: WELCOME',
  2: 'STEP 2: SYSTEM CHECK',
  3: 'STEP 3: SECURITY',
  4: 'STEP 4: PROVIDER',
  5: 'STEP 5: CHANNEL',
  6: 'STEP 6: ACTIVATION',
};

/** Right panel titles. */
const PANEL_TITLES: Record<number, string> = {
  1: 'Get if2AI ready in minutes.',
  2: 'Check first, install after.',
  3: 'Rules first, then run.',
  4: 'Choose your model source.',
  5: 'Connect your platforms.',
  6: 'All set — activate now.',
};

/** Right panel bullet points per step. */
const PANEL_BULLETS: Record<number, string[]> = {
  1: [
    '6 guided steps to get you started',
    'No privacy concerns — runs locally',
    'Visual management for all configs',
  ],
  2: [
    'Checking CPU, GPU, and Node.js',
    'Downloading embedded model for offline capability',
    'All checks must pass before continuing',
  ],
  3: [
    'Permission security review',
    'Developer channel access',
    'Controllable permissions, zero abuse',
  ],
  4: [
    'Choose from 14+ model providers',
    'Local Ollama or cloud APIs',
    'Test connection before saving',
  ],
  5: [
    'Support for 13 messaging platforms',
    'Real-time connection testing',
    'Auto-configure default routing',
  ],
  6: [
    'Review your complete configuration',
    'One-click activation',
    'Start your first conversation',
  ],
};

/** Placeholder content for steps not yet implemented (6g.11-6g.14). */
function StepPlaceholder({
  step,
  onNext,
  onPrev,
}: {
  step: number;
  onNext: () => void;
  onPrev: () => void;
}) {
  return (
    <OnboardingLayout
      rightPanel={
        <InfoPanel
          stepLabel={STEP_LABELS[step] ?? 'ONBOARDING'}
          title={PANEL_TITLES[step] ?? 'Welcome'}
          bullets={PANEL_BULLETS[step]}
        >
          <div className="mt-4">
            <StepProgress currentStep={step} />
          </div>
        </InfoPanel>
      }
    >
      <StepHeader
        currentStep={step}
        title={STEP_TITLES[step] ?? 'if2AI Onboarding'}
      />
      <div className="flex-1 flex flex-col items-center justify-center px-8">
        <p className="text-token-base text-muted-foreground mb-2">
          Step {step}: {STEP_TITLES[step] ?? 'if2AI Onboarding'}
        </p>
        <p className="text-token-sm text-muted-foreground/60">
          Detailed step page coming soon...
        </p>
      </div>
      <div className="flex items-center justify-between px-8 py-5 border-t border-border">
        <button
          type="button"
          disabled={step <= 1}
          onClick={onPrev}
          className="px-4 py-2 rounded-md text-token-sm font-medium text-muted-foreground hover:text-foreground disabled:text-muted-foreground/40 disabled:cursor-not-allowed"
        >
          Previous
        </button>
        <button
          type="button"
          onClick={onNext}
          className="px-6 py-2.5 rounded-md text-token-sm font-semibold text-white bg-brand-orange hover:bg-brand-orange-dark disabled:bg-brand-orange/50 disabled:text-white/60 disabled:cursor-not-allowed"
        >
          Next
        </button>
      </div>
    </OnboardingLayout>
  );
}

export function OnboardingApp() {
  const {
    appState,
    currentStep,
    nextStep,
    prevStep,
    confirmSecurity,
  } = useOnboarding();

  // If the app is already in Ready state, don't render onboarding.
  if (appState.type === 'Ready') {
    return null;
  }

  const step = currentStep || 1;

  // Render the appropriate step page
  switch (step) {
    case 1:
      return <WelcomeStep onNext={nextStep} />;
    case 3:
      return <SecurityConfirmStep onConfirm={confirmSecurity} onPrev={prevStep} />;
    default:
      return (
        <StepPlaceholder step={step} onNext={nextStep} onPrev={prevStep} />
      );
  }
}
