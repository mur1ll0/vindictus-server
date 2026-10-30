# Sistema de pets: estado, IDs e requisitos

Análise de 30/10/2026, a pedido do operador: o sistema de pets já existe no servidor Rust? O ID do pet está certo para o servidor, comparado com os arquivos do NA, pensando em importar depois as demais classes do NA? Tudo foi lido sem executar serviços: código Rust, C# descompilado (`legacy/decompiled`, referência local), catálogos SQL Server (`docs/evidence/database/study_*.schema.json`), consultas SQLite somente leitura e leitura pontual de bytes dos DLLs do NA (`F:\Games\vindictus\appdata\en-US\bin_x64`, hashes iguais aos de `.cache/bin_x64`: `client.dll` `0be9bfa5…`, `server.dll` `9f674dba…`).

## 1. Estado no servidor Rust

**Implementado em 30/10/2026 (P1–P6), sem teste no cliente.** A seção 5 descreve o que foi feito e os desvios; o roteiro de teste é `docs/client-tests/B5-pets.md`. Antes desta data:

- `src/protocol/server_profile.rs` só registrava a categoria/GUID das mensagens de pet (313–332);
- o `PetStatusInfo` saía sempre nulo na lista de personagens e no início de missão (`NoPet`);
- itens `pet_*` eram recusados ao usar;
- `character_class::PET = 28` só era usado em teste.

## 2. Os "IDs do pet": três valores diferentes

| Valor | 1.69 original | Laboratório (patch) | NA | Situação |
|---|---|---|---|---|
| Índice de classe do pet (`m_iClass` da entidade pet nos DLLs) | 10 (= `BaseCharacter.CHARACTER_COUNT`) | **28** | **28** | correto |
| Tipo do pet no conteúdo (`PetInfo.PetID`, enviado como `PetStatusInfo.PetType`) | 518–793 (36 tipos) | igual ao 1.69 | 518–1739 (85 tipos) | sem conflito |
| ID de instância (`Pet.PetID` no SQL, `bigint`) | gerado pelo banco | — | — | a definir no PostgreSQL |

### 2.1 Índice de classe: 28 está certo

Conferido em bytes nos DLLs do NA (30/10/2026):

- tabela de classes do `client.dll` (`0x1f85aa0`, 16 bytes por entrada): 28 nomes, de `Lethita` (0) a `Lumielle` (27), e a entrada 28 é `None`. A ordem é idêntica a `character_class::NAMES`, com `Delia` = 10;
- acessor do nome (`client.dll` `0xb4b30`, `server.dll` `0xb80c0`): `cmp ecx, 0x1b; ja` → classes 0 a 27;
- `switch` das expressões (`0x202dcb`: `cmp esi, 0x1c`) e das palavras-chave de habilidade (`0x28fdd8`: `cmp eax, 0x1c`) → casos 0 a 28; o 28 é `expression_table_pet` / `pet_skill_keyword_table`.

O limite de 32 vem do `m_iClass` de 5 bits (28 classes + pet + 3 valores livres), não de 32 classes. A tabela da seção 1 de `new-classes.md` fala em "32 classes"; a contagem certa é a da seção 8.13 (28).

Para importar as demais classes do NA, 28 continua certo: todas as classes do NA (11 a 27) já cabem abaixo do pet, e o patch de roteamento (`scripts/client/patch_class_routing.py`, `PET = 28`) exige classes contíguas de 10 até 27. Só um NA futuro com mais de 28 classes moveria o pet.

**Pendências do índice do pet no laboratório** (nenhuma bloqueia, todas precisam de teste no cliente quando houver pet):

- o patch move o pet para 28 nos `switch` de expressões e palavras-chave (cliente e servidor) e no portão da preparação do jogador local (`GATES`, `< 28`). Os vetores de armas por classe (`CLASS_WEAPONS`) limitam o índice a 0–9 e mandam o resto para a linha 0, tanto no original (pet 10) quanto agora (pet 28);
- não se sabe onde o 1.69 grava o 10 no `m_iClass` da entidade pet (`new-classes.md` 8.13: nenhuma escrita imediata achada). O `PetStatusInfo` não leva classe (campos na seção 3.1): o valor nasce no DLL, não no servidor. Antes de ligar pets, achar essa escrita e confirmar que ela usa o índice depois da última classe (que agora é 28) e não um 10 fixo. Se for um 10 fixo, a entidade pet cairia nos casos da Delia.

