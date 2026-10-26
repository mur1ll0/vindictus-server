import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Gem, Search } from 'lucide-react';
import { useState } from 'react';
import { type CashCategory, cashCategories, createEdit } from '@/api/content';
import { sessionQuery } from '@/api/session';
import { ActionForm, errorText } from '@/components/forms';
import { Modal } from '@/components/Modal';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

function PriceModal({ category, onClose, onDone }: { category: CashCategory | 'default'; onClose: () => void; onDone: () => void }) {
  const [price, setPrice] = useState(category === 'default' ? '' : String(category.max_price));
  const title = category === 'default' ? 'Preço padrão' : category.category;
  return (
    <Modal
      open
      title={title}
      subtitle="Preço em cash de cada produto da categoria (produtos VIP por duração seguem a própria regra). Fica pendente até um admin publicar e o host aplicar."
      icon={<Gem className="size-5 text-muted" />}
      onClose={onClose}
    >
      <ActionForm
        submitLabel="Salvar preço"
        onCancel={onClose}
        onDone={onDone}
        onSubmit={async (operationId, reason) => {
          const value = Number(price);
          await createEdit(
            operationId,
            reason,
            category === 'default' ? { kind: 'cash_default', price: value } : { kind: 'cash_price', category: category.category, price: value },
          );
        }}
      >
        <div className="space-y-1.5">
          <Label htmlFor="cash-price">Preço (1 a 1.000.000)</Label>
          <Input id="cash-price" required inputMode="numeric" pattern="[0-9]{1,7}" value={price} onChange={(e) => setPrice(e.target.value.replace(/[^0-9]/g, ''))} />
        </div>
      </ActionForm>
    </Modal>
  );
}

export function CashShopPage() {
  const queryClient = useQueryClient();
  const { data: session } = useQuery(sessionQuery);
  const canEdit = session?.role === 'admin' || session?.role === 'gm';
  const list = useQuery({ queryKey: ['cash-categories'], queryFn: cashCategories });
  const [filter, setFilter] = useState('');
  const [open, setOpen] = useState<CashCategory | 'default' | null>(null);
  const visible = (list.data ?? []).filter((c) => c.category.toLowerCase().includes(filter.trim().toLowerCase()));
  const done = () => {
    setOpen(null);
    void queryClient.invalidateQueries({ queryKey: ['content'] });
    void queryClient.invalidateQueries({ queryKey: ['audit'] });
  };
  return (
    <section className="space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        <div className="relative max-w-sm flex-1">
          <Search className="pointer-events-none absolute top-2.5 left-3 size-4 text-muted" />
          <Input aria-label="Filtrar categorias" placeholder="Filtrar categorias" className="pl-9" value={filter} onChange={(e) => setFilter(e.target.value)} />
        </div>
        {canEdit && (
          <Button variant="outline" onClick={() => setOpen('default')}>
            Preço padrão
          </Button>
        )}
      </div>
      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        {visible.map((c) => (
          <button
            key={c.category}
            type="button"
            disabled={!canEdit}
            onClick={() => setOpen(c)}
            className="rounded-xl border border-border bg-surface p-4 text-left shadow-sm transition-colors enabled:hover:border-accent/70"
          >
            <p className="truncate font-semibold">{c.category}</p>
            <p className="text-xs text-muted">{c.products} produto(s)</p>
            <p className="mt-3 flex items-center gap-1 text-sm font-semibold">
              <Gem className="size-4 text-muted" />
              {c.min_price === c.max_price ? c.max_price.toLocaleString('pt-BR') : `${c.min_price.toLocaleString('pt-BR')}–${c.max_price.toLocaleString('pt-BR')}`}
            </p>
          </button>
        ))}
      </div>
      {list.error && <p className="text-sm text-danger">{errorText(list.error)}</p>}
      {open && <PriceModal category={open} onClose={() => setOpen(null)} onDone={done} />}
    </section>
  );
}
