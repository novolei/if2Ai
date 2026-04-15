import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "@/lib/utils"

const badgeVariants = cva(
  "inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-semibold transition-colors focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2",
  {
    variants: {
      variant: {
        default:
          "border-transparent bg-primary text-primary-foreground shadow hover:bg-primary/80",
        secondary:
          "border-transparent bg-secondary text-secondary-foreground hover:bg-secondary/80",
        destructive:
          "border-transparent bg-destructive text-destructive-foreground shadow hover:bg-destructive/80",
        outline: "text-foreground",
        // Paico status color variants
        "status-active": "border-transparent bg-[var(--status-active-bg)] text-[var(--status-active)]",
        "status-pending": "border-transparent bg-[var(--status-pending-bg)] text-[var(--status-pending)]",
        "status-warning": "border-transparent bg-[var(--status-warning-bg)] text-[var(--status-warning)]",
        "status-success": "border-transparent bg-[var(--status-success-bg)] text-[var(--status-success)]",
        "status-error": "border-transparent bg-[var(--status-error-bg)] text-[var(--status-error)]",
        "status-neutral": "border-transparent bg-[var(--status-neutral-bg)] text-[var(--status-neutral)]",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  }
)

export interface BadgeProps
  extends React.HTMLAttributes<HTMLDivElement>,
    VariantProps<typeof badgeVariants> {}

function Badge({ className, variant, ...props }: BadgeProps) {
  return (
    <div className={cn(badgeVariants({ variant }), className)} {...props} />
  )
}

export { Badge, badgeVariants }