### 2.2 Tipo do pet no conteúdo: o 1.69 é um subconjunto do NA

Comparação de `PetInfo` (`heroesContents.db3` do servidor 1.69 = `Client/sql/heroes.db3`; NA = `F:\Games\vindictus\hfsExtracted\sqlite\heroes.db3`):

- os 36 tipos do 1.69 existem no NA com o mesmo `PetID`, o mesmo `ItemClass` (`pet_create_*`) e os mesmos `DefaultLevel`, `StatBalanceType`, `MaxActiveTime`, `DeactiveTime`, `MaxHunger`, `HungerTime` e `HungerDecrease`;
- o NA tem 49 tipos a mais (960–1739: `pet_create_gremlin_elite`, `succubus_*`…); nenhum dos 49 itens `pet_create_*` existe no 1.69;
- `PetStatusBalanceInfo`: as 150 linhas do 1.69 (5 tipos de atributo × 30 níveis) são iguais no NA, que vai a 9 tipos × 50 níveis;
- `PetSkillInfo`: 43 skills no 1.69, 45 no NA; 4 mudaram de valor (2 e 3 `pet_skill_hp_recovery`: 80 % / 600 s no 1.69, 30 % / 300 s no NA; 6 e 7: faixas de fome 50–75/75–100 no 1.69, 100–149/150–200 no NA);
- `PetFeedInfo`: 4 rações no 1.69; o NA acrescenta a coluna `Priority` e 2 rações;
- o NA não tem `DefaultPetSkillInfo` nem `PetActiveTimePeriodInfo` (o 1.69 tem 234 e 188 linhas) e acrescenta `PetInfo.Race/ItemGrade/UseExtendSkill/Expedition*/ShowingPetBook`, `PetBookBuffInfo`, `PetBookBuffWhiteListInfo`, `PetConditionalInfo`, `PetPreschoolExpInfo` e `PetPreschoolSnackInfo` (livro de pets, pré-escola e expedição, que o cliente 1.69 não tem).

Conclusão: o `PetType` do 1.69 é estável e compatível com o NA. Importar pets do NA é trazer linhas novas de `PetInfo` e seus itens, sem renumerar os atuais.

### 2.3 Máscara de classe dos itens

No 1.69, o bit 10 da `ClassRestriction` já é da Delia (itens `delia_*` com 1024) e nenhum item usa um bit de pet. Os 53 itens `pet_*` usam 2047 (classes 0 a 10). Ao importar as classes 11–27, `2047` não as inclui: o NA guarda `ClassRestriction` como texto (`all`, `female`, `delia,miri`…), e a importação precisa converter `all` para a máscara das 28 classes. Isso vale para itens de pet e para os demais.

O vínculo item → tipo é a `ExtraData` do item `pet_create_*` (inteiro), não a coluna `PetInfo.ItemClass`: no 1.69, `pet_create_wolf_dire` aparece em dois tipos (623 e 652) e o item `pet_create_wolf_dire_event` aponta para o 652 pela `ExtraData`. Conferido nos 37 itens `pet_create_*`: as diferenças são só essas duplicatas e maiúsculas no nome.

## 3. Requisitos para implementar

Origem: C# descompilado (`PlayerService`, `ServiceCore`, `MicroPlayServiceCore`, `FrontendServiceCore`, `DSService`, `UserDSHostService`) e catálogo SQL do `heroes`/`heroesLog`. Caminhos relativos a `legacy/decompiled`.

### 3.1 Protocolo

Cliente → servidor (`FrontendServiceCore.States/InGame.cs`, todas atrás da *feature* `Pet`, exceto a criação):

