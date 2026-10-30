# Create mínimo do laboratório

O operador priorizou criar Fiona e entrar no mapa inicial, adiando correio, pets e serviços acessórios. Esta implementação oferece um caminho nativo explícito; não representa a composição integral dos atores legados nem conclui a migração M01–M24.

`lab-frontend --minimum-creation-assets <configuração privada>` habilita Create. Sem essa opção, a criação continua indisponível. O launcher recebe `-MinimumCreationAssetsFile`; exige o arquivo preparado sob `.cache/private-content`, ACL restrita e prova de nomes. `inspect-minimum-creation-assets --assets-file <arquivo>` valida os vínculos dos pacotes sem abrir banco ou listener e sem imprimir conteúdo privado.

O perfil admite somente o primeiro personagem Fiona normal, nível inicial, sem preset/premium, pertencente à conta sintética autenticada e à revisão atual. A segunda senha precisa estar liberada. O nome e o template vêm da mensagem recuperada CreateCharacterMessage, categoria 482, inclusive a forma com GUID na categoria zero. CID, itens e habilidades usam a mesma autoridade de entidades do processo.

Após a base e os efeitos Dormant já recuperados, o caminho compõe missão/base stats/SP, entrega inicial de inventário, aparência e habilidades com seus callbacks detached. Registra o último CID e confirma as quantidades gravadas. A migration 33 mantém um marcador distinto de inicialização mínima, vinculado ao dono e ao fingerprint do perfil. Só após o readback completo ele se torna inicializado. Um marcador parcial nunca produz lista nem permite replay automático.

O worker acompanha cancelamento e shutdown. Cancela e aguarda o inicializador antes de compensar a base comprometida; cascades removem itens, aparência, habilidades e marcador. Uma falha de compensação impede sucesso. Os códigos recuperados de falha de customização, salvamento de aparência e inicialização são conservados. Os logs registram apenas a fase e o código, sem nome, template, senha ou conteúdo privado. Identidades e journals independentes não são rebobinados.

A resposta de sucesso é o CharacterListMessage 479 já recuperado. A mesma consulta é usada no relogin: dono/revisão, procedência sintética, marcador completo, perfil, Costume e ausência validada de Vocation. Usa os campos persistidos e o XP do conteúdo; calcula slots/prologue com os providers existentes. Não transforma personagem legado ou parcial em lista vazia. O codec não mudou: reutiliza as fixtures originais de 273 casos por arquitetura. O bloco de espera de exclusão de `Character.MakeSummary` tem oracle adicional de 96 casos Framework x86/x64, com conteúdo e UTC injetados somente nas fronteiras.

Guild, pets, café/retorno/VIP, eventos, arena e outros serviços acessórios ficam sem ator nesse perfil. A lista usa o estado inicial explicitamente delimitado, incluindo Pet nulo e BriefGuildInfo vazio; não é uma projeção geral de personagens existentes. Grants de títulos, histórias/quests e progressão completa continuam fora deste caminho mínimo. O seed de cores é explícito de laboratório; não reproduz a agenda/TickCount do processo original. A aritmética corporal fica em `unambiguous-only` até existir evidência para selecionar uma arquitetura nos casos ambíguos.

## Próximo caminho até o mapa

1. Executar Create e relogin no cliente 1.69 EN com o frontend atualizado. Testes TCP sintéticos não certificam aceitação pelo jogo.
2. Receber SelectCharacterMessage como índice da lista, validar ownership e segunda senha, carregar o personagem mínimo e enviar os dados recuperados de seleção. O legado retorna LoginOkMessage, seguido do estado InGame e dos dados de personagem/stats/habilidades exigidos pelo cliente.
3. Ligar EnterChannel à admissão de cidade. O legado entrega ChannelServerAddress com endereço, porta e chave e abre uma conexão separada com HeroesChannelServer. Recuperar e validar esse transporte/handshake antes de implementar a admissão; não substituir por sucesso no frontend.
4. Recuperar os pacotes HeroesChannelServer de entidade/partição/ActionSync, validar posição/movimento e persistir o estado mínimo necessário. Executar o cliente e confirmar entrada, caminhada e relogin.

