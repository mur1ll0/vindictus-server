import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { FileStack, Map as MapIcon, Network, Plug, Radio, Swords, Users } from 'lucide-react';
import type { ReactNode } from 'react';
import { api, type components, fail } from '@/api/client';
import { errorText } from '@/components/forms';
import { Badge } from '@/components/ui/badge';
import { Card } from '@/components/ui/card';

type World = components['schemas']['WorldView'];

async function world(): Promise<World> {
  const { data, error, response } = await api.GET('/api/world');
  if (!data) fail(response, error);
  return data;
}

const SERVICE: Record<string, string> = {
  frontend: 'Login e lista de personagens',
  channel: 'Canal (cidade)',
  relay: 'Relay das missões',
  web_shop: 'Loja web do jogo',
};

function Tile({ icon, label, value, hint }: { icon: ReactNode; label: string; value: ReactNode; hint?: string }) {
  return (
    <Card className="flex items-center gap-4">
      <span className="flex size-10 shrink-0 items-center justify-center rounded-lg bg-accent-soft text-accent [&>svg]:size-5">
        {icon}
      </span>
      <div className="min-w-0">
        <p className="text-sm text-muted">{label}</p>
        <p className="text-xl font-semibold">{value}</p>
        {hint && <p className="text-xs text-muted">{hint}</p>}
      </div>
    </Card>
  );
}

/** Maps and channels (B4.6): read-only, refreshed every 5 seconds. */
export function WorldPage() {
  const q = useQuery({ queryKey: ['world'], queryFn: world, refetchInterval: 5000 });
  if (q.isPending) return <p className="text-muted">Carregando…</p>;
  if (q.isError) return <p className="text-danger">{errorText(q.error)}</p>;
  const w = q.data;
  const { loaded, published } = w.content;
  const behind = loaded != null && published != null && published > loaded;
  return (
    <section className="space-y-5">
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <Tile icon={<Plug />} label="Conexões no jogo" value={w.connections} hint="login, lista e cidade" />
        <Tile
          icon={<Users />}
          label="No canal 1"
          value={w.channel ? w.channel.members.length : '—'}
          hint={w.channel && w.channel.entering > 0 ? `${w.channel.entering} entrando` : undefined}
        />
        <Tile
          icon={<Swords />}
          label="Missões em andamento"
          value={w.relay ? w.relay.groups.length : '—'}
          hint={w.relay ? `${w.relay.connected} no relay` : 'relay não configurado'}
        />
        <Tile
          icon={<FileStack />}
          label="Versão de conteúdo carregada"
          value={loaded ?? '—'}
          hint={published != null ? `última publicada: ${published}` : 'nenhuma publicada'}
        />
      </div>

      {behind && (
        <Card className="border-accent/60 text-sm">
          A versão {published} está publicada, mas o jogo carregou a {loaded}. Ela vale no próximo reinício (veja{' '}
          <Link to="/conteudo" className="text-accent underline">
            Conteúdo
          </Link>
          ).
        </Card>
      )}

      <Card>
        <div className="mb-3 flex items-center gap-2">
          <Radio className="size-4 text-accent" />
          <h2 className="font-semibold">Canal 1</h2>
          <span className="text-sm text-muted">o laboratório tem um canal só</span>
        </div>
        {!w.channel ? (
          <p className="text-sm text-muted">O conteúdo carregado não configura o canal.</p>
        ) : w.channel.members.length === 0 ? (
          <p className="text-sm text-muted">Ninguém no canal agora.</p>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead className="text-left text-muted">
                <tr>
                  <th className="py-1 pr-4 font-medium">Personagem</th>
                  <th className="py-1 pr-4 font-medium">Conta</th>
                  <th className="py-1 pr-4 font-medium">Mapa</th>
                  <th className="py-1 font-medium">Partição</th>
                </tr>
              </thead>
              <tbody>
                {w.channel.members.map((m) => (
                  <tr key={m.cid} className="border-t border-border">
                    <td className="py-1.5 pr-4">
                      <Link to="/personagens/$cid" params={{ cid: String(m.cid) }} className="text-accent underline">
                        {m.name || `#${m.cid}`}
                      </Link>
                    </td>
                    <td className="py-1.5 pr-4">{m.account ?? '—'}</td>
                    <td className="py-1.5 pr-4 font-mono">{m.map ?? '—'}</td>
                    <td className="py-1.5 font-mono">{m.partition}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Card>

      <div className="grid gap-5 lg:grid-cols-2">
        <Card>
          <div className="mb-3 flex items-center gap-2">
            <Network className="size-4 text-accent" />
            <h2 className="font-semibold">Portas</h2>
          </div>
          {w.ports.length === 0 ? (
            <p className="text-sm text-muted">O painel não está ligado ao servidor do jogo.</p>
          ) : (
            <ul className="space-y-1.5 text-sm">
              {w.ports.map((p) => (
                <li key={p.service} className="flex justify-between gap-4">
                  <span>{SERVICE[p.service] ?? p.service}</span>
                  <span className="font-mono">{p.address}</span>
                </li>
              ))}
            </ul>
          )}
          {w.relay && (
            <p className="mt-3 text-xs text-muted">
              Relay: {w.relay.connected} conectado(s), {w.relay.waiting} aguardando conexão
              {w.relay.groups.length > 0 && ` · grupos ${w.relay.groups.map((g) => `${g.id} (${g.members})`).join(', ')}`}
            </p>
          )}
        </Card>
        <Card>
          <div className="mb-3 flex items-center gap-2">
            <MapIcon className="size-4 text-accent" />
            <h2 className="font-semibold">Mapas do canal</h2>
          </div>
          {!w.channel || w.channel.maps.length === 0 ? (
            <p className="text-sm text-muted">Nenhum mapa carregado.</p>
          ) : (
            <ul className="flex flex-wrap gap-2">
              {w.channel.maps.map((m) => (
                <li key={m.name}>
                  <Badge tone={m.built ? 'neutral' : 'danger'}>
                    {m.name}
                    {!m.built && ' (falhou)'}
                  </Badge>
                </li>
              ))}
            </ul>
          )}
        </Card>
      </div>
    </section>
  );
}
