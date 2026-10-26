import type { QueryClient } from '@tanstack/react-query';
import { createRootRouteWithContext, createRoute, createRouter, Outlet, redirect } from '@tanstack/react-router';
import { homeOf, sessionQuery } from '@/api/session';
import { Layout } from '@/components/Layout';
import { AccountsPage } from '@/pages/Accounts';
import { AuditPage } from '@/pages/Audit';
import { LoginPage } from '@/pages/Login';
import { MyAccountPage } from '@/pages/MyAccount';

type Context = { queryClient: QueryClient };

const root = createRootRouteWithContext<Context>()({ component: Outlet });

const loginRoute = createRoute({
  getParentRoute: () => root,
  path: '/login',
  validateSearch: (search: Record<string, unknown>): { aviso?: string } =>
    typeof search.aviso === 'string' ? { aviso: search.aviso } : {},
  component: LoginPage,
});

const shellRoute = createRoute({
  getParentRoute: () => root,
  id: 'shell',
  component: Layout,
  beforeLoad: async ({ context }) => {
    const session = await context.queryClient.ensureQueryData(sessionQuery);
    if (!session) throw redirect({ to: '/login' });
    return { session };
  },
});

const indexRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/',
  beforeLoad: ({ context }) => {
    throw redirect({ to: homeOf(context.session) });
  },
});

/** Administration pages need a panel role; an account without one goes to its own data. */
function staffOnly({ context }: { context: { session: { role?: string | null } } }) {
  if (!context.session.role) throw redirect({ to: '/minha-conta' });
}

const accountsRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/contas',
  beforeLoad: staffOnly,
  component: AccountsPage,
});
const auditRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/auditoria',
  beforeLoad: staffOnly,
  component: AuditPage,
});
const myAccountRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/minha-conta',
  component: MyAccountPage,
});

export function makeRouter(queryClient: QueryClient) {
  return createRouter({
    routeTree: root.addChildren([
      loginRoute,
      shellRoute.addChildren([indexRoute, accountsRoute, auditRoute, myAccountRoute]),
    ]),
    context: { queryClient },
  });
}

declare module '@tanstack/react-router' {
  interface Register {
    router: ReturnType<typeof makeRouter>;
  }
}
