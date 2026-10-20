# P0-02 — equipar e desequipar a Fiona preservada

P0-02a prepara os contratos e as transições de slots. O incremento de leitura aceito no commit `d2fc604` continua ativo. O handler de mutação ainda depende do loader de equipamento vestido, dos requisitos e da sincronização completa descritos abaixo.

## Contrato original e comportamento recuperado

- `EquipItemMessage`: categoria 457, GUID `a6551816-3ca1-38a2-8ec0-7ecba2ac7f36`, `ItemID` Int64 e `PartID` Int32. `ItemID=-1` solicita desequipar pelo mesmo caminho. O codec preserva os demais valores assinados para validação na autoridade de armazenamento.
- `EquipmentInfoMessage`: categoria 458, GUID `9df997b4-ce70-3558-86a1-7ee1ea91325c`, dicionário nullable de parte Int32 para ID Int64. Nulidade do objeto e do dicionário são estados distintos; não substituir um pelo outro.
- `ItemServiceProxy.NormalState` confere `BattleServiceProxy.IsEquipAllowed` e a condição PvP antes de solicitar a operação. O recorte inicial será a cidade já implementada; sistemas PvP permanecem adiados.
- `EquipItemProcessor` usa `ItemID=-1` para `Unequip(part)` seguido de `Save`. Para equipar, encontra o item pelo dono, remove a parte anterior quando diferente, chama `ItemOwner.Equip` e salva somente após sucesso. O processador também participa do journal/EventType.EquipItem. Não basta atualizar `game_equippables.part` e anunciar sucesso.
- `ItemOwner.Equip` verifica requisitos, chama `Gear.CheckEquippable`, desequipa o item do destino e então chama `Gear.Equip(skipCheck:true)`. A validação do item novo precede a retirada do item anterior. Repetir Set em slot ocupado retorna `EnhanceItemFail_System`; um item já equipado falha em CheckSettable. Não inventar sucesso idempotente para esses casos.
- `EquippableSlot` usa a primeira linha de metadata para IsExpandedSlot. CheckSettable recusa na ordem: slot bloqueado, item nulo, classe não equipável, item já equipado, categoria incompatível. `Load(Item)` tem regras próprias: aceita slot bloqueado, somente se vazio e a parte resolvida for nula ou igual ao slot. O loader externo `Gear.Load` deve comprovar `IsEquipped` antes disso.
- Set muda EquippedPart e amostra BadStatus antes de CheckAttributeExpired; em seguida registra alteração, aplica EnhanceOther quando necessário e atualiza os stats do item. Clear retira apenas EquippedPart e mantém identidade/localização na bolsa. Reset e os dois caminhos de expansão têm efeitos diferentes; carregar expansão não cria uma nova linha persistida.

## Entrega P0-02a

- Codec puro de 457/458 com envelopes registrado e GUID, inteiros assinados, nulidade e limites de tamanho/contagem. Reutiliza o codec de dicionário do inventário, inclusive recusa de chaves duplicadas. Nenhum handler de mutação foi ligado ao frontend.
- Kernel `equipment_slot` para construção, check, load, set, clear, reset e expansão. Categoria é UTF16 ordinal opaco, inclusive surrogate isolado; perfis ambíguos ou fora dos limites são recusados sem reparo. O kernel emite uma lista de efeitos para o coordenador futuro; não executa callbacks, SQL ou expiração. Exceções/falhas dos callbacks permanecem fora deste recorte.
- Oracle compila os métodos originais de EquippableSlot, com metadata/itens sintéticos e efeitos instrumentados: 128 sequências por arquitetura, comparando estados, ordem dos efeitos e recusas. Não executa o PlayerService original completo.
- Oracle do serializer/reader Framework com os DTOs recuperados: 90 casos por arquitetura, x86=x64, incluindo ItemID=-1, limites dos inteiros, dicionário nulo/vazio e envelopes. Identidades são conferidas contra os metadados do assembly original.
- Testes Rust comparam ambos os oracles e cobrem perfis inválidos, truncamentos, bytes extras, tipo incorreto, contagens excessivas e duplicatas. Inventário aceito e projeção de slots vazios são revalidados junto dos testes focados.

