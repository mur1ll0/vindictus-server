import { api, type components, fail } from './client';

export type ShopSummary = components['schemas']['ShopSummary'];
export type ShopRow = components['schemas']['ShopRowView'];
export type CashCategory = components['schemas']['CashCategoryView'];
export type ContentEdit = components['schemas']['ContentEdit'];
export type ContentVersion = components['schemas']['ContentVersion'];
export type EditRequest = components['schemas']['EditRequest'];

export async function shops(): Promise<ShopSummary[]> {
  const { data, error, response } = await api.GET('/api/shops');
  if (!data) fail(response, error);
  return data;
}

export async function shop(shopId: string): Promise<ShopRow[]> {
  const { data, error, response } = await api.GET('/api/shops/{shop_id}', { params: { path: { shop_id: shopId } } });
  if (!data) fail(response, error);
  return data;
}

export async function cashCategories(): Promise<CashCategory[]> {
  const { data, error, response } = await api.GET('/api/cash-shop/categories');
  if (!data) fail(response, error);
  return data;
}

export async function contentState() {
  const { data, error, response } = await api.GET('/api/content');
  if (!data) fail(response, error);
  return data;
}

export async function createEdit(operation_id: string, reason: string, edit: EditRequest) {
  const { data, error, response } = await api.POST('/api/content/edits', { body: { operation_id, reason, edit } });
  if (!data) fail(response, error);
  return data;
}

export async function withdrawEdit(id: number, operation_id: string, reason: string) {
  const { error, response } = await api.POST('/api/content/edits/{id}/withdraw', {
    params: { path: { id } },
    body: { operation_id, reason },
  });
  if (!response.ok) fail(response, error);
}

export async function publish(operation_id: string, reason: string) {
  const { data, error, response } = await api.POST('/api/content/publish', { body: { operation_id, reason } });
  if (!data) fail(response, error);
  return data;
}

export const KIND_LABEL: Record<string, string> = {
  shop_row: 'Linha de loja (adicionar/mudar)',
  shop_row_remove: 'Linha de loja (remover)',
  cash_price: 'Preço de categoria do cash shop',
  cash_default: 'Preço padrão do cash shop',
};
