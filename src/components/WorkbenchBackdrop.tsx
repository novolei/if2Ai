import { cn } from '@/lib/utils'

export function WorkbenchBackdrop({ className }: { className?: string }) {
  return (
    <div aria-hidden="true" className={cn('pointer-events-none fixed inset-0 z-0 overflow-hidden', className)}>
      <div className="absolute inset-0 bg-[#f6f7f8]" />
      <div className="absolute inset-0 bg-[radial-gradient(circle_at_50%_18%,rgba(255,255,255,0.85),transparent_52%),radial-gradient(circle_at_50%_92%,rgba(242,244,246,0.95),rgba(242,244,246,0.7)_48%,rgba(242,244,246,0.3)_72%,transparent_100%)]" />
      <div className="absolute inset-0 opacity-[0.32] [background-image:radial-gradient(rgba(182,191,198,0.28)_1px,transparent_1px)] [background-size:20px_20px] [mask-image:radial-gradient(ellipse_at_center,black_44%,transparent_100%)]" />
      <div className="absolute inset-0 bg-[linear-gradient(to_bottom,rgba(246,247,248,0.06),rgba(246,247,248,0.16)_60%,rgba(246,247,248,0.88)_100%)]" />
      <div className="absolute inset-x-0 bottom-0 h-48 bg-[radial-gradient(circle_at_50%_100%,rgba(255,255,255,0.5),transparent_58%)]" />
    </div>
  )
}
