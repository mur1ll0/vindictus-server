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
- **Voltar à seleção (LogOutMessage 493):** identificado pelo novo log. Original: `GameClient` chama `CleanUpConnections(disconnectLoginConnection: false)`; o `PlayerConn.Closed` de `CharacterSelecting` entra em `CharacterList(sendList: true)` com o cliente conectado (`InitSecondPassword` não envia nada após a senha aprovada) e reenvia `CharacterListMessage`. Implementado: sai do canal, `is_connected=false`, limpa a seleção e envia a lista 479 na mesma conexão; nova seleção funciona. Teste TCP com categoria registrada e GUID. Gate 2026-10-21T00:49:03.0749564+00:00: 395 Rust, 242 PostgreSQL, 48 Python.
- **Diálogo "Security Code is not currently available" no login:** chave `GAMEUI_HEROES_SECONDPASSWORD_LOCKED` (texto do cliente en-EU). O original só a envia quando a operação Check/ChangeSecondPassword falha. Nossos handlers de QueryInventory/EquipItem a enviavam quando chegavam sem personagem selecionado (o cliente consulta o inventário na tela de seleção). Sem PlayerConn o original ignora essas mensagens; agora são apenas registradas por categoria. Gate 2026-10-21T02:34:46.6253892+00:00: 395 Rust, 242 PostgreSQL, 48 Python.

## P0-02d — mover/trocar itens na bolsa (MoveInventoryItem 502)

- Codec 502 (`targetItemID` Int64, `targetStorage` Byte, `targetSlot` Int32) verificado no oracle `ItemSyncWireOracle` (363 casos/arquitetura com 617/358/447, x86=x64).
- `MoveInventoryItemProcessor` → `BagStorage.SwapSlot`: item inexistente, storage de tipo diferente (Bag/Cash/…), coleção indisponível (apenas 0/1/6 sem status de storage) → diálogo `GameUI_Heroes_Inventory_MoveItemFail` (SystemMessage Dialog). Sucesso: posição trocada com o ocupante e `UpdateInventoryInfo617` na ordem RegisterChange (movido, ocupante). Equipamento vestido mantém a parte.
- Divergências deliberadas: slot fora de 0..47 é recusado (o original carregava o item em slot inválido e salvava); mesmo slot não altera nada (o original empilharia o item consigo mesmo).
- Fora desta fatia (recusado sem alteração): empilhar itens da mesma classe (`Item.Stack`, que pode apagar o item movido) e movimentação no storage Cash.
- Testes focados: PostgreSQL mover/trocar/recusas/relogin e equipar (6), wire 502; suíte completa pendente para o fim do bloco P0-02.
- **Empilhar (P0-02e):** soltar um item sobre outro da mesma classe segue `SwapSlot` → `Item.Stack`: só não equipáveis da mesma classe e vinculação (atributos/expiração verificados ausentes); soma até `MaxStack`; ocupante cheio ou classe diferente → troca. Item movido zerado é apagado e enviado como `SlotInfo` vazio (construtor de 13 argumentos); 617 na ordem ocupante → movido. Ouro (checksum dependente da quantidade) recusado nesta fatia. Lacuna: ledger/ItemLog da alteração de quantidade. Testes: unitário do Stack e PostgreSQL de mover (fixture sintética sem empilhável na bolsa; aceite real no cliente com poções/dardos).

## P0-02f — barra de atalhos (quickslots)

- `EquipBundleMessage` 456 (`ItemClass`, `QuickSlotID`) e `QuickSlotInfoMessage` 551 verificados no oracle (465 casos/arquitetura, x86=x64).
- `EquipBundleProcessor` → `ItemOwner.Equip(part, itemClass)`: atalho já no mesmo slot = sucesso sem mudança; classe inexistente → `ConstraintFail_InvalidClass`; `IsConstraintSatisfied`; ARTIFACT (depende da parte 16) recusado nesta fatia; a classe sai do atalho anterior; `StackableSlot.Set` recusa slot bloqueado/inexistente (`InvalidSlot`) e categoria fora da parte (`InvalidItemCategory`). `ItemClass=""` limpa o slot. Classes `skill_` (atalhos de skill) ficam para o P0-03; `fishing_` é aceito sem operação, como no proxy.
- `EquipItem(-1, quickslot)` limpa o atalho e envia só a 551.
- `game_quick_slots` passa a ser carregado (linhas duplicadas/partes desconhecidas recusadas); `InventoryInfo` envia `ToQuickSlotInfo` real: todo atalho desbloqueado em ordem de conteúdo, vazio como `""` (antes era um dicionário vazio). Slots expandidos continuam recusados.
- `Gear.Unequip` executa `UnequipConsumablesByConstraint`: suportado apenas quando as classes da barra não têm requisito de enhance/enchant nem são poções de despertar.
- Perfil de conteúdo v3 inclui categoria/requisitos das 5 classes consumíveis da Fiona; exportador aceita classes sem `EquipItemInfo`.
- Testes focados: unitários `quick_slot`, perfil, wire; 17 PostgreSQL/TCP de regressão. Fixture sintética sem parte consumível; aceite no cliente pelo roteiro [client-test-checklist.md](client-test-checklist.md).