| Mensagem | Campos | Operação |
|---|---|---|
| `PetCreateNameCheckMessage` | `ItemID` long, `PetName` | checar nome; responde `PetCreateNameCheckResultMessage` e, na falha, diálogo `GameUI_Heroes_Pet_Fail_<resultado>` |
| `PetCreateMessage` | `ItemID`, `PetName` | sem a *feature* `Pet`; exige `IsEquipAllowed(-1)` e a segunda senha; vira `UseInventoryItem` no ramo `pet_create_` |
| `PetChangeNameCheckMessage` / `PetChangeNameMessage` | `ItemID`, `PetID`, `PetName` | renomear com item `pet_name_change*`; respostas `PetChangeNameCheckResultMessage` / `PetChangeNameResultMessage` |
| `QueryPetListMessage` | — | lista completa (`PetListMessage`) |
| `SelectPetMessage` | `PetID` (0 = recolher) | invocar/recolher |
| `MovePetSlotMessage` | `PetID`, `DestSlotNo` | trocar de *slot* |
| `RemovePetMessage` | `PetID` | apagar o pet |
| `RemovePetSkillMessage` | `PetID`, `PetSkillID` | remover skill de cash vencida |
| `RequestPetFoodShareMessage` / `…UnshareMessage` | `ItemID` / `ItemClass` | ração compartilhada (`Pet_food_V2`) |
| `PetOperationMessage` | `OperationCode`, `PetID`, `Arg`, `Value1–3` | comandos de GM (`IsCheat`); vários são vazios no original |

Servidor → cliente: `PetListMessage(PetList, IsTotalPetList)`, `PetFeedListMessage`, os três resultados de nome e diálogos `SystemMessage`. Servidor → host do jogo: `UpdateCurrentPetMessage(UID = slot, CurrentPet)`. Host/DS → servidor: `PetKilledMessage(Tag, PetID)` e `PetRevivedMessage(CasterTag, ReviverTag, PetID, Method)`.

DTOs (`ServiceCore.EndPointNetwork`):
- `PetStatusInfo`: `PetID` long, `PetName`, `PetType` int, `Slot`, `Level`, `Exp`, `Desire` (fome), `Stat`, `Skills`, `Accessories`, `PetStatus` byte, `RemainActiveTime` (s), `RemainExpiredTime` (sempre `UtcNow` no original). **Não tem campo de classe.**
- `PetStatElement` (11 ints), `PetSkillElement` (`SkillID`, `SlotOrder`, `OpenLevel`, `HasExpireDateTimeInfo`, `ExpireDateTimeDiff`), `PetAccessoryElement` (`ItemClass`, `SlotOrder`, `AccessorySize`, `RemainingTime`), `PetFeedElement` (`Type`, `Count`).
- `PetStatus`: 0 Begin, 1 Expired, 2 Disabled, 3 Useabled, 4 Selected, 5 End.

O `PetStatusInfo` não nulo precisa de serialização recuperada e de *fixtures* geradas do C# (`scripts/reverse/generate-fixtures.ps1`), nos três lugares onde aparece: `CharacterSummary.Pet` (lista de personagens), `GameJoinMemberInfo.Pet` (início de missão) e `PetListMessage`/`UpdateCurrentPetMessage`.

### 3.2 Modelo e persistência

SQL Server (`heroes`), alvo PostgreSQL:
- `Pet`: `PetID` bigint (gerado por `MakeEntityID().Ticks`), `OwnerUID` int (**conta, não personagem**: os pets são da conta), `PetName` nvarchar(32), `PetType`, `PetStatType`, `Slot`, `PetStatus` tinyint, `Level`, `Exp`, `TotalExp`, `HungerDesire`, `RemainActiveTime`, `DeactiveStartTime` e `ExpiredTime` (datetime2).
- `PetSkill` (PK `PetID`+`SkillSlot`+`PetSkillID`; `RowID`, `OpenLevel`, `CoolTimeType`, `CoolTime`, `LastSkillUseUpdate`, `ExpiredTime`) e `PetAccessory` (`PetItemID` PK, `PetID`, `ItemClass`, `SlotOrder`, `Size`, `ExpiredTime`), com FK para `Pet`.
- `Pet_Trans` e as procedures `PetTransSelect`, `PetNameUpdate`, `PetNameCheck`, `HDV_PetDelete`, `HDV_ExtendPetSkill`.
- Ração compartilhada: `UserSharedItemGet/Set/Destroy` com `Type = 2`.
- Log (`heroesLog`): `PetLedger01/02` e `AddPetLedger(PetID, CharacterUID, Operation, Event, Arg1, Arg2)`; o `CharacterUID` gravado é o UID da conta. O backup tem 18 + 49 linhas de *ledger*; a contagem de `dbo.Pet` ainda não foi lida.

