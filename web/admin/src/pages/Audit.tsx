import { useInfiniteQuery } from '@tanstack/react-query';
import { Ban, FileStack, Gem, KeyRound, PackagePlus, Trash2, TrendingUp, Undo2, Upload, Link2, LogIn, LogOut, RefreshCw, ShieldCheck, UserPlus, UserRound } from 'lucide-react';
import type { ReactNode } from 'react';
import { api, type AuditEntry, fail } from '@/api/client';
import { errorText, roleLabel } from '@/components/forms';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';

const PAGE = 50;

const ACTIONS: Record<string, { label: string; icon: ReactNode }> = {
  'session.login': { label: 'Entrada no painel', icon: <LogIn /> },
  'session.logout': { label: 'Saída do painel', icon: <LogOut /> },
  'account.register': { label: 'Cadastro pela tela de login', icon: <UserPlus /> },
  'account.create': { label: 'Conta criada', icon: <UserPlus /> },
  'account.role': { label: 'Papel da conta', icon: <ShieldCheck /> },
  'account.password': { label: 'Senha trocada', icon: <KeyRound /> },
  'account.status': { label: 'Conta ativada/desativada', icon: <Ban /> },
  'account.game_link': { label: 'Conta ligada a usuário do jogo', icon: <Link2 /> },
  'play.code': { label: 'Código de jogo (recurso retirado)', icon: <KeyRound /> },
  'character.item_give': { label: 'Item dado ao personagem', icon: <PackagePlus /> },
  'character.item_remove': { label: 'Item removido do personagem', icon: <Trash2 /> },
  'character.cash': { label: 'Cash da conta ajustado', icon: <Gem /> },
  'character.progression': { label: 'Progressão do personagem', icon: <TrendingUp /> },
  'content.edit': { label: 'Edição de conteúdo', icon: <FileStack /> },
  'content.withdraw': { label: 'Edição de conteúdo retirada', icon: <Undo2 /> },
  'content.publish': { label: 'Versão de conteúdo publicada', icon: <Upload /> },
};

async function page(beforeId: number | undefined): Promise<AuditEntry[]> {
  const { data, error, response } = await api.GET('/api/audit', {
    params: { query: { limit: PAGE, before_id: beforeId } },
  });
  if (!data) fail(response, error);
  return data;
}

/** JSON of the audit row as readable "key: value" text. */
function describe(json?: string | null) {
  if (!json) return '—';
  try {
    const value = JSON.parse(json) as Record<string, unknown>;
    return Object.entries(value)
      .map(([k, v]) => `${k}: ${v ?? '—'}`)
      .join(', ');
  } catch {
    return json;
  }
}

function Entry({ row }: { row: AuditEntry }) {
  const action = ACTIONS[row.action] ?? { label: row.action, icon: <UserRound /> };
  const refused = row.outcome !== 'ok';
  return (
    <li className="flex gap-3 rounded-xl border border-border bg-surface p-4 shadow-sm">
      <span
        className={`flex size-9 shrink-0 items-center justify-center rounded-lg [&_svg]:size-4 ${refused ? 'bg-danger-soft text-danger' : 'bg-accent-soft text-fg'}`}
      >
        {action.icon}
      </span>
      <div className="min-w-0 flex-1 space-y-1">
        <div className="flex flex-wrap items-center gap-2">
          <span className="font-medium">{action.label}</span>
          {refused ? <Badge tone="danger">recusado</Badge> : <Badge tone="ok">ok</Badge>}
          {row.target_id && <Badge>{`${row.target_kind}: ${row.target_id}`}</Badge>}
          <span className="ml-auto text-xs text-muted">{new Date(row.at).toLocaleString('pt-BR')}</span>
        </div>
        <p className="text-sm text-muted">
          {row.actor_name ? `${row.actor_name} (${roleLabel(row.role)})` : 'CLI do operador / login desconhecido'}
          {row.peer && ` · ${row.peer}`}
        </p>
        {row.reason && <p className="text-sm">Motivo: {row.reason}</p>}
        {(row.before || row.after) && (
          <p className="rounded-md bg-raised px-2 py-1 font-mono text-xs break-all">
            {describe(row.before)} → {describe(row.after)}
          </p>
        )}
      </div>
    </li>
  );
}

export function AuditPage() {
  const query = useInfiniteQuery({
    queryKey: ['audit'],
    queryFn: ({ pageParam }) => page(pageParam),
    initialPageParam: undefined as number | undefined,
    getNextPageParam: (last) => (last.length === PAGE ? last[last.length - 1]?.id : undefined),
  });
  const rows = query.data?.pages.flat() ?? [];
  return (
    <section className="space-y-4">
      <div className="flex justify-end">
        <Button variant="outline" size="sm" onClick={() => void query.refetch()}>
          <RefreshCw /> Atualizar
        </Button>
      </div>
      <ul className="space-y-2">
        {rows.map((row) => (
          <Entry key={row.id} row={row} />
        ))}
      </ul>
      {rows.length === 0 && !query.isPending && <p className="text-sm text-muted">Nenhum evento.</p>}
      {query.error && (
        <p role="alert" className="text-sm text-danger">
          {errorText(query.error)}
        </p>
      )}
      {query.hasNextPage && (
        <Button variant="outline" disabled={query.isFetchingNextPage} onClick={() => void query.fetchNextPage()}>
          Carregar mais
        </Button>
      )}
    </section>
  );
}