Verificação e hashes: [minimum-equipment-foundation.json](evidence/auth/minimum-equipment-foundation.json). Esse aceite automático certifica a base de P0-02a; não certifica equipar no cliente.

## Próxima fatia: equipamento vestido até o cliente

1. Exportar e validar as referências de classe/categoria/partes, EquipClass, requisitos de nível/classe/skills e dependências efetivas. `EntityCommunicator.IsConstraintSatisfied` inclui café, limites de nível, classe, guilda e requisitos de equipamentos; antecipar apenas dependências comprovadas dos cinco equipamentos iniciais e recusar estados não suportados. Não tratar dados ausentes como permissão.
2. Carregar Gear a partir dos itens realmente equipados. Recuperar `ToEquippedItemInfo`, placeholders, BadStatus, aparência/Avatar, set bonuses e skills de itens. A carga atual usa `EmptyGear`; ampliar CalculateItemStat para pesos reais com a aritmética Framework explícita. A lista do lobby e a seleção devem aceitar o estado vestido antes de permitir que o writer o grave.
3. Integrar a transação de equipar/desequipar com dono, revisão, marcador, perfil e locks. Preservar troca de destino, item/slot/quantidade, comportamento de falha e journal/logs conforme o original. Validar cidade/segunda senha e concorrência. Não reaplicar defaults ou autoequipar a criação.
4. Sincronizar EquipmentInfo458, atributos/carga, `CostumeManager.LoadItemCostumes`/Save, resumo e NotifyLook no canal. Fonte `CharacterImpl.UpdateItemStat` salva aparência e atualiza stats/look; omitir essas dependências produziria um item vestido no banco e invisível ou inválido no cliente.
5. Testes PostgreSQL/TCP de troca, desequipar, requisitos, falhas e relogin; gate único do lote e teste real. Aceite: vestir/retirar os cinco equipamentos, conferir aparência/atributos e relogar, sem perder ou duplicar itens. Somente então habilitar outras mutações P0-02 e seguir para skills P0-03.

As fontes originais permanecem somente leitura; Fiona/schema33 e os 11 itens são preservados nesta preparação.

## Entrega P0-02b — equipamento vestido na leitura (20/10/2026)

- Perfil somente leitura `heroesContents.normal-equipment` (classe/categoria/nível, EquipClass/CostumeType, sets e efeitos) vinculado aos hashes do catálogo e às fontes; exportador Python com testes.
- `minimum_equipment::load` carrega Gear a partir de `game_equippables.part`, recusa partes inválidas/duplicadas, Avatar, atributos, expiração, storages extras e quickslots. Projeção `ToEquippedItemInfo` (placeholders, peso inteiro, slots desativados por durabilidade), `ToEquipmentInfo` e set bonus com dedup por (SetID, ItemClass).
- Seleção/relogin: EquipmentInfo do InventoryInfo468 reflete as partes vestidas; UpdateStat usa CalculateItemStat com peso real (perfil Framework x64) e a aparência aplica `LoadItemCostumes` sobre os costumes salvos. Sem itens vestidos, o caminho aceito anteriormente fica inalterado.
- Oracles Framework (Gear/costume/load) x86=x64 com fixtures sintéticas; testes PostgreSQL/TCP de seleção com equipamento salvo, consultas repetidas e reentrada sem escrita.
- Gate 2026-10-20T19:56:44Z: 390 Rust, 241 PostgreSQL nativos (0 ignorados), 48 Python, zero falhas/avisos. Pré-verificação somente leitura na base real: 11 itens, 5 classes de metadata, 0 equipados, nenhum DML. Evidência: [minimum-equipped-read.json](evidence/auth/minimum-equipped-read.json).
- Ainda não habilitado: handler EquipItem457 (transação, ItemFail, UpdateInventoryInfo/EquipmentInfo/UpdateStat/CostumeUpdate). Próxima fatia.

## Entrega P0-02c — equipar/desequipar no cliente (20/10/2026)

Handler `EquipItem457` ligado no frontend após a seleção (mesma admissão de segunda senha/seleção do QueryInventory). Coordenador `minimum_equip` segue `EquipItemProcessor` → `ItemOwner.Equip` → `Gear`/`EquippableSlot`:

