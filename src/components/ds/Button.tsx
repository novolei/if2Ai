import type { ButtonHTMLAttributes, ReactNode } from 'react'
import type { LucideIcon } from 'lucide-react'
import { cn } from '@/lib/utils'

export type ButtonVariant = 'primary' | 'secondary' | 'ghost'
export type ButtonSize = 'sm' | 'md' | 'lg'

export interface ButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'onClick'> {
  variant?: ButtonVariant
  size?: ButtonSize
  label?: string
  Icon?: LucideIcon
  fullWidth?: boolean
  onClick?: () => void
  children?: ReactNode
}

const VARIANT_CLASSES: Record<ButtonVariant, string> = {
  primary:
    'bg-jade text-primary-foreground hover:opacity-90 active:scale-95 border border-transparent',
  secondary:
    'bg-accent text-foreground/75 border border-border hover:bg-secondary hover:text-foreground active:scale-95',
  ghost:
    'bg-transparent text-muted-foreground border border-transparent hover:bg-accent hover:text-foreground active:scale-95',
}

const SIZE_CLASSES: Record<ButtonSize, { box: string; text: string; iconSize: number; gap: string }> = {
  sm: { box: 'h-7 px-3 rounded-md', text: 'text-[11px]', iconSize: 12, gap: 'gap-1.5' },
  md: { box: 'h-8 px-4 rounded-lg', text: 'text-[12px]', iconSize: 13, gap: 'gap-2' },
  lg: { box: 'h-9 px-5 rounded-lg', text: 'text-[13px]', iconSize: 14, gap: 'gap-2.5' },
}

export default function Button({
  variant = 'primary',
  size = 'md',
  label = 'Button',
  Icon,
  disabled = false,
  fullWidth = false,
  onClick,
  children,
  className,
  ...rest
}: ButtonProps) {
  const sizeConfig = SIZE_CLASSES[size]

  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className={cn(
        'inline-flex items-center justify-center font-medium transition-all duration-150 select-none shrink-0',
        sizeConfig.box,
        sizeConfig.text,
        sizeConfig.gap,
        VARIANT_CLASSES[variant],
        disabled ? 'opacity-50 cursor-not-allowed pointer-events-none' : 'cursor-pointer',
        fullWidth && 'w-full',
        className
      )}
      {...rest}
    >
      {Icon && <Icon size={sizeConfig.iconSize} className="shrink-0" />}
      {children ?? label}
    </button>
  )
}
