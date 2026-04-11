import { useEffect, useState, useRef, useCallback } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeHighlight from 'rehype-highlight'
import rehypeRaw from 'rehype-raw'
import { cn } from '@/lib/utils'

interface StreamingMarkdownProps {
  content: string
  isStreaming?: boolean
  className?: string
}

/**
 * Streaming-aware markdown renderer
 * - During streaming: progressively renders markdown with cursor animation
 * - After streaming: renders full markdown using react-markdown
 */
export function StreamingMarkdown({ content, isStreaming = false, className }: StreamingMarkdownProps) {
  const [renderedContent, setRenderedContent] = useState('')
  const [isComplete, setIsComplete] = useState(!isStreaming)
  const cursorRef = useRef<HTMLSpanElement>(null)
  const renderTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Debounced render for progressive updates
  const debouncedRender = useCallback((text: string) => {
    if (renderTimeoutRef.current) {
      clearTimeout(renderTimeoutRef.current)
    }
    renderTimeoutRef.current = setTimeout(() => {
      setRenderedContent(text)
    }, 50)
  }, [])

  // Update content progressively during streaming
  useEffect(() => {
    if (isStreaming) {
      setIsComplete(false)
      debouncedRender(content)
    } else {
      setIsComplete(true)
      setRenderedContent(content)
    }
  }, [content, isStreaming, debouncedRender])

  // Cursor blink animation during streaming
  useEffect(() => {
    if (!isStreaming || !cursorRef.current) return

    const cursor = cursorRef.current
    const interval = setInterval(() => {
      cursor.style.opacity = cursor.style.opacity === '0' ? '1' : '0'
    }, 530)

    return () => clearInterval(interval)
  }, [isStreaming])

  // Cleanup timeout on unmount
  useEffect(() => {
    return () => {
      if (renderTimeoutRef.current) {
        clearTimeout(renderTimeoutRef.current)
      }
    }
  }, [])

  // Streaming state: show raw text with cursor
  if (!isComplete) {
    return (
      <div className={cn('md-content', className)}>
        {renderedContent}
        <span ref={cursorRef} className="streaming-cursor" />
      </div>
    )
  }

  // Complete state: use react-markdown for proper rendering
  return (
    <div className={cn('md-content', className)}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[rehypeHighlight, rehypeRaw]}
      >
        {renderedContent}
      </ReactMarkdown>
    </div>
  )
}
