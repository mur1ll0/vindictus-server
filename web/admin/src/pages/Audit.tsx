import { useInfiniteQuery } from '@tanstack/react-query';
import { api, type AuditEntry, fail } from '@/api/client';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';

const PAGE = 50;

const ACTIONS: Record<string, string> = {
  'session.login': 'Entrada no painel',
  'session.logout': 'Saída do painel',
  'account.role': 'Papel da conta',
  'account.create': 'Conta criada',
  'account.password': 'Senha trocada',
  'account.status': 'Conta ativada/desativada',
};

async function page(beforeId: number | undefined): Promise<AuditEntry[]> {
  const { data, error, response } = await api.GET('/api/audit', {
    params: { query: { limit: PAGE, before_id: beforeId } },
  });
  if (!data) fail(response, error);
  return data;
}

function when(at: string) {
  return new Date(at).toLocaleString('pt-BR');
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
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold">Auditoria</h2>
        <Button variant="outline" size="sm" onClick={() => void query.refetch()}>
          Atualizar
        </Button>
      </div>
      <Card className="overflow-x-auto p-0">
        <table className="w-full text-left text-sm">
          <thead className="border-b border-border text-muted">
            <tr>
              <th className="px-4 py-2 font-medium">Quando</th>
              <th className="px-4 py-2 font-medium">Quem</th>
              <th className="px-4 py-2 font-medium">Ação</th>
              <th className="px-4 py-2 font-medium">Alvo</th>
              <th className="px-4 py-2 font-medium">Motivo</th>
              <th className="px-4 py-2 font-medium">Antes → depois</th>
              <th className="px-4 py-2 font-medium">Resultado</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.id} className="border-b border-border align-top last:border-0">
                <td className="whitespace-nowrap px-4 py-2">{when(row.at)}</td>
                <td className="px-4 py-2">
                  {row.actor_name ?? <span className="text-muted">CLI / desconhecido</span>}
                  {row.role && <span className="ml-1 text-xs text-muted">({row.role})</span>}
                </td>
                <td className="px-4 py-2">{ACTIONS[row.action] ?? row.action}</td>
                <td className="px-4 py-2">
                  {row.target_id ? `${row.target_kind}: ${row.target_id}` : '—'}
                </td>
                <td className="px-4 py-2">{row.reason ?? '—'}</td>
                <td className="px-4 py-2 font-mono text-xs">
                  {row.before || row.after ? `${row.before ?? '—'} → ${row.after ?? '—'}` : '—'}
                </td>
                <td className={row.outcome === 'ok' ? 'px-4 py-2' : 'px-4 py-2 text-danger'}>
                  {row.outcome === 'ok' ? 'ok' : 'recusado'}
                </td>
              </tr>
            ))}
            {rows.length === 0 && !query.isPending && (
              <tr>
                <td colSpan={7} className="px-4 py-6 text-center text-muted">
                  Nenhum evento.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </Card>
      {query.error && (
        <p role="alert" className="text-sm text-danger">
          {query.error.message}
        </p>
      )}
      {query.hasNextPage && (
        <Button
          variant="outline"
          disabled={query.isFetchingNextPage}
          onClick={() => void query.fetchNextPage()}
        >
          Carregar mais
        </Button>
      )}
    </section>
  );
}
