import { useEffect, useId, useRef, type ReactNode } from 'react';
import { X } from 'lucide-react';

const FOCUSABLE =
  'a[href], area[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

const SIZES = {
  sm: 'max-w-md',
  md: 'max-w-2xl',
  lg: 'max-w-3xl',
  xl: 'max-w-6xl',
} as const;

export interface DialogProps {
  isOpen: boolean;
  onClose: () => void;
  title: string;
  description?: string;
  size?: keyof typeof SIZES;
  /** Centered modal (default) or a full-height panel docked to the right edge. */
  align?: 'center' | 'sheet';
  /** Extra controls rendered beside the close button. */
  actions?: ReactNode;
  footer?: ReactNode;
  children: ReactNode;
  bodyClassName?: string;
}

/**
 * The single modal surface: backdrop, titled header, focus trap, ESC to close,
 * focus restore, and animation that honours prefers-reduced-motion.
 */
export default function Dialog({
  isOpen,
  onClose,
  title,
  description,
  size = 'md',
  align = 'center',
  actions,
  footer,
  children,
  bodyClassName = '',
}: DialogProps) {
  const panelRef = useRef<HTMLDivElement | null>(null);
  const restoreRef = useRef<HTMLElement | null>(null);
  const titleId = useId();

  useEffect(() => {
    if (!isOpen) return;

    restoreRef.current =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';

    const panel = panelRef.current;
    const preferred = panel?.querySelector<HTMLElement>('[data-autofocus]');
    const initial = preferred ?? panel?.querySelector<HTMLElement>(FOCUSABLE) ?? panel;
    initial?.focus();

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        event.stopPropagation();
        onClose();
        return;
      }

      if (event.key !== 'Tab' || !panel) return;

      const focusable = Array.from(panel.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
        (element) => element.offsetParent !== null,
      );

      if (focusable.length === 0) {
        event.preventDefault();
        panel.focus();
        return;
      }

      const first = focusable[0];
      const last = focusable[focusable.length - 1];

      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };

    document.addEventListener('keydown', onKeyDown, true);

    return () => {
      document.removeEventListener('keydown', onKeyDown, true);
      document.body.style.overflow = previousOverflow;
      restoreRef.current?.focus();
    };
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  const sheet = align === 'sheet';

  return (
    <div
      className={`dialog-backdrop fixed inset-0 z-50 flex bg-black/60 backdrop-blur-sm ${
        sheet ? 'justify-end' : 'items-center justify-center p-4'
      }`}
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
        className={`dialog-panel flex flex-col border-aether-border bg-aether-surface shadow-2xl outline-none ${
          sheet
            ? 'h-full w-full max-w-md border-l'
            : `max-h-[90vh] w-full overflow-hidden rounded-xl border ${SIZES[size]}`
        }`}
      >
        <header className="flex items-start justify-between gap-3 border-b border-aether-border px-6 py-4">
          <div className="min-w-0">
            <h2 id={titleId} className="text-section font-semibold text-aether-text">
              {title}
            </h2>
            {description && <p className="mt-1 text-meta text-aether-text-muted">{description}</p>}
          </div>
          <div className="flex shrink-0 items-center gap-2">
            {actions}
            <button
              type="button"
              onClick={onClose}
              aria-label={`Close ${title}`}
              className="rounded p-1 text-aether-text-muted transition hover:bg-aether-bg hover:text-aether-text focus-visible:outline focus-visible:outline-2 focus-visible:outline-aether-primary"
            >
              <X size={16} />
            </button>
          </div>
        </header>

        <div className={`flex-1 overflow-y-auto px-6 py-4 ${bodyClassName}`}>{children}</div>

        {footer && (
          <footer className="flex flex-wrap items-center justify-end gap-2 border-t border-aether-border px-6 py-4">
            {footer}
          </footer>
        )}
      </div>
    </div>
  );
}
