import { Component, ErrorInfo, ReactNode } from 'react';
import { getLogger } from '@/lib/logger';

const logger = getLogger('ErrorBoundary');

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

class ErrorBoundary extends Component<Props, State> {
  public state: State = {
    hasError: false,
    error: null,
  };

  public static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  public componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    logger.error('React ErrorBoundary caught error:', {
      error: error.message,
      stack: error.stack,
      componentStack: errorInfo.componentStack,
    });
  }

  public render() {
    if (this.state.hasError) {
      return (
        <div className="flex items-center justify-center h-screen w-screen bg-background">
          <div className="text-center p-8">
            <h1 className="text-2xl font-bold mb-4 text-destructive">
              Something went wrong
            </h1>
            <details className="text-left">
              <summary className="cursor-pointer mb-2">Error Details</summary>
              <pre className="mt-2 p-4 bg-muted rounded overflow-auto max-w-2xl">
                {this.state.error?.message}
                {'\n\n'}
                {this.state.error?.stack}
              </pre>
            </details>
            <button
              onClick={() => window.location.reload()}
              className="mt-4 px-4 py-2 bg-primary text-primary-foreground rounded hover:bg-primary/90"
            >
              Reload Application
            </button>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}

export default ErrorBoundary;
