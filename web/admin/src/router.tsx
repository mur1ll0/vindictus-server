import { type QueryClient, useMutation, useQueryClient, useSuspenseQuery } from '@tanstack/react-query';
import {
  createRootRouteWithContext,
  createRoute,
  createRouter,
  Link,
  Outlet,
  redirect,
  useNavigate,
} from '@tanstack/react-router';
import type { Role } from '@/api/client';
import { logout, sessionQuery } from '@/api/session';
import { Button } from '@/components/ui/button';
import { AccountsPage } from '@/pages/Accounts';
import { AuditPage } from '@/pages/Audit';
import { LoginPage } from '@/pages/Login';

const ROLE_LABEL: Record<Role, string> = { admin: 'Admin', gm: 'GM', leitura: 'Leitura' };

function Shell() {
  const { data: session } = useSuspenseQuery(sessionQuery);
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const exit = useMutation({
    mutationFn: logout,
    onSettled: () => {
      queryClient.clear();
      void navigate({ to: '/login' });
    },
  });
  return (
    <div className="min-h-screen">
      <header className="border-b border-border bg-surface">
        <div className="mx-auto flex max-w-6xl items-center justify-between px-4 py-3">
          <nav className="flex items-center gap-4 text-sm">
            <span className="font-semibold">Painel Vindictus</span>
            {[
              { to: '/', label: 'Contas' },
              { to: '/auditoria', label: 'Auditoria' },
            ].map((item) => (
              <Link
                key={item.to}
                to={item.to}
                className="text-muted hover:text-fg"
                activeProps={{ className: 'font-medium text-fg' }}
                activeOptions={{ exact: true }}
              >
                {item.label}
              </Link>
            ))}
          </nav>
          <div className="flex items-center gap-3 text-sm">
            <span>
              {session?.username}{' '}
              <span className="text-muted">
                ({session ? (ROLE_LABEL[session.role as Role] ?? session.role) : ''})
              </span>
            </span>
            <Button variant="outline" size="sm" disabled={exit.isPending} onClick={() => exit.mutate()}>
              Sair
            </Button>
          </div>
        </div>
      </header>
      <main className="mx-auto max-w-6xl px-4 py-6">
        <Outlet />
      </main>
    </div>
  );
}

const root = createRootRouteWithContext<{ queryClient: QueryClient }>()({ component: Outlet });

const loginRoute = createRoute({ getParentRoute: () => root, path: '/login', component: LoginPage });

const shellRoute = createRoute({
  getParentRoute: () => root,
  id: 'shell',
  component: Shell,
  beforeLoad: async ({ context }) => {
    const session = await context.queryClient.ensureQueryData(sessionQuery);
    if (!session) throw redirect({ to: '/login' });
  },
});

const accountsRoute = createRoute({ getParentRoute: () => shellRoute, path: '/', component: AccountsPage });
const auditRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/auditoria',
  component: AuditPage,
});

export function makeRouter(queryClient: QueryClient) {
  return createRouter({
    routeTree: root.addChildren([loginRoute, shellRoute.addChildren([accountsRoute, auditRoute])]),
    context: { queryClient },
  });
}

declare module '@tanstack/react-router' {
  interface Register {
    router: ReturnType<typeof makeRouter>;
  }
}
