import React, { Component, ErrorInfo, ReactNode } from 'react'
import ReactDOM from 'react-dom/client'
import AppShell from './components/AppShell'
import { copyText } from './components/Toaster'
import './index.css'

interface Props {
  children: ReactNode
}

interface State {
  hasError: boolean
  error: Error | null
}

class ErrorBoundary extends Component<Props, State> {
  public state: State = {
    hasError: false,
    error: null,
  }

  public static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error }
  }

  public componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('Uncaught error:', error, errorInfo)
  }

  public render() {
    if (this.state.hasError) {
      const message = this.state.error?.message || String(this.state.error)
      return (
        <div className="flex min-h-screen flex-col items-center justify-center bg-aether-bg p-8 text-aether-text">
          <div className="w-full max-w-lg space-y-4 rounded-xl border border-aether-error/50 bg-aether-surface p-6 shadow-xl">
            <h1 className="text-section font-semibold text-aether-error">Application Error</h1>
            <p className="text-body text-aether-text-muted">
              CareerOS encountered an unexpected error while rendering:
            </p>
            <pre className="max-h-48 overflow-auto rounded border border-aether-border bg-aether-bg p-3 font-mono text-meta text-aether-error">
              {message}
            </pre>
            <div className="flex flex-wrap gap-2">
              <button
                onClick={() => window.location.reload()}
                className="rounded bg-aether-primary px-4 py-2 text-body font-medium text-aether-primary-fg transition hover:bg-aether-primary-hover"
              >
                Reload Application
              </button>
              <button
                onClick={() => void copyText(message)}
                className="rounded border border-aether-border px-4 py-2 text-body text-aether-text transition hover:bg-aether-bg"
              >
                Copy error
              </button>
            </div>
          </div>
        </div>
      )
    }

    return this.props.children
  }
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <ErrorBoundary>
      <AppShell />
    </ErrorBoundary>
  </React.StrictMode>
)