## P0-03a — AP e cápsula de AP

- Codecs `QueryAPMessage` 526, `APMessage` 229 (campos na ordem de declaração: nextBonusTimeTicks, ap, maxap, apBonusInterval), `UseInventoryItemMessage` 629 e `UseInventoryItemOkMessage` 627 verificados no oracle (585 casos/arquitetura, x86=x64).
- Recuperado: a SkillList e o AP **não** são enviados na seleção original (o SkillManager é construído antes de `InitFrontendConn`; o AP só é enviado se o INT mudar com os itens). O cliente pede `QueryAP`/`QuerySkillList` (InGame).
- `QueryAPProcessor`: `UpdateAP` + resposta `APMessage(ap, APMax, next, interval)`. Para a Fiona sem meditação treinada `CalculateAPTime` = 0, então não há regeneração e next/interval = −1. Meditação treinada é recusada.
- `UseInventoryItemProcessor`, ramo `capsule_ap`: `UseUseableItem` destrói 1 unidade do próprio item (apaga se zerar) e salva; envia `UseInventoryItemOk`; quantidade = atributo VALUE (ausente → 0) ou sufixo da classe; 0 → `ItemFail UseItemFail_NotUseableItem` com o item já consumido (como no original); senão `GiveAP(n, allowExceed)` (soma Int32 sem verificação) e `APMessage`. O `Sync` do inventário é enfileirado pelo `Save`, por isso a ordem é 627 → 229/358 → 617. Outros tipos de item usável são recusados sem alteração. Lacunas: ItemLog/journal e growth log.
- Concessão de laboratório pedida pelo operador (21/10/2026): 5× `capsule_ap50` na aba Cash (100) da Fiona, slot livre seguinte, ID por ticks com checagem de unicidade, `tradable` copiado do item Cash existente (mesma regra `Bind=Disable`).
- Testes focados: wire (585 casos), PostgreSQL `QueryAP`/uso não suportado e regressão (18).

## P0-03b — lista de skills (QuerySkillList)

- Codecs `QuerySkillListMessage` 540 e `SkillListMessage` 583 (com `BriefSkillInfo`/`BriefSkillEnhance` em ordem de declaração) verificados no oracle (618 casos/arquitetura, x86=x64).
- `QuerySkillListProcessor`: `SkillListMessage("", 0, ToSkillList(), false)`. Domínio `skill_list` recupera BaseSkillRank (ordem das linhas, duplicata recusada), FinalRank com rank de set bonus, `CalculateRequiredAP` (próximo rank com AP > 0, senão −1), `ConstraintPassedForNextRank` (bloqueio 1/2, rank e AP do próximo, SkillInfo, feature, classe `(restrição & bit) == bit`, nível, pré-requisitos), `GetUntrainAP` e `GetResetAP`. Paladino (entrada de transformação) recusado; enhances de skill não migrados (sem tabela, lista vazia).
- Perfil privado `heroesContents.skill-ranks` (exportador `scripts/content/export_skill_profile.py`, testado): SkillInfo/SkillRankInfo/SkillConstraintInfo das skills selecionadas em ordem de linha (o último duplicado vence, como `ToDoubleDictionary`), vinculado aos hashes do catálogo e das fontes; features avaliadas na política do catálogo. Exportado para as 2 skills da Fiona; configuração do runtime aponta `skills`.
- Testes focados: 4 unitários `skill_list`, wire (618), PostgreSQL `QuerySkillList` sem escrita e regressão (19).

## P0-03c — atalhos de skill

- `EquipBundle` com classe `skill_`: skill inexistente no personagem → `ConstraintFail_InvalidData`; já no mesmo atalho → sem mudança; `SkillInfo` ausente ou `IsActiveSkill` falso (`UseType == 0`) → `ConstraintFail_InvalidData`; sai do atalho anterior; `StackableSlot.SetSkill` só verifica bloqueio (sem categoria). Perfil de skills v2 inclui `UseType`.
- No conteúdo original as duas skills iniciais da Fiona são passivas (`UseType = 0`), portanto não vão para a barra; o comportamento esperado no cliente é a recusa.
- Testes focados: unitários `quick_slot`/`skill_list`, PostgreSQL de skills/equipamento.

## Correção (21/10/2026): skills e AP na entrada

