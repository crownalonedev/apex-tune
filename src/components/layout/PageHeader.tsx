interface PageHeaderProps {
  title: string;
  summary: string;
  actions?: React.ReactNode;
}

export function PageHeader({ title, summary, actions }: PageHeaderProps) {
  return (
    <header className="flex flex-wrap items-end justify-between gap-4 border-b border-line pb-5">
      <div className="max-w-[68ch]">
        <h1 className="text-[30px] font-semibold">{title}</h1>
        <p className="mt-1 text-muted">{summary}</p>
      </div>
      {actions && <div className="flex items-center gap-2">{actions}</div>}
    </header>
  );
}
