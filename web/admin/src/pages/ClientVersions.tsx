import { useQuery, useQueryClient } from '@tanstack/react-query';
import { EyeOff, Eye, FileUp, FileX, HardDriveDownload, Megaphone, Rocket, Undo2 } from 'lucide-react';
import { useState } from 'react';
import { api, type components, fail } from '@/api/client';
import { sessionQuery } from '@/api/session';
import { ActionForm, errorText } from '@/components/forms';
import { Modal } from '@/components/Modal';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

type View = components['schemas']['ClientVersionsView'];
type News = components['schemas']['NewsRow'];
type Draft = components['schemas']['DraftView'];
type Item = components['schemas']['ItemRow'];
type Published = components['schemas']['PublishedFile'];

async function load(): Promise<View> {
  const { data, error, response } = await api.GET('/api/client-versions');
  if (!data) fail(response, error);
  return data;
}

async function loadDraft(): Promise<Draft> {
  const { data, error, response } = await api.GET('/api/client-versions/draft');
  if (!data) fail(response, error);
  return data;
}

function UploadModal({ onClose, onDone }: { onClose: () => void; onDone: () => void }) {
  const [path, setPath] = useState('');
  const [file, setFile] = useState<File | null>(null);
  return (
    <Modal
      open
      title="Adicionar ou trocar arquivo"
      subtitle="O arquivo entra no rascunho da próxima versão no caminho indicado (relativo à pasta do jogo). Se o caminho já existe, ele é trocado."
      icon={<FileUp className="size-5 text-muted" />}
      onClose={onClose}
    >
      <ActionForm
        submitLabel="Enviar"
        onCancel={onClose}
        onDone={onDone}
        onSubmit={async (operation_id) => {
          if (!file) throw new Error('Escolha o arquivo.');
          const { data, error, response } = await api.PUT('/api/client-versions/draft/file', {
            params: { query: { operation_id, path } },
            body: file as unknown as number[],
            bodySerializer: (body) => body as unknown as BodyInit,
            headers: { 'Content-Type': 'application/octet-stream' },
          });
          if (!data) fail(response, error);
        }}
      >
        <div className="space-y-1.5">
          <Label htmlFor="draft-file">Arquivo (até 256 MB)</Label>
          <Input
            id="draft-file"
            type="file"
            required
            onChange={(e) => {
              const chosen = e.target.files?.[0] ?? null;
              setFile(chosen);
              if (chosen && !path) setPath(chosen.name);
            }}
          />
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="draft-path">Caminho no cliente (ex.: nmcogame.dll, bin/client.dll, hfs/&lt;pacote&gt;.hfs)</Label>
          <Input id="draft-path" required maxLength={260} value={path} onChange={(e) => setPath(e.target.value)} />
        </div>
      </ActionForm>
    </Modal>
  );
}

function RemoveModal({ onClose, onDone }: { onClose: () => void; onDone: () => void }) {
  const [search, setSearch] = useState('');
  const [path, setPath] = useState('');
  const found = useQuery({
    queryKey: ['client-files', search],
    enabled: search.trim().length >= 2,
    queryFn: async (): Promise<Published[]> => {
      const { data, error, response } = await api.GET('/api/client-versions/files', { params: { query: { search } } });
      if (!data) fail(response, error);
      return data;
    },
  });
  return (
    <Modal
      open
      title="Remover arquivo"
      subtitle="O arquivo sai da próxima versão: os launchers o apagam ao atualizar."
      icon={<FileX className="size-5 text-muted" />}
      onClose={onClose}
    >
      <ActionForm
        danger
        submitLabel="Marcar remoção"
        onCancel={onClose}
        onDone={onDone}
        onSubmit={async (operation_id) => {
          const { data, error, response } = await api.POST('/api/client-versions/draft/remove', { body: { operation_id, path } });
          if (!data) fail(response, error);
        }}
      >
        <div className="space-y-1.5">
          <Label htmlFor="remove-search">Buscar na versão publicada</Label>
          <Input id="remove-search" value={search} onChange={(e) => setSearch(e.target.value)} placeholder="parte do caminho" />
        </div>
        {(found.data ?? []).length > 0 && (
          <ul className="max-h-48 overflow-y-auto rounded-lg border border-border text-sm">
            {(found.data ?? []).map((f) => (
              <li key={f.path}>
                <button type="button" className={`w-full px-3 py-1 text-left hover:bg-surface-2 ${f.path === path ? 'font-semibold' : ''}`} onClick={() => setPath(f.path)}>
                  {f.path} <span className="text-xs text-muted">{size(f.size)}</span>
                </button>
              </li>
            ))}
          </ul>
        )}
        <div className="space-y-1.5">
          <Label htmlFor="remove-path">Caminho</Label>
          <Input id="remove-path" required maxLength={260} value={path} onChange={(e) => setPath(e.target.value)} />
        </div>
      </ActionForm>
    </Modal>
  );
}

