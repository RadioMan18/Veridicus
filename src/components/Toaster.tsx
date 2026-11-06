import { useEffect, useState } from 'react';
import { CheckCircle2, Info, TriangleAlert, X } from 'lucide-react';

export type ToastKind = 'success' | 'error' | 'info';

export interface ToastAction {
  label: string;
  onClick: () => void;
}

export interface ToastItem {
  id: number;
  kind: ToastKind;
  message: string;
  /** Raw technical text (Rust error, path) offered behind "Copy details". */
  detail?: string;
  action?: ToastAction;
  durationMs?: number;
}

let items: ToastItem[] = [];
let listeners: Array<(next: ToastItem[]) => void> = [];
let nextId = 1;

function publish() {
  for (const listener of listeners) listener([...items]);
}

export function dismissToast(id: number) {
  items = items.filter((item) => item.id !== id);
  publish();
}

export function pushToast(toast: Omit<ToastItem, 'id'>) {
  const id = nextId++;
  items = [...items.slice(-3), { ...toast, id }];
  publish();
  const timeout = toast.durationMs ?? (toast.kind === 'error' ? 15000 : 6000);
  window.setTimeout(() => dismissToast(id), timeout);
  return id;
}

/** Clipboard write with a fallback for webviews that block the async API. */
export async function copyText(value: string) {
  try {
    await navigator.clipboard.writeText(value);
    return true;
  } catch {
    try {
      const area = document.createElement('textarea');
      area.value = value;
      area.setAttribute('readonly', '');
      area.style.position = 'fixed';
      area.style.opacity = '0';
      document.body.appendChild(area);
      area.select();
      const copied = document.execCommand('copy');
      document.body.removeChild(area);
      return copied;
    } catch {
      return false;
    }
  }
}

const ICONS = { success: CheckCircle2, error: TriangleAlert, info: Info };
const TONES: Record<ToastKind, string> = {
  success: 'text-aether-success',
  error: 'text-aether-error',
  info: 'text-aether-text-muted',
};

export default function Toaster() {
  const [toasts, setToasts] = useState<ToastItem[]>(items);

  useEffect(() => {
    listeners.push(setToasts);
    setToasts([...items]);
    return () => {
      listeners = listeners.filter((listener) => listener !== setToasts);
    };
  }, []);

  if (toasts.length === 0) return null;

  return (
    <div
      className="pointer-events-none fixed bottom-4 right-4 z-[60] flex w-full max-w-sm flex-col gap-2"
      role="status"
      aria-live="polite"
    >
      {toasts.map((toast) => {
        const Icon = ICONS[toast.kind];
        return (
          <div
            key={toast.id}
            className="pointer-events-auto flex items-start gap-3 rounded-lg border border-aether-border bg-aether-surface p-3 shadow-lg"
          >
            <Icon size={16} className={`mt-0.5 shrink-0 ${TONES[toast.kind]}`} />
            <div className="min-w-0 flex-1">
              <p className="text-body text-aether-text">{toast.message}</p>
              {toast.detail && (
                <button
                  onClick={() => void copyText(toast.detail ?? '')}
                  className="mt-1 text-meta text-aether-text-muted underline transition hover:text-aether-text"
                >
                  Copy details
                </button>
              )}
              {toast.action && (
                <button
                  onClick={() => {
                    toast.action?.onClick();
                    dismissToast(toast.id);
                  }}
                  className="mt-1 block text-meta font-medium text-aether-primary-ink underline"
                >
                  {toast.action.label}
                </button>
              )}
            </div>
            <button
              onClick={() => dismissToast(toast.id)}
              aria-label="Dismiss notification"
              className="shrink-0 text-aether-text-muted transition hover:text-aether-text"
            >
              <X size={14} />
            </button>
          </div>
        );
      })}
    </div>
  );
}