As fontes de seleção são `FrontendServiceCore.States.CharacterList`, `CharacterSelecting`, `InGame`, `FrontendServiceCore.Proxy.MMOChannelProxy`, `SelectCharacterProcessor`, `MMOChannelService.Processor.JoinChannelProcessor` e `HeroesChannelServer.Server`. `SelectCharacterMessage.Index` indexa a lista, não o CharacterSN. `LoginOk.RegionCode` converte o valor 1 em 0. `EnterChannel` carrega ChannelID, PartitionID e ActionSync. Esses achados são estáticos; nenhum deles certifica execução nativa de seleção/cidade.

Verificação completa e ativação são registradas em docs/history/session-log.md, no relatório corrente e em `docs/evidence/auth/minimum-character-create.json`. Logs completos permanecem em `.cache/verification`; somente resumos devem ser coletados no terminal. Originais e SQL Server continuam somente leitura.

## Estado atual da etapa M15.1c.2 (Seleção e Cidade)

### Concluído e verificado na árvore de trabalho
- **Oracle diferencial `.NET Framework 4.8` (`x86` e `x64`)**: `scripts/reverse/framework/MinimumMapWireOracle.cs`, `scripts/reverse/generate-minimum-map-wire-fixtures.ps1` e `tests/fixtures/framework-minimum-map-wire.json` (`189` vetores de serialização/desserialização para as 14 mensagens + framing sem cifra `MessageAnalyzer`, `7` vetores `DateTime.ToInt32()`, `8` arquivos `.dat` de mapa, `400` vizinhanças de partição e `8` passos de `Channel` com múltiplos jogadores; `0` divergências `x86`/`x64`).
- **Protocolo (`src/protocol/minimum_map.rs`, `src/protocol/transport.rs`, `src/protocol/character_list.rs`)**: codecs completos para `NoticeGameEnvironment` (`345`), `Identify` (`363`), `EnterChannel` (`364`), `ChannelServerAddress` (`365`), `IdentifyFailed` (`366`), `MovePartition` (`368`), `UpdateAction` (`369`), `Disappeared` (`372`), `NotifyLook` (`373`), `NotifyAction` (`374`), `SelectCharacter` (`481`), `LoginOk` (`483`), `CharacterCommonInfo` (`609`) e `UpdateStat` (`619`), além de `PlainReceiver` (framing TCP sem cifra do `HeroesChannelServer`).
- **Domínio (`src/domain/map_partition.rs`, `src/domain/skill_runtime.rs`, `src/domain/character_stats.rs`)**: `utility_epoch_seconds`, `login_ok_region_code`, `recommend_channel`, parser binário `MapFile::parse`, grafo `Topology::from_maps`, máquina de estado `Channel` (reproduzindo a ordem de slots LIFO do `Dictionary<Key, IComponent>` do `.NET Framework`), `Cache::load_base` (`SkillManager.LoadBaseSkillRank()`) e `stat_dictionary` (`StatManager.GetDictionary()`).
- **Testes diferenciais (`tests/minimum_map.rs`)**: `2/2` testes aprovados (`cargo test --test minimum_map --locked`).

