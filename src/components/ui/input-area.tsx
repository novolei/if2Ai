import { useState, useRef, useEffect, FormEvent, KeyboardEvent } from 'react'
import { cn } from '@/lib/utils'
import inputStyles from '@/styles/input-area.module.css'

interface InputAreaProps {
  value: string
  onChange: (value: string) => void
  onSubmit: () => void
  onStop?: () => void
  disabled?: boolean
  isStreaming?: boolean
  placeholder?: string
  statusText?: string
  statusType?: 'busy' | 'error' | null
}

export function InputArea({
  value,
  onChange,
  onSubmit,
  onStop,
  disabled = false,
  isStreaming = false,
  placeholder = '输入消息…',
  statusText,
  statusType = null,
}: InputAreaProps) {
  const [isFocused, setIsFocused] = useState(false)
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      if (isStreaming && value.trim()) {
        // Steer mode - handled by parent
      } else if (value.trim() && !disabled) {
        onSubmit()
      }
    }
  }

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault()
    if (value.trim() && !disabled) {
      onSubmit()
    }
  }

  // Auto-resize textarea
  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto'
      textareaRef.current.style.height = `${Math.min(textareaRef.current.scrollHeight, 120)}px`
    }
  }, [value])

  // Determine button mode: send / steer / stop
  const buttonMode = isStreaming ? (value.trim() ? 'steer' : 'stop') : 'send'

  return (
    <div className={inputStyles.inputAreaWrapper}>
      {/* Status bar (for busy/error states) */}
      {statusText && (
        <div className={inputStyles.statusBar}>
          <span className={cn(inputStyles.statusDot, inputStyles[statusType || 'busy'])} />
          <span>{statusText}</span>
        </div>
      )}

      <form onSubmit={handleSubmit}>
        <div className={cn(inputStyles.inputContainer, isFocused && 'focused')}>
          <textarea
            ref={textareaRef}
            className={inputStyles.inputBox}
            value={value}
            onChange={(e) => onChange(e.target.value)}
            onKeyDown={handleKeyDown}
            onFocus={() => setIsFocused(true)}
            onBlur={() => setIsFocused(false)}
            placeholder={placeholder}
            disabled={disabled}
            rows={1}
          />
          <div className={inputStyles.inputBottomBar}>
            <div className={inputStyles.inputActions}>
              {/* Left side buttons - placeholder for future features like attachments, etc */}
            </div>
            <div className={inputStyles.inputControls}>
              <button
                type="submit"
                className={cn(
                  inputStyles.sendBtn,
                  buttonMode === 'steer' && inputStyles.steer,
                  buttonMode === 'stop' && inputStyles.stop
                )}
                disabled={buttonMode === 'send' ? (disabled || !value.trim()) : false}
                onClick={() => {
                  if (buttonMode === 'stop' && onStop) {
                    onStop()
                  }
                }}
              >
                <span className={inputStyles.sendLabel}>
                  {buttonMode === 'send' && (
                    <>
                      <svg className={inputStyles.sendEnterIcon} width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                        <polyline points="9 10 4 15 9 20" /><path d="M20 4v7a4 4 0 01-4 4H4" />
                      </svg>
                      <span>发送</span>
                    </>
                  )}
                  {buttonMode === 'steer' && (
                    <>
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                        <polyline points="15 18 9 12 15 6" />
                      </svg>
                      <span>插话</span>
                    </>
                  )}
                  {buttonMode === 'stop' && (
                    <>
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                        <rect x="6" y="6" width="12" height="12" rx="2" />
                      </svg>
                      <span>停止</span>
                    </>
                  )}
                </span>
              </button>
            </div>
          </div>
        </div>
      </form>
    </div>
  )
}