Regras do gerenciador (`PlayerService.ItemServiceCore.PetSystem/PetManager.cs`): até 100 *slots*, menor *slot* livre; criar exige `Pet` ligado e `PetPurchaseLimit` maior que o número de pets; um único pet atual (status 4), restaurado no carregamento.

### 3.3 Regras de jogo

- **Criação:** consome o item `pet_create_*` (`ExtraData` = tipo); checa compra liberada, nome, `PetInfo` e *slot*; na falha devolve o item (ou manda por correio). Valores iniciais de `PetInfo`: nível `DefaultLevel`, atributos `StatBalanceType`, fome `MaxHunger`, tempo ativo `MaxActiveTime`, status 3.
- **Nome:** nome proibido (`IsForbiddenName`), regex `NamingRule`, limite `NamingRuleMaxBytes` (metade para não ASCII), único entre os pets da mesma conta, sem diferenciar maiúsculas. Há também a troca forçada na seleção de personagem (`PetTransSelect` → `TransferValuesSetMessage "SetPetName"` → `PetNameChangeProcessor`).
- **Invocar:** `SelectPet(0)` recolhe; trocar de pet volta o anterior a 3. Com `Pet_food_V2`, alimenta ao invocar.
- **Ração:** V1 (`pet_feed_*`, `ExtraData` = quantidade, teto `MaxHunger`) e V2 (ração depositada por conta, alimentação automática em ordem de `FeedType`, devolução como item vinculado).
- **Fim de missão:** EXP = EXP de monstros × `Pet_ExpBattle_Point`/100, limitada a `MaxExp` do nível; sobe de nível até `PetMaxLevel` (30 no 1.69); fome cai `HungerDecrease × tempo / HungerTime`; o tempo ativo cai e, ao zerar, o pet fica desativado (2). Sem EXP em fracasso por tempo mínimo, recompensa nula ou saída do jogador. Pets não valem em raide gigante nem em missão de caça.
- **Tempo ativo diário:** `PetActiveTimePeriodInfo` reinicia o tempo e reativa o pet a cada período.
- **Skills:** padrão vem do conteúdo (`DefaultPetSkillInfo`, não vai ao banco); skills de cash (`pet_skill_*`, validade pelo nome `day`/`min`) nos *slots* 101 e 102, no máximo 2; remover só se vencida. `LearnPetSkill` e `GivePetSkillPoint` sempre falham no original.
- **Acessórios:** o código existe, mas `EquipPetAccessory` não tem chamador; fica fora do primeiro corte.
- **Batalha:** o servidor não cria a entidade; manda o `PetStatusInfo` no `GameJoinMemberInfo` e o host (cliente ou DS) cria o pet. `MicroPlayerPet` conta tempo e EXP; morto/revivido param e retomam a contagem. Mudança de pet durante a missão vai por `UpdateCurrentPetMessage`.
- **Histórias:** a condição `Cond_has_pet(tipo)` usa o tipo inteiro.

Comportamentos do original para decidir (copiar ou corrigir): `UpdatePetStatus` marca vencido quando `ExpiredTime > agora` (parece invertido); `RemainExpiredTime` sempre `UtcNow`; `RemainingTime` do acessório usa só a parte dos segundos; `GiveSkill` lê o tipo do pet atual, não do alvo; `SelectPet` não confere desativado/vencido.

### 3.4 *Feature flags* (EN-EU-X-GM no 1.69)

`Pet` = `1.09.03` (ligado), `Pet_food_V2` = `2.02.01` (ligado), `PetPurchaseLimit` = 100, `PetMaxLevel` = 30, `Pet_ExpBattle_Point` = 20, e `NamingRule`/`NamingRuleMaxBytes`. As linhas de `PetInfo` têm `Feature = Pet`.

### 3.5 Dependências no servidor Rust

- Inventário e uso de itens: o ramo `pet_` de `minimum_use.rs`, entrega/devolução com correio quando cheio.
- Política de nomes já usada na criação de personagem (`docs/character-naming.md`).
- Missão (B2): `GameJoinMemberInfo` com pet, contagem de EXP de monstros, fim de missão e mensagens do host (`PetKilled`/`PetRevived`), pelo ponto único de difusão (o pet é visível aos outros jogadores no B5).
- Painel (B4): ver, criar e remover pets de uma conta.