function PublishModal({ next, changes, onClose, onDone }: { next: number; changes: number; onClose: () => void; onDone: () => void }) {
  const [title, setTitle] = useState('');
  const [notes, setNotes] = useState('');
  return (
    <Modal
      open
      title={`Publicar a versão ${next}`}
      subtitle={`A versão atual mais as ${changes} mudança(s) do rascunho. Os launchers trocam Jogar por Atualizar até baixarem a versão nova.`}
      icon={<Rocket className="size-5 text-muted" />}
      onClose={onClose}
    >
      <ActionForm
        submitLabel="Publicar"
        onCancel={onClose}
        onDone={onDone}
        onSubmit={async (operation_id) => {
          const { data, error, response } = await api.POST('/api/client-versions/draft/publish', { body: { operation_id, title, notes } });
          if (!data) fail(response, error);
        }}
      >
        <div className="space-y-1.5">
          <Label htmlFor="publish-title">Título (aparece no launcher, até 200 caracteres)</Label>
          <Input id="publish-title" required maxLength={200} value={title} onChange={(e) => setTitle(e.target.value)} />
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="publish-notes">Notas (opcional)</Label>
          <textarea
            id="publish-notes"
            maxLength={4000}
            rows={4}
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
            className="w-full rounded-lg border border-border bg-surface px-3 py-2 text-sm"
          />
        </div>
      </ActionForm>
    </Modal>
  );
}