### Implementado e verificado neste lote
- **Seleção (`minimum_selection.rs`)**: índice zero da lista, admissão pela segunda senha no frontend e validação transacional do dono/revisão/origem, marcador completo/perfil, vocação inicial, ausência de títulos, `dirty=false`, stats base e skills exatas. Calcula Gear inicial, cache de skills, carga e stats; envia `483 → 345 → 609 → 619`, guarda CID/NotifyLook e persiste last-login/conectividade/limpeza da quest. Falha tardia reverte as escritas.
- **Canal (`channel_server.rs`)**: um canal de laboratório em TCP loopback sem cifra; EnterChannel responde endereço/chave descartável; Identify obrigatório primeiro, chave consumida inclusive quando errada. Encaminha partição/ação ao domínio e difunde Look/Action/Disappeared; desconexão remove membership, limpa channel e revoga chave pendente. Frontend desconectado limpa também `is_connected`.
- **Integração**: `minimum_assets.rs`, `lab_frontend.rs` e `main.rs`; `map_partition_dir` e `channel_bind` opcionais, exigidos juntos. Os oito `.dat` ficam somente no cache privado e conferem com os hashes do oracle. Configuração sem canal preserva o caminho anterior de Create/lobby.
- **Testes novos PostgreSQL + TCP**: seleção/relogin/segunda senha, ordem e conteúdo das respostas, quest preenchida→NULL, Identify fragmentado/primeiro/chave errada/replay, movimento, desconexão, revogação pendente, índice inválido/CharacterSN, dono obsoleto, marcador parcial, stats/skills/títulos incompatíveis e rollback tardio.

### Verificação e ativação
- `cargo test --test minimum_map --locked`: **2/2 PASS**. Novos testes PostgreSQL/TCP (`minimum_selection`): **2/2 PASS**.
- Gate completo único `scripts/dev/verify.ps1 -DatabaseIntegration`, em `2026-10-20T12:10:16.5885992+00:00`: **378 Rust, 235 PostgreSQL (zero ignorados), 44 Python; zero falhas/avisos**, formatação, clippy, doc-tests e build aprovados. PostgreSQL descartável próprio em tmpfs, durabilidade on/on/on, removido após o gate.
- Serviços reiniciados e listeners loopback conferidos: HTTP **8081** (resposta 200), frontend **27015**, canal sem cifra **27003**. Binário privado confere com o build validado.
- Fiona do operador preservada antes/depois: **schema33, 1 marcador completo, 23 costumes, 2 skills, 11 itens**. Nenhuma migration histórica, diretório legado ou banco `study_*` alterado.
- Evidência sanitizada: [minimum-character-selection.json](evidence/auth/minimum-character-selection.json); logs completos em `.cache/verification/`.

### Correção após o primeiro teste de login
- O operador relatou `EXCEPTION_ACCESS_VIOLATION` no login, antes da seleção. Dois minidumps concordam em leitura nula em `client.dll+0x520c87` (`mov ecx,[eax+ebp*4]`, EAX=EBP=0).
- Foi identificado e corrigido um contrato inválido em `CharacterListMessage 479`: havia **1 personagem e 0 LoginPartyState**. O legado adiciona um estado por personagem, inclusive com CombatRejoinSystem desligado; a Fiona mínima agora recebe **[0]** (`None`). O vínculo do acesso nativo com esse campo é inferido; o defeito do pacote é comprovado pelo código recuperado e pelas regressões.
- Regressão antes da correção: **0 aprovados/2 falhas esperadas**, `Some([]) != Some([0])`. Depois: **8 testes PostgreSQL/TCP e 2 de mapa aprovados**, incluindo a cardinalidade no Create e no relogin.
- Gate único deste lote de correção em `2026-10-20T12:10:16.5885992+00:00`: **378 Rust, 235 PostgreSQL, 44 Python; zero falhas/avisos**. Frontend/canal reiniciados, três portas loopback conferidas e Fiona preservada; cliente, migrations históricas e bancos `study_*` não alterados. Novo teste real solicitado; sem commit até o aceite.
- Evidência: [minimum-login-crash.json](evidence/auth/minimum-login-crash.json). Dumps e memória privados permanecem fora do Git.