- Teste no cliente: a janela de skills ficou vazia e o log mostrou que o cliente nunca envia `QueryAP`/`QuerySkillList` ao abrir a janela. No original, `InGame.OnEnter` força o observer de Stat (609/619), o de Skill (`SkillListMessage` 583) e o `QueryAP` (`APMessage` 229) sem consulta do cliente. A seleção passa a enviar 483 → 345 → 609 → 619 → 583 → 229 → 468 quando o perfil de skills está configurado. TitleList, SpSkill, VocationSkillList, pescaria, sussurros e missões do mesmo `OnEnter` continuam adiados.
- "Depot" no cliente é o depósito da loja Cash (`CashShopService`: 242 lista de produtos, 238 saldo — sem handler). Fica no P3; as cápsulas de teste estão na aba Cash do inventário (coleção 100).
- Checagem somente leitura na Fiona real: sem vocação, 2 skills cobertas pelo perfil, AP 0. Testes focados: 19 PostgreSQL/TCP com a sequência de entrada verificada.

## P0-03d — treinar skills com AP (LearnSkill) e tela do personagem

- Codecs `LearnSkillMessage` 475, `SkillCompletionMessage` 582 e `QueryCharacterCommonInfoMessage` 527 verificados no oracle (732 casos/arquitetura, x86=x64).
- `LearnSkillProcessor` → `SkillManager.Learn` (AP > 0, `ConstraintPassedForNextRank`, `RequiredAP` > 0, `UseAP`; excedente devolvido com `GiveAP` e `TotalUsedAP += ap − troco`) e `CompleteSkill` (rank + 1, `CurrentAP` = 0, `RecalculateRequiredAP` bloqueia se o próximo rank exige desbloqueio). Mensagens: completo → SkillList (OnSkillRankChanged), AP, SkillList, SkillCompletion; parcial → AP, SkillList; `Learn` falso → nada. Desbloqueio por história (`TryUnlockStory`) e meditação são recusados sem escrita. Logs de skill/crescimento, pontuação de guilda e `UpdateRankBasis` não são gravados.
- Ranks treinados carregam os argumentos de `SkillArgumentInfo` do rank final (`UpdateSkillArgument`), no cache usado por seleção/equipar; perfil de skills v3 inclui os argumentos. O efeito nos atributos aparece no próximo recálculo (equipar ou entrar), como no original.
- `QueryCharacterCommonInfo` do próprio CID responde `CharacterCommonInfoMessage` com o `QueryID` (WhoIs Costume); outros CIDs ainda sem handler.
- `LearnNewSkill` (340, cobra ouro pelo preço da skill) fica para a fatia seguinte, com a carteira de ouro.
- Testes focados: 5 unitários `skill_list`, wire 732, 19 PostgreSQL/TCP (treino parcial, conclusão com troco, relogin no rank treinado, 527).

## Rodada de complementos (21/10/2026)

- **Logs de item:** cada processador aloca o `GetJournalID` (EquipItem 33, EquipBundle 32, MoveInventoryItem 53, UseInventoryItem 68), mesmo quando a execução falha. Mudanças de quantidade (empilhar, usar) geram linhas em `game_item_log` após o Save com `AutoItemLogging`, a última com a contagem do pacote; `UseUseableItem` grava o journal "UseItem" e o ledger DestroyItem/Use. Falhas de log não desfazem a operação.
- **Logs de personagem (skill/crescimento/estado):** dependem do journal de sessão `AddCharacterJournal("Login")` e de tabelas ainda não migradas (só a sequência `game_character_journal_id` existe). Ficam para uma etapa própria de migração de logs de personagem.
- **Vocação:** `ToSkillList` acrescenta `skill_voc_pl_transform00` (Paladino) ou `skill_voc_dk_transform00` (Cavaleiro Sombrio) como o original, em vez de recusar. O sistema de vocação em si (escolha, nível, transformação) continua fora deste bloco.
- **Atalho de artefato:** EquipBundle de classe ARTIFACT exige o mesmo artefato vestido na parte 16 (`ArtifactQuickSlotFail`); vestir um artefato o coloca no atalho de artefato amostrado, no primeiro atalho ARTIFACT vazio ou no maior atalho; desvestir a parte 16 limpa esse atalho; QuickSlotInfo vai depois do sync de equipamento. Sem itens de artefato no perfil atual, o caminho só é exercitado por regras unitárias.
- Correção: a checagem `UnequipConsumablesByConstraint` ignora atalhos de skill (sem ItemClassInfo), evitando falhar o equipar com skill ativa na barra.
- **Itens usáveis:** além da cápsula de AP, os demais ramos do `UseInventoryItemProcessor` continuam recusados. A caixa `randomitem_levelup_v2_10` exige nível 10 (o original recusa a Fiona nível 1); abrir caixas exige `RandomItemV2` e um pipeline geral de entrega de itens (`GiveItem`), que será a fundação de loja/loot/recompensas.
- **Melhorias de skill (SkillEnhance):** não migradas (sem tabelas, pedras de melhoria e sessões); a lista envia enhances vazios. Fica no P2 (evolução de equipamento/skill).
