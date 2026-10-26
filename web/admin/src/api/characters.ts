import { api, type components, fail } from './client';

export type CharacterSummary = components['schemas']['CharacterSummary'];
export type CharacterSheet = components['schemas']['CharacterSheet'];
export type CharacterItem = components['schemas']['CharacterItem'];

/** BaseCharacter (ServiceCore) with the English game names. */
export const CLASS_NAME: Record<number, string> = {
  0: 'Lann',
  1: 'Fiona',
  2: 'Evie',
  3: 'Karok',
  4: 'Kai',
  5: 'Vella',
  6: 'Hurk',
  7: 'Lynn',
  8: 'Arisha',
  9: 'Hagie',
};

export function className(value: number) {
  return CLASS_NAME[value] ?? `Classe ${value}`;
}

/** Bag collections in ValidBagStorageIDs order, then the special collections. */
const BAG_ORDER = [0, 1, 6, 2, 3, 4, 5, 7, 8];
const SPECIAL: Record<number, string> = {
  100: 'Cash',
  101: 'Conta (ouro)',
  102: 'Restauração',
  103: 'Quest',
};

export function collectionName(value: number) {
  const bag = BAG_ORDER.indexOf(value);
  if (bag >= 0) return `Bolsa ${bag + 1}`;
  return SPECIAL[value] ?? `Coleção ${value}`;
}

/** Bag collections hold 48 slots (StorageSetting.IsValid). */
export const BAG_SLOTS = 48;

export function isBag(value: number) {
  return BAG_ORDER.includes(value);
}

export function collectionOrder(value: number) {
  const bag = BAG_ORDER.indexOf(value);
  return bag >= 0 ? bag : 100 + value;
}

export async function accountCharacters(username: string): Promise<CharacterSummary[]> {
  const { data, error, response } = await api.GET('/api/accounts/{username}/characters', {
    params: { path: { username } },
  });
  if (!data) fail(response, error);
  return data;
}

export async function myCharacters(): Promise<CharacterSummary[]> {
  const { data, error, response } = await api.GET('/api/me/characters');
  if (!data) fail(response, error);
  return data;
}

export async function characterSheet(cid: string): Promise<CharacterSheet> {
  const { data, error, response } = await api.GET('/api/characters/{cid}', { params: { path: { cid } } });
  if (!data) fail(response, error);
  return data;
}

/** MoveInventoryItem with the game's SwapSlot; the character must be offline. */
export async function moveItem(cid: string, item: string, storage: number, slot: number, operation_id: string, reason: string) {
  const { error, response } = await api.POST('/api/characters/{cid}/inventory/move', {
    params: { path: { cid } },
    body: { operation_id, reason, item, storage, slot },
  });
  if (!response.ok) fail(response, error);
}

/** SortInventory: -1 every bag, one bag, 100 cash or 103 quest. */
export async function sortInventory(cid: string, target: number, operation_id: string, reason: string) {
  const { data, error, response } = await api.POST('/api/characters/{cid}/inventory/sort', {
    params: { path: { cid } },
    body: { operation_id, reason, target },
  });
  if (!data) fail(response, error);
  return data.moved;
}

export function gold(value: number) {
  return value.toLocaleString('pt-BR');
}
