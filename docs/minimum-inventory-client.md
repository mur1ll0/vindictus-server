# M15.1c.3 — inventário e equipamentos no cliente

Status: diagnóstico inicial concluído; integração TCP ainda pendente. Prioridade P0-01 do [roteiro PvE](playable-game-roadmap.md).

## Evidência atual

O operador confirmou cidade/movimento, mas observou “Downloading data” infinito no inventário e ausência de equipamento visível. Consulta somente leitura da Fiona preservada encontrou **11 itens, 5 equippables e nenhuma parte equipada**. Os itens ocupam 10 slots da bolsa (collection 0, slots 0–9) e um slot Cash (collection 100, slot 0). Não há evidência de perda dos defaults; não recriar nem reaplicar a entrega inicial.

O writer inicial grava `game_equippables.part=NULL` e o `Gear.Load` original só carrega itens `IsEquipped`. `Gear.ToEquipmentInfo` projeta somente itens equipados. Portanto, equipamentos na bolsa não devem ser anunciados como vestidos sem implementar/recuperar a operação de equipar. A projeção atual `initial_gear` representa slots vazios, não um loader de equipamento persistido.

O frontend retorna imediatamente para mensagens comuns depois de `selected.is_some()`. **QueryInventoryMessage 529 não possui handler**, e o caminho mínimo de seleção não envia o snapshot inicial do inventário. Isso deixa a consulta sem resposta e é compatível com o download infinito relatado. Nenhum pacote capturado do cliente foi usado para afirmar campos além dos contratos recuperados.

## Caminho recuperado

- QueryInventoryMessage: categoria **529**, GUID `efe95078-ef7e-34eb-82e1-2daf3280a576`; sem campos de instância.
- InventoryInfoMessage: categoria **468**, GUID `bdc965b5-aca1-3b62-b222-279e96683d93`. Identidades conferem nos metadados Framework x86 e x64.
- `ItemServiceProxy` envia InventoryInfo na sincronização inicial e ao receber QueryInventory. O snapshot vem de `ItemOwner.ToInventoryInfo`: StorageInfo, SlotInfo, EquipmentInfo, QuickSlotInfo e UnequippableParts.
- StorageInfo inclui storage ID/disponibilidade/expiração/nome/tag. SlotInfo inclui identidade/classe/quantidade/local, expiração, cores/durabilidade, atributos, status de enchant, preços e MinUpTime. Preservar nulidade, valores negativos e diferenças entre construtores.
- QuickSlotInfo deriva dos slots persistidos; UnequippableParts deriva dos slots bloqueados. Não presumir coleções vazias para encerrar o loading.

## Próxima fatia de implementação

1. Gerar oracle do serializador/leitor Framework para os dois envelopes e tipos aninhados, com casos de nulidade, limites e coleções. Acrescentar codec puro e testes diferenciais, incluindo entrada registrada e categoria 0/GUID.
2. Carregar um snapshot transacional limitado da Fiona admitida, verificando dono/revisão/marcador/perfil e referências de itens. Reutilizar catálogo, stats/durabilidade, storages e quickslots já recuperados. Responder com os itens reais; nenhum DML de entrega inicial.
3. Ligar sincronização inicial e QueryInventory após seleção à mesma projeção. Cobrir consulta antes da seleção, owner inválido, corrupção/estado parcial, relogin e ausência de efeitos de escrita.
4. Gate único do lote e teste real: inventário deixa de baixar indefinidamente e mostra os itens corretos. Esse aceite cobre **leitura**, não habilita equipar/consumir/mover sem handlers próprios.
5. Seguir P0-02: equipar/desequipar e demais mutações, com persistência, aparência/NotifyLook, stats/carga e validação de requisitos. Confirmar o comportamento de autoequipar na criação pela fonte antes de alterar a política dos defaults.

Evidência sanitizada e hashes das fontes: [minimum-inventory-gap.json](evidence/auth/minimum-inventory-gap.json). Banco real consultado somente leitura; nenhum item, aparência, skill ou equipamento alterado nesta investigação.