- `ItemID=-1`: `Unequip(part)`; slot vazio ou quickslot vazio não salva nem envia nada; parte inexistente (NRE no original) é rejeitada sem resposta e sem desconectar.
- Item inexistente/de outro dono → `ItemFail358("GameUI_Heroes_EquipItemFail_System")`. `cash_freepass_` sem LOOK → sem alteração.
- Item vestido em outra parte é retirado antes; depois `IsConstraintSatisfied` (café, nível mín./máx., ClassRestriction `1<<class`, guilda, skill exigida com rank ausente = -1, enhance/enchant), `CheckEquippable` (InvalidSlot / CheckSettable), retirada do destino e `Set`. Recusas enviam `ItemFail358` com o nome do `ItemOperationResult`.
- Sucesso: `game_equippables.part` atualizado; `CostumeManager.Save` após `LoadItemCostumes` (insere/atualiza linhas de item, remove com `DeleteOnNull`; linhas base nunca alteradas). Ordem de pacotes do `Sync`: `UpdateInventoryInfo617` (SlotInfo dos itens modificados, ordem de registro do HashSet), `EquipmentInfo458`, e do `UpdateItemStat`: `UpdateStat619`, `CostumeUpdate447`. A aparência do canal (`look`) da sessão é atualizada para entradas seguintes.
- Perfil de equipamento v2 inclui os campos de requisito de `ItemClassInfo`; os 5 itens iniciais: nível 1, sem limite superior, classe 2047/2, sem café/skill/enhance.

Diferenças deliberadas e limites:
- Uma única transação para item + costume + projeção. No original, falha de restrição/slot após retirar o item de outra parte deixava o estado em memória não salvo; aqui há rollback completo.
- `ItemLog`/journal `EventType.EquipItem` e `NotifyLook` para outros jogadores no canal ainda não são gravados/difundidos (sessão solo).
- Itens fora do perfil de equipamento, Avatar, atributos, expiração, quickslots e slots expandidos continuam recusados.

Verificação: oracle Framework `ItemSyncWireOracle` (617/358/447, 72 casos/arquitetura, x86=x64); testes de requisitos; PostgreSQL de equipar → recusas → relogin → desequipar restaurando todas as linhas; TCP desequipar → reequipar → recusa pelo socket. Aceite no cliente ainda pendente.

## Aceite e ajustes (20/10/2026)

- **Aceito pelo operador no cliente 1.69 EN:** equipar/desequipar e relogin mantendo o resultado.
- **Latência observada (~2 s por troca):** causa medida no nosso código, não no Docker. `item_catalog_inspection::catalog` reconstrói o catálogo efetivo (19.702 classes) a cada chamada: ~46 ms em debug, ~11 ms em release. Um equipar fazia ~35–40 reconstruções. Correção: metadados/requisitos do perfil resolvidos uma vez ao anexar os assets; stats vinculados uma vez por operação; o snapshot do inventário reaproveita o EquipmentInfo já projetado. Commits do PostgreSQL no Docker Desktop: ~15–30 ms, com picos ocasionais de ~1 s no volume.
- **Voltar à seleção de personagem desconectava:** o servidor de canal tratava qualquer mensagem diferente de MovePartition/UpdateAction como erro e encerrava a conexão. Agora mensagens sem handler (canal e frontend após seleção) são registradas apenas por categoria e ignoradas; o retorno ao lobby em si ainda não está implementado e depende de identificar a mensagem enviada pelo cliente.
- Gate 2026-10-20T22:12:05.8417607+00:00: 395 Rust, 242 PostgreSQL, 48 Python, zero falhas/avisos.
- **Segundo ajuste de latência:** o log com `elapsed_ms` mostrou ~1000 ms constantes. Medição com os assets reais: `stats.bind` custa ~214 ms no debug (catálogo efetivo + stats de todos os equipamentos) e ainda rodava 4× por troca. Agora `Assets` mantém um índice owned (`StatsIndex`) vinculado uma vez na carga; demais etapas medidas ≤1,3 ms. Gate 2026-10-20T23:11:14.2655867+00:00: 395 Rust, 242 PostgreSQL, 48 Python.
- **Voltar à seleção:** com o novo log, o cliente não envia nenhuma mensagem ao frontend nem ao canal; apenas fecha as conexões. A implementação depende de identificar o comportamento esperado do cliente.
