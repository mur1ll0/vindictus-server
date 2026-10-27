import { useMutation, useQueryClient, useSuspenseQuery } from '@tanstack/react-query';
import { Link, Outlet, useNavigate, useRouterState } from '@tanstack/react-router';
import { Box, FileStack, Gem, History, LogOut, Moon, Shield, SlidersHorizontal, Store, Sun, UserRound, Users } from 'lucide-react';
import { type ReactNode, useState } from 'react';
import { logout, sessionQuery } from '@/api/session';
import { roleLabel } from '@/components/forms';
import { Button } from '@/components/ui/button';
import { applyTheme, initialTheme, type Theme } from '@/lib/theme';
import { cn } from '@/lib/utils';

type Item = { to: string; label: string; icon: ReactNode };

const STAFF: Item[] = [
  { to: '/contas', label: 'Contas', icon: <Users /> },
  { to: '/lojas', label: 'Lojas de NPC', icon: <Store /> },
  { to: '/cash-shop', label: 'Cash shop', icon: <Gem /> },
  { to: '/ajustes', label: 'Ajustes de jogo', icon: <SlidersHorizontal /> },
  { to: '/conteudo', label: 'Conteúdo', icon: <FileStack /> },
  { to: '/modelos', label: 'Modelos 3D', icon: <Box /> },
  { to: '/auditoria', label: 'Auditoria', icon: <History /> },
];
const OWN: Item[] = [{ to: '/minha-conta', label: 'Minha conta', icon: <UserRound /> }];

export const TITLES: Record<string, { title: string; subtitle: string }> = {
  '/contas': { title: 'Contas', subtitle: 'No jogo, digite o login no ID (senha em branco); a senha protege o painel' },
  '/lojas': { title: 'Lojas de NPC', subtitle: 'O que cada NPC vende; edições entram por versão publicada' },
  '/cash-shop': { title: 'Cash shop', subtitle: 'Preços por categoria do Depot' },
  '/ajustes': { title: 'Ajustes de jogo', subtitle: 'Taxas de EXP/ouro/AP, drops por fase e chances de Enhance/Enchant' },
  '/conteudo': { title: 'Conteúdo', subtitle: 'Edições pendentes, versões publicadas e manutenção' },
  '/modelos': { title: 'Modelos 3D', subtitle: 'Visualize o modelo de um item e exporte o kit para o add-on do Blender' },
  '/auditoria': { title: 'Auditoria', subtitle: 'Toda entrada e toda mudança, com quem, o quê e quando' },
  '/minha-conta': { title: 'Minha conta', subtitle: 'Seus dados, seus personagens e sua senha' },
};

function Section({ label, items }: { label: string; items: Item[] }) {
  return (
    <div className="space-y-1">
      <p className="px-3 pb-1 text-xs font-semibold tracking-wider text-muted uppercase">{label}</p>
      {items.map((item) => (
        <Link
          key={item.to}
          to={item.to}
          className="flex items-center gap-3 rounded-lg border border-transparent px-3 py-2 text-sm text-muted hover:bg-raised hover:text-fg [&_svg]:size-4"
          activeProps={{ className: 'border-accent/60 bg-raised font-medium text-fg' }}
        >
          {item.icon}
          {item.label}
        </Link>
      ))}
    </div>
  );
}

export function Layout() {
  const { data: session } = useSuspenseQuery(sessionQuery);
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const path = useRouterState({ select: (s) => s.location.pathname });
  const [theme, setTheme] = useState<Theme>(initialTheme);
  const exit = useMutation({
    mutationFn: logout,
    onSettled: () => {
      queryClient.clear();
      void navigate({ to: '/login' });
    },
  });
  const toggleTheme = () => {
    const next = theme === 'dark' ? 'light' : 'dark';
    applyTheme(next);
    setTheme(next);
  };
  const staff = Boolean(session?.role);
  const heading =
    TITLES[path] ??
    (path.startsWith('/personagens/')
      ? { title: 'Personagem', subtitle: 'Ficha, inventário, skills, efeitos e títulos (somente leitura)' }
      : { title: 'Painel', subtitle: '' });
  return (
    <div className="flex min-h-screen">
      <aside className="hidden w-60 shrink-0 flex-col gap-6 border-r border-border bg-surface px-3 py-5 md:flex">
        <div className="flex items-center gap-3 px-2">
          <span className="flex size-9 items-center justify-center rounded-lg bg-accent-soft text-accent">
            <Shield className="size-5" />
          </span>
          <div>
            <p className="font-semibold leading-tight">Vindictus</p>
            <p className="text-xs text-muted">Administração</p>
          </div>
        </div>
        {staff && <Section label="Administração" items={STAFF} />}
        <Section label="Conta" items={OWN} />
      </aside>
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex flex-wrap items-center justify-between gap-3 border-b border-border px-4 py-4 md:px-8">
          <div>
            <h1 className="text-xl font-semibold">{heading.title}</h1>
            <p className="text-sm text-muted">{heading.subtitle}</p>
          </div>
          <div className="flex items-center gap-2">
            <span className="hidden text-sm text-muted sm:inline">
              {session?.username} · {roleLabel(session?.role)}
            </span>
            <Button variant="outline" size="sm" aria-label="Alternar tema" onClick={toggleTheme}>
              {theme === 'dark' ? <Sun /> : <Moon />}
            </Button>
            <Button variant="ghost" size="sm" disabled={exit.isPending} onClick={() => exit.mutate()}>
              <LogOut />
              Sair
            </Button>
          </div>
        </header>
        {/* Narrow screens: the sidebar links as a row of tabs. */}
        <nav className="flex gap-1 overflow-x-auto border-b border-border px-4 py-2 md:hidden">
          {[...(staff ? STAFF : []), ...OWN].map((item) => (
            <Link
              key={item.to}
              to={item.to}
              className={cn('whitespace-nowrap rounded-md px-3 py-1.5 text-sm text-muted')}
              activeProps={{ className: 'bg-raised text-fg' }}
            >
              {item.label}
            </Link>
          ))}
        </nav>
        <main className="flex-1 px-4 py-6 md:px-8">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
