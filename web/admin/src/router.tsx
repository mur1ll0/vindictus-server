import type { QueryClient } from '@tanstack/react-query';
import { createRootRouteWithContext, createRoute, createRouter, Outlet, redirect } from '@tanstack/react-router';
import { homeOf, sessionQuery } from '@/api/session';
import { Layout } from '@/components/Layout';
import { AccountsPage } from '@/pages/Accounts';
import { AuditPage } from '@/pages/Audit';
import { CashShopPage } from '@/pages/CashShop';
import { TuningPage } from '@/pages/Tuning';
import { ContentPage } from '@/pages/Content';
import { ShopsPage } from '@/pages/Shops';
import { CharacterPage } from '@/pages/Character';
import { LoginPage } from '@/pages/Login';
import { ModelsPage } from '@/pages/Models';
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
const shopsRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/lojas',
  beforeLoad: staffOnly,
  component: ShopsPage,
});
const cashShopRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/cash-shop',
  beforeLoad: staffOnly,
  component: CashShopPage,
});
const tuningRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/ajustes',
  beforeLoad: staffOnly,
  component: TuningPage,
});
const contentRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/conteudo',
  beforeLoad: staffOnly,
  component: ContentPage,
});
const modelsRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/modelos',
  beforeLoad: staffOnly,
  component: ModelsPage,
});
/** Character sheet: the server answers only for staff or the character's own account. */
const characterRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/personagens/$cid',
  component: CharacterPage,
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
      shellRoute.addChildren([
        indexRoute,
        accountsRoute,
        shopsRoute,
        cashShopRoute,
        tuningRoute,
        contentRoute,
        modelsRoute,
        auditRoute,
        characterRoute,
        myAccountRoute,
      ]),
    ]),
    context: { queryClient },
  });
}

declare module '@tanstack/react-router' {
  interface Register {
    router: ReturnType<typeof makeRouter>;
  }
}
