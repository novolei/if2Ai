import { Component, type ReactNode } from 'react'
import { AlertCircle } from 'lucide-react'

type ErrorBoundaryProps = {
  children: ReactNode
  fallback?: ReactNode | ((errorMessage?: string) => ReactNode)
}

type ErrorBoundaryState = {
  hasError: boolean
  errorMessage?: string
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { hasError: false, errorMessage: undefined }

  static getDerivedStateFromError() {
    return { hasError: true }
  }

  componentDidCatch(error: unknown) {
    console.error('ErrorBoundary caught an error:', error)
    const errorMessage = error instanceof Error ? error.message : String(error)
    this.setState({ errorMessage })
  }

  componentDidUpdate(prevProps: ErrorBoundaryProps) {
    if (this.state.hasError && prevProps.children !== this.props.children) {
      this.setState({ hasError: false, errorMessage: undefined })
    }
  }

  render() {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return typeof this.props.fallback === 'function'
          ? this.props.fallback(this.state.errorMessage)
          : this.props.fallback
      }

      return (
        <div className="flex h-full min-h-0 items-center justify-center p-6">
          <div className="max-w-sm rounded-3xl border border-black/5 bg-white px-6 py-5 text-center shadow-sm">
            <div className="mx-auto mb-3 flex h-11 w-11 items-center justify-center rounded-2xl bg-red-50 text-red-500">
              <AlertCircle className="h-5 w-5" />
            </div>
            <div className="text-[14px] font-semibold tracking-tight">组件渲染异常</div>
            <div className="mt-2 text-[12px] leading-5 text-black/45">
              这个区域发生了运行时错误，已经被隔离，避免整页白屏或侧栏消失。
            </div>
            {this.state.errorMessage ? (
              <div className="mt-3 rounded-xl border border-black/6 bg-black/[0.03] px-3 py-2 text-left font-mono text-[11px] leading-5 text-black/55">
                {this.state.errorMessage}
              </div>
            ) : null}
          </div>
        </div>
      )
    }

    return this.props.children
  }
}
