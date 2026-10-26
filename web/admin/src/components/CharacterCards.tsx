import { Link } from '@tanstack/react-router';
import { type CharacterSummary, className } from '@/api/characters';
import { Avatar } from '@/components/Avatar';
import { Badge } from '@/components/ui/badge';

/** Character cards; each opens the character sheet. */
export function CharacterCards({ characters, empty }: { characters: CharacterSummary[]; empty: string }) {
  if (characters.length === 0) return <p className="text-sm text-muted">{empty}</p>;
  return (
    <div className="grid gap-2 sm:grid-cols-2">
      {characters.map((c) => (
        <Link
          key={c.cid}
          to="/personagens/$cid"
          params={{ cid: c.cid }}
          className="flex items-center gap-3 rounded-xl border border-border bg-surface p-3 transition-colors hover:border-accent/70"
        >
          <Avatar name={c.name || '?'} />
          <div className="min-w-0 flex-1">
            <p className="truncate font-semibold">{c.name || '(sem nome)'}</p>
            <p className="text-xs text-muted">
              {className(c.class)} · nível {c.level}
            </p>
          </div>
          {c.connected && <Badge tone="ok">online</Badge>}
          {c.deleting && <Badge tone="danger">excluindo</Badge>}
        </Link>
      ))}
    </div>
  );
}