## 4. Etapas propostas

1. **P1 — conteúdo e esquema:** carregar `PetInfo`, `PetStatusBalanceInfo`, `PetSkillInfo`, `DefaultPetSkillInfo`, `PetActiveTimePeriodInfo` e `PetFeedInfo` com o filtro de *feature*; migração PostgreSQL `pets`, `pet_skills`, `pet_accessories`, `pet_ledger` por conta. Cliente: não necessário.
2. **P2 — codecs:** `PetStatusInfo` e mensagens da seção 3.1 com *fixtures* do C#; a lista de personagens e o `GameJoinMemberInfo` passam a aceitar pet não nulo.
3. **P3 — criar e listar:** checagem de nome, `PetCreateMessage` pelo uso do item, `QueryPetList`, renomear, remover, mover *slot*. Primeiro teste no cliente: criar um pet com um item `pet_create_*` dado por comando de laboratório e vê-lo na janela de pets após relogar.
4. **P4 — invocar e alimentar:** `SelectPet`, ração V1 e V2, `PetFeedListMessage`.
5. **P5 — pet na missão:** `PetStatusInfo` no início de missão, EXP/fome/tempo no fim, morto/revivido, reinício diário. *Checkpoint* de cliente: o pet aparece na fase com o índice 28 (seção 2.1) antes de seguir para EXP.
6. **P6 — skills de cash e painel.** Acessórios e os sistemas só do NA (livro, pré-escola, expedição) ficam fora, porque o cliente 1.69 não os tem.
7. **Importação dos pets do NA** (junto com as demais classes): linhas novas de `PetInfo`/`PetStatusBalanceInfo`/`PetSkillInfo` e os itens `pet_create_*`, modelos e ícones, com `PetMaxLevel` revisto (o NA vai a 50). Os 36 tipos atuais não mudam.

## 5. Implementação (30/10/2026)

Pedido do operador ("implemente completamente o sistema de pets"), antes da ordem do B5. Cliente: **necessário** (roteiro `docs/client-tests/B5-pets.md`).

### 5.1 O que existe

| Parte | Onde | Origem |
|---|---|---|
| Conteúdo (`PetInfo`, `PetStatusBalanceInfo`, `PetSkillInfo`, `DefaultPetSkillInfo`, `PetActiveTimePeriodInfo`, `PetFeedInfo`, `ExtraData` dos itens `pet_*` e os valores `PetPurchaseLimit`, `PetMaxLevel`, `Pet_ExpBattle_Point`, `NamingRuleMaxBytes`) | `scripts/content/export_item_economy.py` → seção `pets` do `item-economy.json`; `domain::pet::Contents` | `ItemContents` (pet dictionaries) |
| Esquema | `migrations/0050_pets.sql`: `game_pets`, `game_pet_skills`, `game_pet_feeds` (UserSharedItem tipo 2), `game_pet_ledger` | `heroes.Pet/PetSkill`, `heroesLog.PetLedger` |
| Fio | `src/protocol/pet.rs` (20 mensagens, `PetStatusInfo` com `DateTime` = `dateData`), `CharacterSummary.Pet` e `GameJoinMemberInfo.Pet` não nulos | *fixtures* `tests/fixtures/framework-pet-wire.json` (195 casos, x86 = x64) de `scripts/reverse/generate-pet-wire-fixtures.ps1` |
| Regras | `src/domain/pet.rs`: nome, *slot*, EXP/nível, fome, tempo ativo, status, período, ração automática, skills de cash, validade pelo nome do item | `PetManager`, `UseInventoryItemProcessor.GetTimeSpanInfoByItemClass` |
| Operações | `src/infrastructure/minimum_pet.rs`: carga na seleção, lista, checagem de nome, criar, renomear, invocar/recolher, mover *slot*, apagar, remover skill, ração V1 e V2, fim de batalha, reinício por período (temporizador do frontend) | processadores de `PlayerService.ItemServiceCore.PetSystem` |
| Missão | `BattlePets` (MicroPlayerPetManager) no `Mission`: pet atual no `GameJoinMemberInfo`, contagem de tempo, EXP de monstros × `Pet_ExpBattle_Point`/100, `PetKilled`/`PetRevived`, `EndBattle` nas recompensas (com EXP) e na recompensa nula/saída (sem EXP), `UpdateCurrentPetMessage` quando o pet muda em jogo e `MemberInfo` no navio | `MicroPlayer`, `MicroPlay.IsPetUsable` |
| Aparência | o pet atual entra no `CharacterSummary` da seleção e da cidade (`NotifyLook`) e é atualizado a cada mudança (`PushUpdatePetStatus`) | `CharacterEntity.MakeSummary` |
| História | condição `has_pet` | `Cond_has_pet` → `PetManager.HasPet` |
| Painel | aba Pets na ficha; `GET/POST /api/characters/{cid}/pets`, `POST …/pets/{pet}/remove`, `GET /api/pets/types` (auditoria e `operation_id`, recusa online) | GM `_CHEAT_AddPet`/`RemovePet` |

