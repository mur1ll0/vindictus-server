import { api, type components, fail } from './client';

export type EditorItem = components['schemas']['EditorItemSummary'];
export type EditorDetail = components['schemas']['EditorItemDetail'];
export type ContentStatus = components['schemas']['ItemContentStatus'];
export type StoredKit = components['schemas']['StoredKit'];
export type Fields = Record<string, string | number>;
export type Draft = Omit<components['schemas']['ItemDraft'], 'fields'> & { fields: Fields };

export async function editorItems(search: string, filter?: string): Promise<EditorItem[]> {
  const { data, error, response } = await api.GET('/api/item-editor/items', {
    params: { query: { search, filter: filter || undefined } },
  });
  if (!data) fail(response, error);
  return data;
}

export async function editorItem(itemClass: string): Promise<EditorDetail> {
  const { data, error, response } = await api.GET('/api/item-editor/items/{item_class}', {
    params: { path: { item_class: itemClass } },
  });
  if (!data) fail(response, error);
  return data;
}

export async function itemStatus(): Promise<ContentStatus> {
  const { data, error, response } = await api.GET('/api/item-editor/status');
  if (!data) fail(response, error);
  return data;
}

export async function saveItem(operation_id: string, draft: Draft) {
  const { data, error, response } = await api.POST('/api/item-editor/items', {
    // The editable columns are free-form in the schema (checked by the server).
    body: { operation_id, draft: draft as components['schemas']['ItemDraft'] },
  });
  if (!data) fail(response, error);
  return data;
}

export async function hideItem(itemClass: string, operation_id: string, hidden: boolean) {
  const { data, error, response } = await api.POST('/api/item-editor/items/{item_class}/hide', {
    params: { path: { item_class: itemClass } },
    body: { operation_id, hidden },
  });
  if (!data) fail(response, error);
  return data;
}

export async function removeItem(itemClass: string, operation_id: string) {
  const { data, error, response } = await api.POST('/api/item-editor/items/{item_class}/remove', {
    params: { path: { item_class: itemClass } },
    body: { operation_id },
  });
  if (!data) fail(response, error);
  return data;
}

export async function restoreItem(itemClass: string, operation_id: string) {
  const { data, error, response } = await api.POST('/api/item-editor/items/{item_class}/restore', {
    params: { path: { item_class: itemClass } },
    body: { operation_id },
  });
  if (!data) fail(response, error);
  return data;
}

export async function uploadKit(file: File): Promise<StoredKit> {
  const { data, error, response } = await api.POST('/api/item-editor/kits', {
    body: file as unknown as number[],
    bodySerializer: (body) => body as unknown as BodyInit,
    headers: { 'Content-Type': 'application/zip' },
  });
  if (!data) fail(response, error);
  return data;
}

/** Bit of each class in ItemClassInfo.ClassRestriction; the 1.69 client knows the first ten. */
export const CLASSES: [string, number][] = [
  ['Lann', 1],
  ['Fiona', 2],
  ['Evie', 4],
  ['Karok', 8],
  ['Kai', 16],
  ['Vella', 32],
  ['Hurk', 64],
  ['Lynn', 128],
  ['Arisha', 256],
  ['Hagie', 512],
];

/** Editable columns, in form order: [column, label, kind]. */
export const ITEM_FIELDS: [string, string, 'text' | 'int' | 'float'][] = [
  ['Icon', 'Ícone', 'text'],
  ['Category', 'Categoria', 'text'],
  ['TradeCategory', 'Categoria de troca', 'text'],
  ['TradeCategorySub', 'Subcategoria de troca', 'text'],
  ['Rarity', 'Raridade', 'int'],
  ['Level', 'Nível do item', 'int'],
  ['RequiredLevel', 'Nível exigido', 'int'],
  ['MaxStack', 'Pilha máxima', 'int'],
  ['Price', 'Preço', 'int'],
  ['SellPrice', 'Preço de venda', 'int'],
  ['ExpireIn', 'Expira em', 'int'],
];

export const EQUIP_FIELDS: [string, string, 'text' | 'int' | 'float'][] = [
  ['CostumeType', 'CostumeType (modelo)', 'int'],
  ['MaxDurability', 'Durabilidade', 'int'],
  ['ATK', 'ATK', 'int'],
  ['MATK', 'MATK', 'int'],
  ['ATK_Speed', 'Vel. de ataque', 'int'],
  ['Critical', 'Crítico', 'int'],
  ['Balance', 'Equilíbrio', 'int'],
  ['DEF', 'DEF', 'int'],
  ['Res_Critical', 'Resist. crítico', 'int'],
  ['STR', 'STR', 'int'],
  ['DEX', 'AGI (DEX)', 'int'],
  ['INT', 'INT', 'int'],
  ['WILL', 'WILL', 'int'],
  ['LUCK', 'LUCK', 'int'],
  ['HP', 'HP', 'int'],
  ['STAMINA', 'Stamina', 'int'],
  ['Weight', 'Peso', 'float'],
  ['Dyeable', 'Tingível (0/1)', 'int'],
];