### Correção da desconexão na seleção após login confirmado
- O operador confirmou que o login passou após a correção de LoginPartyState; a tentativa de selecionar a Fiona desconectou antes de entrar no canal.
- Diagnóstico somente leitura: stats base, ordem/ranks das 2 skills, marcador completo, `dirty=false`, `is_connected=false` e ausência de títulos conferem. A seleção reaproveitava indevidamente a política de arredondamento da aparência (`unambiguous-only`) e recusava stats distintos entre x86/x64.
- Caso comprovado: DEX=18 produz DEF=8 no Framework x86 e DEF=9 no x64. `minimum_selection::select` agora exige `Arithmetic` explícito, fornecido pela arquitetura já declarada no frontend (x64 no perfil ativo). A política de corpo continua independente; nenhum atributo persistido foi alterado.
- Regressão TCP com base `[29,18,18,43,901,140]`: **falha antes por EOF**, **passa após**, com DEF=9 e seleção→canal→movimento. Pós-correção: **8 PostgreSQL/TCP + 2 mapa**; gate único deste lote em `2026-10-20T12:36:00.1885674+00:00`: **378 Rust/235 PostgreSQL/44 Python, zero falhas/avisos**.
- Frontend/canal corrigidos reiniciados e três portas loopback verificadas. Fiona preservada (1 inicializado/23 costumes/2 skills/11 itens); PostgreSQL descartável removido. Novo teste de cidade/movimento solicitado, sem commit até confirmação.
- Evidência: [minimum-selection-arithmetic.json](evidence/auth/minimum-selection-arithmetic.json).

### Correção da sala de evento após seleção confirmada
- O operador confirmou seleção/carregamento após a correção x64, mas ficou preso em fundo vermelho. O log real registra `lobby_event` às 09:41:55; não houve EnterChannel observado. Região persistida 0, MapStateInfo 0 e prólogo já concluído, verificados somente leitura.
- Defeito comprovado no LoginOk: enviava UserCareType=0 (`CareUser`) e UserCareNextState=0 (`Promise`). O construtor original UserInfo e os dois enums usam **-1/-1 (`Nothing`)** para inativo. O legado associa o estado 0 à história Event_usercare_kokr_2013, fase A. Corrigidos os dois campos, sem habilitar NPC/recompensa/evento. O ramo nativo do cliente não foi decompilado; aceite de cidade/movimento continua pendente.
- Regressão TCP antes: **1 aprovado/1 falha esperada**, recebido 0 vs esperado -1. Após: **8 PostgreSQL/TCP + 2 mapa aprovados**, verificando ambos os campos inativos e fluxo até movimento/desconexão.
- Gate único deste lote em `2026-10-20T13:04:23.0495014+00:00`: **378 Rust/235 PostgreSQL/44 Python, zero falhas/avisos**. Frontend/canal reiniciados e 8081/27015/27003 loopback verificados; HTTP200. Fiona preservada (schema33/1 inicializado/23 costumes/2 skills/11 itens). Base descartável removida; commit local aguarda aceite real.
- Evidência: [minimum-inactive-user-care.json](evidence/auth/minimum-inactive-user-care.json).

### Aceite de seleção/cidade/movimento e próximo incremento
- Operador confirmou em 20/10/2026: **“Sim funcionou”** e autorizou commit local de todo o lote. Seleção, entrada na cidade e movimento aceitos após os flags UserCare -1/-1.
- Escopo do aceite: Fiona mínima no cliente 1.69 EN. Inventário continua em **Downloading data** e o personagem foi observado sem equipamento visível. Esses pontos não estão aceitos; não reenviar defaults para contornar a ausência de sincronização.
- Próximo: **P0-01 / M15.1c.3 — leitura/sincronização de inventário e equipamento inicial**, seguido das mutações básicas e SkillList/AP/SP/quickslots. A criação já preserva 11 itens e 2 skills; verificar a integração antes de alterar os dados.
- Ordem restante e critérios de aceite: [roteiro PvE](playable-game-roadmap.md). NPCs/quests/fases/combate/recompensas precedem craft e expansão; PvP/guildas/eventos ficam por último. A migração M01–M24 permanece em curso.
- Workflow automático de build/teste no GitHub removido a pedido do operador. O gate local permanece; nenhum push automático.
