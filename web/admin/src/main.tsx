import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { RouterProvider } from '@tanstack/react-router';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { ApiFailure } from '@/api/client';
import { initialTheme } from '@/lib/theme';
import { makeRouter } from '@/router';
import './index.css';

document.documentElement.dataset.theme = initialTheme();

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // Client errors (an ended session, a refused role) are not retried.
      retry: (count, error) => !(error instanceof ApiFailure && error.status < 500) && count < 2,
    },
  },
});
const router = makeRouter(queryClient);
// An ended session anywhere sends the user back to the login page.
queryClient.getQueryCache().subscribe((event) => {
  const error = event.query.state.error;
  if (error instanceof ApiFailure && error.code === 'sessao_encerrada') {
    queryClient.setQueryData(['session'], null);
    void router.navigate({ to: '/login' });
  }
});

const element = document.getElementById('root');
if (element) {
  createRoot(element).render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
      </QueryClientProvider>
    </StrictMode>,
  );
}