function SimpleAction({ label, danger, run, onDone }: { label: string; danger?: boolean; run: (operation_id: string) => Promise<void>; onDone: () => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<unknown>(null);
  return (
    <span className="inline-flex items-center gap-2">
      {error != null && <span className="text-xs text-danger">{errorText(error)}</span>}
      <Button
        variant={danger ? 'danger' : 'outline'}
        size="sm"
        disabled={busy}
        onClick={() => {
          setBusy(true);
          setError(null);
          run(crypto.randomUUID())
            .then(onDone)
            .catch(setError)
            .finally(() => setBusy(false));
        }}
      >
        {label}
      </Button>
    </span>
  );
}

const ACTION: Record<string, string> = { put: 'novo', remove: 'removido' };

function itemLabel(item: Item) {
  if (item.action === 'remove') return ACTION.remove;
  return item.before_sha256 ? 'alterado' : ACTION.put;
}

/** The draft of the next version (game-launcher.md 4.10): files by path, content from the panel, publish. */
function DraftSection({ current, admin }: { current: number; admin: boolean }) {
  const queryClient = useQueryClient();
  const q = useQuery({
    queryKey: ['client-draft'],
    queryFn: loadDraft,
    refetchInterval: (query) => (query.state.data?.jobs.some((j) => j.state === 'queued' || j.state === 'running') ? 4000 : false),
  });
  const [open, setOpen] = useState<'upload' | 'remove' | 'publish' | null>(null);
  const done = () => {
    setOpen(null);
    void queryClient.invalidateQueries({ queryKey: ['client-draft'] });
    void queryClient.invalidateQueries({ queryKey: ['client-versions'] });
    void queryClient.invalidateQueries({ queryKey: ['audit'] });
  };
  if (q.isPending) return <p className="text-muted">Carregando o rascunho…</p>;
  if (q.isError) return <p className="text-danger">{errorText(q.error)}</p>;
  const { draft, items, jobs } = q.data;
  const job = jobs[0];
  const busy = jobs.some((j) => j.state === 'queued' || j.state === 'running');
  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-3">
        <h2 className="text-lg font-semibold">Próxima versão ({current + 1})</h2>
        {draft && <Badge>rascunho sobre a versão {draft.base_version}</Badge>}
        {admin && (
          <>
            <Button variant="outline" disabled={!q.data.staging} onClick={() => setOpen('upload')}>
              <FileUp className="size-4" /> Adicionar ou trocar arquivo
            </Button>
            <Button variant="outline" onClick={() => setOpen('remove')}>
              <FileX className="size-4" /> Remover arquivo
            </Button>
            <SimpleAction
              label={busy ? 'Aplicando o conteúdo…' : 'Gerar a partir do conteúdo publicado'}
              run={async (operation_id) => {
                const { data, error, response } = await api.POST('/api/client-versions/draft/content', { body: { operation_id } });
                if (!data) fail(response, error);
              }}
              onDone={done}
            />
          </>
        )}
      </div>
      {job && (
        <p className={`text-sm ${job.state === 'failed' ? 'text-danger' : 'text-muted'}`}>
          Construtor do cliente: {job.state === 'queued' ? 'na fila' : job.state === 'running' ? 'aplicando o conteúdo publicado' : job.state === 'done' ? 'concluído' : 'falhou'}
          {job.message ? ` — ${job.message}` : ''}
        </p>
      )}
      {!draft || items.length === 0 ? (
        <p className="text-sm text-muted">Nenhuma mudança no rascunho. Envie arquivos ou gere a partir do conteúdo publicado no painel.</p>
      ) : (
        <Card className="space-y-2">
          <ul className="divide-y divide-border text-sm">
            {items.map((item) => (
              <li key={item.path} className="flex flex-wrap items-center justify-between gap-2 py-1">
                <span className="flex flex-wrap items-center gap-2">
                  <Badge>{itemLabel(item)}</Badge>
                  <code>{item.path}</code>
                  {item.size != null && <span className="text-xs text-muted">{size(item.size)}</span>}
                  <span className="text-xs text-muted">{item.source === 'content' ? 'conteúdo do painel' : 'manual'}</span>
                </span>
                {admin && (
                  <SimpleAction
                    label="Tirar"
                    run={async (operation_id) => {
                      const { error, response } = await api.POST('/api/client-versions/draft/drop', { body: { operation_id, path: item.path } });
                      if (!response.ok) fail(response, error);
                    }}
                    onDone={done}
                  />
                )}
              </li>
            ))}
          </ul>
          {admin && (
            <div className="flex flex-wrap gap-2">
              <Button disabled={busy} onClick={() => setOpen('publish')}>
                <Rocket className="size-4" /> Publicar a versão {current + 1}
              </Button>
              <SimpleAction
                danger
                label="Descartar o rascunho"
                run={async (operation_id) => {
                  const { error, response } = await api.POST('/api/client-versions/draft/discard', { body: { operation_id } });
                  if (!response.ok) fail(response, error);
                }}
                onDone={done}
              />
            </div>
          )}
        </Card>
      )}
      {open === 'upload' && <UploadModal onClose={() => setOpen(null)} onDone={done} />}
      {open === 'remove' && <RemoveModal onClose={() => setOpen(null)} onDone={done} />}
      {open === 'publish' && <PublishModal next={current + 1} changes={items.length} onClose={() => setOpen(null)} onDone={done} />}
    </div>
  );
}

