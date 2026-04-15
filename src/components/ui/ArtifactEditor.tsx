import * as React from 'react'
import { EditorState } from '@codemirror/state'
import {
  drawSelection,
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
} from '@codemirror/view'
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands'
import { defaultHighlightStyle, syntaxHighlighting } from '@codemirror/language'
import { markdown } from '@codemirror/lang-markdown'
import { javascript } from '@codemirror/lang-javascript'
import { json } from '@codemirror/lang-json'
import { html } from '@codemirror/lang-html'
import { css } from '@codemirror/lang-css'

export interface ArtifactEditorProps {
  content: string
  language?: string | null
  onChange: (value: string) => void
}

export function ArtifactEditor({ content, language, onChange }: ArtifactEditorProps) {
  const containerRef = React.useRef<HTMLDivElement | null>(null)
  const viewRef = React.useRef<EditorView | null>(null)
  const onChangeRef = React.useRef(onChange)

  onChangeRef.current = onChange

  React.useEffect(() => {
    if (!containerRef.current) return

    const state = EditorState.create({
      doc: content,
      extensions: [
        drawSelection(),
        history(),
        lineNumbers(),
        highlightActiveLine(),
        highlightActiveLineGutter(),
        syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
        keymap.of([...defaultKeymap, ...historyKeymap]),
        EditorView.lineWrapping,
        resolveLanguage(language),
        EditorView.theme({
          '&': {
            height: '100%',
            backgroundColor: '#FAF9F6',
            color: 'rgba(28, 25, 23, 0.82)',
            fontSize: '13px',
          },
          '.cm-scroller': {
            overflow: 'auto',
            padding: '18px 0',
            fontFamily: '"SF Mono", "Geist Mono", "IBM Plex Mono", ui-monospace, monospace',
          },
          '.cm-content': {
            padding: '0 18px 48px',
            minHeight: '100%',
          },
          '.cm-gutters': {
            backgroundColor: 'transparent',
            borderRight: '1px solid rgba(226, 220, 212, 0.6)',
            color: 'rgba(140, 130, 118, 0.7)',
          },
          '.cm-activeLine, .cm-activeLineGutter': {
            backgroundColor: 'rgba(235, 231, 225, 0.8)',
          },
          '.cm-selectionBackground': {
            backgroundColor: 'rgba(134, 185, 171, 0.18) !important',
          },
          '.cm-cursor': {
            borderLeftColor: 'rgba(134, 185, 171, 0.9)',
          },
          '&.cm-focused': {
            outline: 'none',
          },
        }),
        EditorView.updateListener.of((update) => {
          if (!update.docChanged) return
          onChangeRef.current(update.state.doc.toString())
        }),
      ],
    })

    const view = new EditorView({
      state,
      parent: containerRef.current,
    })

    viewRef.current = view
    return () => {
      view.destroy()
      viewRef.current = null
    }
  }, [language])

  React.useEffect(() => {
    const view = viewRef.current
    if (!view) return
    const current = view.state.doc.toString()
    if (current === content) return
    view.dispatch({
      changes: { from: 0, to: current.length, insert: content },
    })
  }, [content])

  return <div className="h-full overflow-hidden" ref={containerRef} />
}

function resolveLanguage(language?: string | null) {
  switch ((language ?? '').toLowerCase()) {
    case 'markdown':
      return markdown()
    case 'typescript':
    case 'tsx':
      return javascript({ typescript: true, jsx: true })
    case 'javascript':
    case 'jsx':
      return javascript({ jsx: true })
    case 'json':
      return json()
    case 'html':
      return html()
    case 'css':
      return css()
    default:
      return []
  }
}
