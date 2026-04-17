import type { ReactNode } from "react";
import type { LucideIcon } from "lucide-react";

// ─── Types ────────────────────────────────────────────────────────────────────

export type ButtonVariant = "primary" | "secondary" | "ghost";
export type ButtonSize    = "sm" | "md" | "lg";

export interface ButtonProps {
  /** Visual style variant */
  variant?: ButtonVariant;
  /** Size preset */
  size?: ButtonSize;
  /** Label text */
  label?: string;
  /** Lucide icon component (before label) */
  Icon?: LucideIcon;
  /** Whether button is disabled */
  disabled?: boolean;
  /** Full-width */
  fullWidth?: boolean;
  /** Click handler */
  onClick?: () => void;
  /** Optional children override (rendered instead of label) */
  children?: ReactNode;
}

// ─── Style maps ───────────────────────────────────────────────────────────────
/*
 * Variants
 * ──────────────────────────────────────────────────────────────────────────────
 * primary   bg-jade text-primary-foreground
 *           hover: opacity-90   active: scale-95
 *           disabled: opacity-50 cursor-not-allowed
 *
 * secondary bg-accent text-foreground/75 border border-border
 *           hover: bg-secondary text-foreground
 *           active: scale-95
 *           disabled: opacity-50 cursor-not-allowed
 *
 * ghost     transparent text-muted-foreground
 *           hover: bg-accent text-foreground
 *           active: scale-95
 *           disabled: opacity-50 cursor-not-allowed
 *
 * Sizes
 * ──────────────────────────────────────────────────────────────────────────────
 * sm   h-7  px-3  text-[11px] gap-1.5  icon-size=12  rounded-md
 * md   h-8  px-4  text-[12px] gap-2    icon-size=13  rounded-lg
 * lg   h-9  px-5  text-[13px] gap-2.5  icon-size=14  rounded-lg
 */

const VARIANT_CLASSES: Record<ButtonVariant, string> = {
  primary:
    "bg-jade text-primary-foreground hover:opacity-90 active:scale-95 border border-transparent",
  secondary:
    "bg-accent text-foreground/75 border border-border hover:bg-secondary hover:text-foreground active:scale-95",
  ghost:
    "bg-transparent text-muted-foreground border border-transparent hover:bg-accent hover:text-foreground active:scale-95",
};

const SIZE_CLASSES: Record<ButtonSize, { box: string; text: string; iconSize: number; gap: string }> = {
  sm: { box: "h-7 px-3 rounded-md",  text: "text-[11px]", iconSize: 12, gap: "gap-1.5" },
  md: { box: "h-8 px-4 rounded-lg",  text: "text-[12px]", iconSize: 13, gap: "gap-2"   },
  lg: { box: "h-9 px-5 rounded-lg",  text: "text-[13px]", iconSize: 14, gap: "gap-2.5" },
};

// ─── Component ────────────────────────────────────────────────────────────────

export default function Button({
  variant  = "primary",
  size     = "md",
  label    = "Button",
  Icon,
  disabled = false,
  fullWidth = false,
  onClick  = () => {},
  children,
}: ButtonProps) {
  const sizeConfig = SIZE_CLASSES[size];

  return (
    <button
      data-cmp="Button"
      data-variant={variant}
      data-size={size}
      disabled={disabled}
      onClick={() => {
        console.log("Button clicked:", label);
        onClick();
      }}
      className={[
        "inline-flex items-center justify-center font-medium",
        "transition-all duration-150 select-none",
        "shrink-0",
        sizeConfig.box,
        sizeConfig.text,
        sizeConfig.gap,
        VARIANT_CLASSES[variant],
        disabled ? "opacity-50 cursor-not-allowed pointer-events-none" : "cursor-pointer",
        fullWidth ? "w-full" : "",
      ]
        .filter(Boolean)
        .join(" ")}
    >
      {Icon && <Icon size={sizeConfig.iconSize} className="shrink-0" />}
      {children ?? label}
    </button>
  );
}