function size(bytes: number | null | undefined) {
  if (bytes == null) return '—';
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toLocaleString('pt-BR', { maximumFractionDigits: 2 })} GB`;
  return `${(bytes / 1024 ** 2).toLocaleString('pt-BR', { maximumFractionDigits: 1 })} MB`;
}

function day(unix: number) {
  return new Date(unix * 1000).toLocaleString('pt-BR');
}

function MinimumModal({ current, minimum, onClose, onDone }: { current: number; minimum: number; onClose: () => void; onDone: () => void }) {
  const [value, setValue] = useState(String(minimum));
  return (
    <Modal open title="Versão mínima" subtitle="Abaixo dela o launcher não abre o jogo e pede para atualizar." icon={<Rocket className="size-5 text-muted" />} onClose={onClose}>
      <ActionForm
        submitLabel="Salvar"
        onCancel={onClose}
        onDone={onDone}
        onSubmit={async (operation_id) => {
          const { error, response } = await api.POST('/api/client-versions/minimum', { body: { operation_id, minimum: Number(value) } });
          if (!response.ok) fail(response, error);
        }}
      >
        <div className="space-y-1.5">
          <Label htmlFor="minimum">Versão mínima (1 a {current})</Label>
          <Input id="minimum" required inputMode="numeric" pattern="[0-9]{1,6}" value={value} onChange={(e) => setValue(e.target.value.replace(/[^0-9]/g, ''))} />
        </div>
      </ActionForm>
    </Modal>
  );
}

function WithdrawModal({ current, onClose, onDone }: { current: number; onClose: () => void; onDone: () => void }) {
  return (
    <Modal
      open
      title={`Retirar a versão ${current}`}
      subtitle={`Publica a versão ${current + 1} com os arquivos da versão ${current - 1}; os launchers voltam a ela na próxima atualização. Nada é apagado do armazém.`}
      icon={<Undo2 className="size-5 text-muted" />}
      onClose={onClose}
    >
      <ActionForm
        danger
        submitLabel="Retirar"
        onCancel={onClose}
        onDone={onDone}
        onSubmit={async (operation_id) => {
          const { data, error, response } = await api.POST('/api/client-versions/withdraw', { body: { operation_id } });
          if (!data) fail(response, error);
        }}
      />
    </Modal>
  );
}

function NewsModal({ onClose, onDone }: { onClose: () => void; onDone: () => void }) {
  const [title, setTitle] = useState('');
  const [body, setBody] = useState('');
  return (
    <Modal open title="Nova notícia" subtitle="Aparece no launcher (as 20 mais recentes) assim que for salva." icon={<Megaphone className="size-5 text-muted" />} onClose={onClose}>
      <ActionForm
        submitLabel="Publicar"
        onCancel={onClose}
        onDone={onDone}
        onSubmit={async (operation_id) => {
          const { data, error, response } = await api.POST('/api/client-versions/news', { body: { operation_id, title, body } });
          if (!data) fail(response, error);
        }}
      >
        <div className="space-y-1.5">
          <Label htmlFor="news-title">Título (até 120 caracteres)</Label>
          <Input id="news-title" required maxLength={120} value={title} onChange={(e) => setTitle(e.target.value)} />
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="news-body">Texto (até 2.000 caracteres)</Label>
          <textarea
            id="news-body"
            required
            maxLength={2000}
            rows={6}
            value={body}
            onChange={(e) => setBody(e.target.value)}
            className="w-full rounded-lg border border-border bg-surface px-3 py-2 text-sm"
          />
        </div>
      </ActionForm>
    </Modal>
  );
}

function HideButton({ item, onDone }: { item: News; onDone: () => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const toggle = async () => {
    setBusy(true);
    setError(null);
    try {
      const { error: failure, response } = await api.POST('/api/client-versions/news/{id}/hidden', {
        params: { path: { id: item.id } },
        body: { operation_id: crypto.randomUUID(), hidden: !item.hidden },
      });
      if (!response.ok) fail(response, failure);
      onDone();
    } catch (e) {
      setError(e);
    } finally {
      setBusy(false);
    }
  };
  return (
    <span className="flex items-center gap-2">
      {error != null && <span className="text-xs text-danger">{errorText(error)}</span>}
      <Button variant="ghost" size="sm" disabled={busy} onClick={() => void toggle()}>
        {item.hidden ? <Eye className="size-4" /> : <EyeOff className="size-4" />}
        {item.hidden ? 'Mostrar' : 'Ocultar'}
      </Button>
    </span>
  );
}

/** Client versions of the update service (L5): versions, minimum, withdraw, launcher and news. */
export function ClientVersionsPage() {
  const queryClient = useQueryClient();
  const { data: session } = useQuery(sessionQuery);
  const admin = session?.role === 'admin';
  const editor = admin || session?.role === 'gm';
  const q = useQuery({ queryKey: ['client-versions'], queryFn: load });
  const [open, setOpen] = useState<'minimum' | 'withdraw' | 'news' | null>(null);
  const done = () => {
    setOpen(null);
    void queryClient.invalidateQueries({ queryKey: ['client-versions'] });
    void queryClient.invalidateQueries({ queryKey: ['audit'] });
  };
  if (q.isPending) return <p className="text-muted">Carregando…</p>;
  if (q.isError) return <p className="text-danger">{errorText(q.error)}</p>;
  const { store, store_error, news } = q.data;
  const versions = [...(store?.versions ?? [])].reverse();
  return (
    <section className="space-y-6">
      {store_error && <p className="text-sm text-danger">{store_error}</p>}
      {store && (
        <>
          <div className="flex flex-wrap items-center gap-3">
            <Badge>Atual: versão {store.current}</Badge>
            <Badge>Mínima: versão {store.minimum}</Badge>
            <Badge>Launcher: {store.launcher_version ?? 'não publicado'}</Badge>
            <Badge>Armazém: {size(store.store_bytes)}</Badge>
            {admin && (
              <>
                <Button variant="outline" onClick={() => setOpen('minimum')}>
                  Versão mínima
                </Button>
                <Button variant="danger" disabled={store.current < 2} onClick={() => setOpen('withdraw')}>
                  Retirar a versão atual
                </Button>
              </>
            )}
          </div>
          <div className="grid gap-3">
            {versions.map((v) => (
              <Card key={v.version} className="space-y-1">
                <div className="flex flex-wrap items-center gap-2">
                  <p className="font-semibold">
                    Versão {v.version}: {v.title}
                  </p>
                  {v.version === store.current && <Badge>atual</Badge>}
                  {v.version < store.minimum && <Badge>abaixo da mínima</Badge>}
                </div>
                {v.notes && <p className="text-sm text-muted">{v.notes}</p>}
                <p className="flex flex-wrap gap-x-4 text-xs text-muted">
                  <span>{day(v.created_unix)}</span>
                  <span>{v.files.toLocaleString('pt-BR')} arquivos, {size(v.bytes)}</span>
                  <span className="inline-flex items-center gap-1">
                    <HardDriveDownload className="size-3" />
                    instalar {size(v.install_bytes)}
                  </span>
                  {v.update_bytes != null && (
                    <span>
                      atualizar da anterior {size(v.update_bytes)} ({v.deltas} deltas{v.removed > 0 ? `, ${v.removed} removidos` : ''})
                    </span>
                  )}
                </p>
              </Card>
            ))}
          </div>
          <p className="text-xs text-muted">
            Uma versão nova é a atual mais as mudanças do rascunho abaixo, sem montar o cliente inteiro. O launcher publica-se com{' '}
            <code>client-dist launcher --exe … --version …</code> (docs/future/game-launcher.md 4.7 e 4.10).
          </p>
        </>
      )}
      {store && <DraftSection current={store.current} admin={admin} />}
      <div className="space-y-3">
        <div className="flex items-center gap-3">
          <h2 className="text-lg font-semibold">Notícias do launcher</h2>
          {editor && (
            <Button variant="outline" onClick={() => setOpen('news')}>
              Nova notícia
            </Button>
          )}
        </div>
        {news.length === 0 && <p className="text-sm text-muted">Nenhuma notícia.</p>}
        {news.map((n) => (
          <Card key={n.id} className={n.hidden ? 'space-y-1 opacity-60' : 'space-y-1'}>
            <div className="flex flex-wrap items-center justify-between gap-2">
              <p className="font-semibold">{n.title}</p>
              {editor && <HideButton item={n} onDone={done} />}
            </div>
            <p className="text-sm whitespace-pre-line">{n.body}</p>
            <p className="text-xs text-muted">
              {day(n.published_unix)}
              {n.author ? ` · ${n.author}` : ''}
              {n.hidden ? ' · oculta' : ''}
            </p>
          </Card>
        ))}
      </div>
      {store && open === 'minimum' && <MinimumModal current={store.current} minimum={store.minimum} onClose={() => setOpen(null)} onDone={done} />}
      {store && open === 'withdraw' && <WithdrawModal current={store.current} onClose={() => setOpen(null)} onDone={done} />}
      {open === 'news' && <NewsModal onClose={() => setOpen(null)} onDone={done} />}
    </section>
  );
}
