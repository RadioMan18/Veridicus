import Dialog from './Dialog';

const GROUPS: { title: string; items: [string, string][] }[] = [
  {
    title: 'Anywhere',
    items: [
      ['Ctrl K', 'Open the command palette'],
      ['g then a', 'Go to Applications'],
      ['g then d', 'Go to Dashboard'],
      ['g then p', 'Go to Prep'],
      ['g then s', 'Go to Settings'],
      ['?', 'Show this sheet'],
      ['Esc', 'Close the topmost dialog'],
    ],
  },
  {
    title: 'Command palette',
    items: [
      ['↑ / ↓', 'Move through commands'],
      ['Tab', 'Complete the selected command'],
      ['Enter', 'Run the command'],
    ],
  },
  {
    title: 'Applications',
    items: [
      ['j / k', 'Next / previous application'],
      ['Enter', 'Open the selected application'],
      ['Esc', 'Close the detail panel'],
    ],
  },
];

export default function ShortcutSheet({ isOpen, onClose }: { isOpen: boolean; onClose: () => void }) {
  return (
    <Dialog
      isOpen={isOpen}
      onClose={onClose}
      title="Keyboard shortcuts"
      description="Veridicus is usable end to end without a mouse."
      size="md"
    >
      <div className="space-y-5">
        {GROUPS.map((group) => (
          <section key={group.title}>
            <h3 className="text-meta font-semibold uppercase tracking-wider text-aether-text-muted">
              {group.title}
            </h3>
            <dl className="mt-2 divide-y divide-aether-border">
              {group.items.map(([keys, description]) => (
                <div key={keys} className="flex items-center justify-between gap-4 py-2">
                  <dd className="text-body text-aether-text">{description}</dd>
                  <dt className="shrink-0 rounded border border-aether-border bg-aether-bg px-2 py-0.5 font-mono text-meta text-aether-text-muted">
                    {keys}
                  </dt>
                </div>
              ))}
            </dl>
          </section>
        ))}
      </div>
    </Dialog>
  );
}