`IsEnable("Pet")`, `IsEnable("Pet_food_V2")` e as *features* de região do `PetActiveTimePeriodInfo` são avaliados pela política das NPCs (`assets.npc_policy()`), que tem os pares de versão medidos. O conteúdo de pets é montado no primeiro uso (`Assets::pets`). No 1.69 EN-EU: 36 tipos, 32 com período diário (31 `enEU` às 07:00 UTC + o tipo 659 sem *feature*), `PetPurchaseLimit` 100, `PetMaxLevel` 30, `Pet_ExpBattle_Point` 20.

### 5.2 Desvios do original (todos com o mesmo efeito visível ou em caminhos sem uso)

- **Item usado na falha:** o original destrói o item e devolve (ou manda por correio) quando a operação de pet falha; aqui a operação é checada antes e o item só é consumido no sucesso. O inventário e o diálogo ficam iguais.
- **Ração compartilhada com inventário cheio:** o original manda o item por correio e libera a ração; o correio não está migrado, então a ração continua compartilhada e o diálogo `GameUI_Heroes_Pet_Fail_Food_Unshare` é enviado.
- **`UpdatePetStatus`:** o original marca "vencido" com `ExpiredTime > agora` (invertido). Nenhum caminho do 1.69 grava `ExpiredTime` no pet; aqui o vencimento vale quando o prazo passou.
- **Nome proibido:** não é checado, como no nome de personagem (nome livre, decisão do operador de 26/10/2026). A regra `NamingRule` (EN-EU, 4+ letras) e `NamingRuleMaxBytes` valem.
- **`PetOperationMessage`:** exige `IsCheat` (sessão de GM), que o laboratório não tem; é ignorada. O painel faz criar e apagar.
- **Acessórios:** sem tabela e sempre lista vazia (`EquipPetAccessory` não tem chamador no 1.69).
- **Raide gigante:** não migrada; `IsPetUsable` usa só `IsHuntingQuest`.
- **`PetTransSelect`/"SetPetName"** (troca forçada de nome na seleção, para pets transferidos): não há transferência de pets, não foi portado.

Copiados do original: `SelectPet` aceita pet desativado; `RemainExpiredTime` é sempre `UtcNow`; `GiveSkill` (GM) não foi exposto.

### 5.3 Verificação

- `cargo test --lib domain::pet` (7), `--test pet_wire` (195 casos com truncamentos e campos), `--test character_list_wire --test minimum_map --test mission_wire --test item_sync_wire --test admin_http`.
- PostgreSQL: `minimum_pets_create_select_battle_slot_and_remove_per_account` (criar, nome, invocar, batalha com EXP limitada, fome e ração automática, *slot*, apagar, *ledger*, painel com recusa online, repetição e nome repetido) e regressões `minimum_mission minimum_use minimum_hot_spring minimum_in_game migration minimum_story character_list lobby panel_item admin`.
- `scripts/content/test_export_item_economy.py`; `npm run build` do painel.

### 5.4 Pendências de cliente

- **Índice 28 do pet:** o pet aparece na cidade e na fase? (Seção 2.1: o DLL cria a entidade; o despacho do patch de classes compara por igualdade com as classes extras, então o pet não cai nos casos da Delia, mas não há teste.)
- Janela de pets, ração compartilhada, renomear, skills de cash e mensagens (`GameUI_Heroes_Pet_*`).
- Multijogador (B5): o pet de outro jogador na cidade e na missão.
