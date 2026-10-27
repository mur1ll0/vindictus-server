# Caminho incremental para o jogo funcionar

Plano revisado pelo operador em 21/10/2026. A ordem prioriza o ciclo PvE jogável de uma Fiona solo. Sistemas que dependem de fases vêm depois delas. A administração e a análise de extensões (novas classes) vêm antes do multijogador. A migração integral M01–M24 do `MIGRATION.md` continua sendo o objetivo final, e este roteiro define a ordem de trabalho.

## Como os blocos funcionam

- O trabalho é organizado em **blocos grandes**. Cada bloco é uma sequência de etapas pequenas (fatias cliente → armazenamento).
- **Dentro do bloco:** a cada etapa rodam só testes focados (alvos Rust afetados e testes PostgreSQL filtrados), seguidos de um commit local. A etapa seguinte começa sem pedir confirmação ao operador. Toda etapa com efeito visível no cliente acrescenta uma entrada ao roteiro de testes do bloco; uma etapa sem efeito visível registra `cliente: não necessário` no `MIGRATION.md`.
- **No fim do bloco (gate):**
  1. Reiniciar o PostgreSQL de teste se o WAL estiver alto.
  2. Rodar a suíte completa `scripts/dev/verify.ps1 -DatabaseIntegration` e o smoke Docker.
  3. Finalizar o roteiro de testes do bloco (ver abaixo).
  4. Fazer o commit de fechamento do bloco.
  5. Parar para a sessão de gameplay do operador, que segue o roteiro do bloco.

## Roteiro de testes de cada bloco

- Cada bloco tem um arquivo próprio, `docs/client-tests/<bloco>.md` (exemplo: `docs/client-tests/B1-cidade.md`). O arquivo é criado na primeira etapa do bloco com efeito visível no cliente.
- Cada etapa visível acrescenta uma entrada:

  ```
  ### <ID da etapa> — <funcionalidade>
  Pré-condição: <ouro/itens/quest/estado necessário; comando CLI de laboratório, se houver>
  - [ ] <ação no cliente> → <resultado esperado>
  - [ ] Relogar → <o que deve estar preservado>
  Limites conhecidos: <o que ainda é recusado de propósito>
  ```

- No gate, o roteiro ganha no topo:
  - a **preparação**: como iniciar o runtime (`scripts/dev/start-lab-frontend.ps1`), quais comandos de laboratório rodar e qual personagem usar;
  - uma **ordem de gameplay** que percorre todas as entradas numa sessão natural;
  - **como relatar**: ID da entrada, o que fez, o que apareceu na tela e o horário, para comparar com os logs em `.cache/verification`.
- Depois da sessão, as falhas relatadas viram correções no mesmo bloco. Quando o operador confirmar o bloco inteiro, o aceite é registrado no `MIGRATION.md` e o arquivo do roteiro é removido.
- Itens que só podem ser testados num bloco seguinte ficam listados naquele bloco deste roadmap, e não no roteiro atual.
- **Checkpoint de cliente no meio do bloco:** parar apenas quando a próxima etapa não puder ser verificada sem confirmar no cliente uma etapa anterior (exemplo: o primeiro carregamento de uma dungeon antes de implementar combate dentro dela).
- **Sincronização desde já:** toda mudança de estado visível a outros jogadores (look, posição, ações, spawn, HP, drops, estado de instância) passa por um ponto único de difusão/autoridade. Assim, o multijogador do bloco B5 não exige reescrita. Testes multijogador reais ficam para o B5.
- **Ferramentas de laboratório antes do painel:** conceder ouro ou itens para testar pode ser feito por comandos CLI pontuais, registrados no checklist. O painel completo é o bloco B4.

## Estado confirmado

- **B0 fechado.** Suíte completa em 2026-10-21T17:13:11Z com 406 Rust, 245 PostgreSQL (0 ignorados), 50 Python e zero falhas.
- **B0 aceito no cliente (21/10/2026):** criação/relogin da Fiona, seleção, cidade, movimento, inventário, equipar/desequipar, mover itens, quickslots de consumíveis, lista de skills/AP, cápsula de AP e treino de skill com AP.
- **Testes adiados:** aprender skill nova com ouro (B1.2), empilhar pilhas da mesma classe (B1.4 via loja ou B2.6 via loot), usar consumível pela barra (B2.5) e o "Depot" da loja Cash (B5).
- **B1 e B2 fechados no código, sem aceite de cliente.** B1 em 21/10 e B2 em 22/10/2026 (suíte completa verde). Nenhuma entrada dos dois roteiros foi testada no cliente ainda; a sessão combinada segue `docs/client-tests/B2-fases.md`. O transporte ProudNet (relay 27005) e toda a missão foram construídos sem checkpoint de cliente, por decisão do operador.
- **B3 fechado no código (24/10/2026), sem aceite de cliente.** Itens 0–4 implementados (atributos, Craft, Enhance, Enchant, slots, reparo completo, armazenamento pessoal e SkillEnhance); o item 5 foi adiado pelo operador. Gate verde (PostgreSQL em lotes por causa do tmpfs); roteiro em `docs/client-tests/B3-equipamento.md`.
- **B4 fechado no código (27/10/2026), aceite de cliente parcial.** Painel de administração (contas, personagens, itens, progressão, lojas, cash shop, ajustes ao vivo, conteúdo versionado, mapas e canais), nome livre e criação das 10 classes, relatório de extensões (`docs/extension-feasibility.md`) e contratos dos pontos de extensão (`docs/b4-painel-plano.md`, seção 4). Gate verde (PostgreSQL em lotes); roteiro em `docs/client-tests/B4-painel.md`, com o B4.2-L já aceito. Classes novas (Delia, Miri) investigadas em `docs/future/new-classes.md`: não cabem no cliente 1.69 sem o código delas.

## Pendências conhecidas do B2 (não implementadas)

Levantadas na auditoria de 22/10/2026 a partir do código e dos limites de cada entrada do roteiro; revisadas em 23/10/2026. O servidor recusa ou ignora esses casos sem enviar sucesso falso.

- **Entrada e grupo:** só missão solo (host local). Recusadas: pesca, caça, torre, missões com item obrigatório ou limite de equipamento. Moedas e pedras de bênção vêm vazias. Reinício pelo navio (ReturnToShip) não existe; o botão de reiniciar fica oculto. Reentrar numa missão em andamento (CombatRejoinSystem) não existe.
- **Setores:** sem AntiSectorSkip e sem diálogos de console de setor.
- **Drops:** baú do tesouro, mapas do tesouro, núcleos de evento/história, pontos fracos (WeakPoint) e HugeLucky. O `AbilityBonus` de props e props nomeados foi para o item 5 do B3. Itens com atributo (ex.: pergaminhos de encantamento) dependem do inventário com atributos (B3); inventário cheio é recusado e o envio por correio da sobra não existe.
- **Combate:** o anti-trapaça de dano (MDRSystem) só marca rankings, que não existem. Caixas alquímicas na barra continuam recusadas.
- **Conclusão e recompensas:** recompensas de troca de item (ex.: `slayer_stamp_limit>slayer_stamp`); rankings (CharacterRank); bônus de status, cafe, guilda, moedas e pedras de bênção; AddMicroPlayTime e contagem diária de quests grátis; OnEnterPhase das histórias após a quest. Um objetivo de quest (do sapo) usa uma regex que o motor não compila e nunca conclui.
- **Progressão:** AddGrowthLog, efeitos de guilda e títulos de rank. Destruir item equipado por história (`take_item` de peça vestida) continua recusado até o desequipar pelo ItemService.
- **Histórias:** ainda param o diálogo, sem gravar nada, a vocação (`select_vocation`), recompensas `enchant_scroll[...]` com atributo (B3) e `complete_episode`/`give_title_by_episode`.
- **Robustez:** envio por correio dos itens reservados de quem sai da missão.
- **Feitos depois da auditoria (22–23/10):** títulos adquiridos (status concluído, bônus de atributo, TitleListMessage na entrada, títulos por ação de combate), recompensa de título e de skill de história, props nomeados (`NamedPropDropTable`), itens com validade e valores originais de conteúdo (`docs/content-tuning.md`).
- **Gate:** o smoke Docker não completou no fechamento (carga externa); repetido na auditoria de 22/10 com exit 0, o que completa o gate do B2.

## B0 — personagem na cidade (fechado em 21/10/2026)

Inventário, equipamento, mover/empilhar, quickslots, AP/cápsula, lista, treino e atalhos de skill (P0-01 a P0-03d). Aceito no cliente em 21/10/2026.

## B1 — cidade: dinheiro, NPCs, lojas e quests

Objetivo: a Fiona vive a cidade inteira sem entrar em fases.

1. **Dinheiro do personagem:** carteira de ouro, persistência, logs/ledgers e mensagens de saldo. GiveItem geral (entrega de itens a partir de loja/recompensa, com empilhamento e falta de espaço).
2. **LearnNewSkill com ouro:** aprender as skills novas pagando o preço original.
3. **NPCs:** presença na cidade, interação, diálogos e navegação de menus. Estado mínimo das histórias exigidas no percurso inicial.
4. **Lojas de NPC:** catálogo da loja, compra, venda, recompra (se o original tiver) e reparo pago. Recusas por ouro/espaço/requisitos sem perder nem duplicar itens.
5. **Quests iniciais:** oferta, aceite, objetivos que não exigem combate (conversa/entrega), acompanhamento, conclusão, recompensas (ouro/itens/XP quando aplicável) e persistência no relogin. Objetivos de combate são implementados até o ponto de registro e concluídos no B2.
6. **Usáveis restantes alcançados no percurso:** caixas (RandomItemV2, por exemplo a caixa Level Up) e logs de personagem que as operações acima exigirem.

Gate B1: conversar com os NPCs, comprar, vender, reparar, aprender skill com ouro, aceitar e concluir uma quest de cidade, e relogar mantendo ouro, itens e estado das quests.

## B2 — fases e combate (grande sessão de testes)

Objetivo: o ciclo PvE completo, cidade → quest → fase → combate → recompensa → NPC → relogin.

1. **Saída para a fase:** quadro/barco, lobby de missão, seleção de fase/dificuldade, requisitos, criação/admissão da instância e retorno à cidade.
2. **Transporte de batalha (M19):** ProudNet/relay/host conforme a evidência recuperada. É a maior incerteza técnica do projeto e é investigada no início do bloco.
3. **Mapas das dungeons:** carregamento, spawn do jogador, setores e portais. *Checkpoint de cliente:* confirmar o primeiro carregamento antes de seguir.
4. **Monstros e boss:** spawn, AI, aggro, ações e hits.
5. **Combate:** dano, bloqueio/esquiva, HP/stamina/SP, consumíveis e skills em combate, cooldowns, morte/revive.
6. **Drops e loot:** coleta, ouro/itens e empilhamento (fecha a pendência do P0-02e).
7. **Progressão:** XP, level up, AP por nível, objetivos de quest de combate, conclusão/falha da missão e recompensas sem reaplicar em retry.
8. **Robustez:** logout/reconexão, queda durante a missão ou a recompensa, limites e operações atômicas.

Gate B2: repetir o ciclo completo várias vezes, inclusive morrer e desconectar, sem duplicar nem perder progresso e sem loading infinito.

## B3 — progressão de equipamento: Craft, Enhance e Enchant

Depende de fases, drops e chefes do B2.

0. **Atributos de item (base dos itens 1–5):** carga, projeção no SlotInfo, empilhamento, entrega e gravação de `game_item_attributes`. B3.0a (23/10/2026): itens não equipáveis (pergaminhos `enchant_scroll[PREFIX/SUFFIX:…]`) em entrega, loja/recompra, história, missão e quest. Pendente: equipamento com atributos (ItemStat de Enhance/Enchant/Quality), junto com os itens 2 e 3.
1. **Craft:** NPC/ofício, receitas desbloqueadas por chefes e fases conforme a progressão, materiais, custos, fabricação/filas e resultado persistido.
2. **Enhance (ferreiro):** níveis de melhoria, chances de sucesso e falha, quebra e restauração de itens, itens que aumentam a chance e itens que impedem a quebra.
3. **Enchant:** scrolls de prefixo e sufixo, chances, falha e destruição conforme o original, e itens de proteção.
4. **Complementos:** armazenamento pessoal, expansão de slots, reparo completo e SkillEnhance.
5. **Combine Craft e habilidades de item:** atributos `PS_<n>` das peças de Combine Craft (`Gear.ToAbilityInfo` → `MicroPlayer.AbilityList`) e, junto com eles, o `AbilityBonus` dos drops de prop (`PropDropTable.AbilityBonus`) e de prop nomeado (`NamedPropDropTable.AbilityBonus`, 81 habilidades `AbilityClassInfo` com Event 2): chance `Arg2`%, título `Arg3`, `TargetQuestGroupID`, regra de fadiga/desistência e o anúncio `SystemMessage` "GameUI_Heroes_ItemAbility_Enable_Announce". Pendente desde 23/10/2026, porque o inventário ainda recusa itens com atributos e nenhum personagem tem habilidade.

Gate B3: fabricar, melhorar e encantar com sucesso e com falha (inclusive quebra e restauração), consumindo recursos uma única vez, e relogar com os resultados.

## B4 — painel de administração e análise de extensões

1. **Painel de administrador** inspirado no projeto `pw-universal-server` do operador (`F:\Python_C_Projects\PWSource1.5.3\pw-universal-server`), sobre o HTTP operacional, com autenticação e auditoria. Funções previstas: criar/gerenciar contas, gerenciar personagens (ouro, itens, nível, skills), recursos do jogo, edição de lojas e mapas/canais, se houver controle necessário. O painel nunca reimplementa o login do jogo. Os valores de jogo ajustáveis (drops por fase, contagens de título, nível mínimo, EXP de história, pilha máxima, taxas do FeatureMatrix) estão mapeados por tabela/coluna em `docs/content-tuning.md`; o painel deve editar esses pontos.
2. **Análise de viabilidade das extensões:** novos equipamentos e itens, novas missões, novas fases e novos monstros. O objetivo principal é **novas classes**. Para cada tipo, levantar o que vive no conteúdo SQLite, o que vive nos assets do cliente e o que exige servidor. O resultado é um relatório de viabilidade, que precede qualquer implementação de extensão.

Gate B4: suíte completa e uso real do painel para criar uma conta, ajustar a Fiona e editar uma loja, com o efeito confirmado no cliente.

## B5 — demais recursos e multijogador

- **Multijogador:** party, convites, instância/combate/loot sincronizados, host/servidor, saída e reconexão de membros. Teste com vários clientes reais pela primeira vez neste bloco.
- **Sistemas restantes:** PvP/arena/ranking, guildas, trade, correio completo, eventos/UserCare, pets, títulos/achievements, leilão/mercado, cash shop e cosméticos.
- **Paridade e aceite final:** M23/M24 do `MIGRATION.md`.

## Recursos futuros do painel

Planejados em 26/10/2026, só como documentação. Não mudam a ordem dos blocos B0–B5. Cada um alimenta o relatório de viabilidade do B4.7 e traz a própria evidência e as próprias hipóteses.

- [Editor de itens](future/item-editor.md): criar, clonar, editar e remover itens (equipamento, roupa e consumível; ícone e modelo 3D com Blender 5.1). Primeiro objetivo: inner armors novos para todas as classes. Vem **depois do B4.5** (taxas do servidor). A etapa I1/I2 usa só conteúdo; ícones, textos e modelos dependem do launcher.
- [Launcher do jogo](future/game-launcher.md): baixar, atualizar só o que mudou (manifesto assinado e objetos por hash, no modelo do pw-universal) e substituir o `start-lab.bat`. O teste L0 (arquivos soltos × `.hfs`) é um checkpoint de cliente que pode rodar a qualquer momento. L1–L3 vêm depois do B4 e antes de qualquer etapa que mude assets do cliente.
- [Editor de fases](future/stage-editor.md): quest, dificuldades, setores, spawns, boss e drops pelo mesmo ciclo de edições e manutenção do B4.4c. Vem depois do editor de itens (os drops referenciam itens). Fases com BSP novo ficam como estudo à parte.

## Regras de cada etapa

- Usar fonte recuperada e fixtures/oracles para fio/comportamento; primeiro verificar o que já existe no domínio/banco.
- Entregar uma fatia pequena do cliente ao armazenamento, com recusas/persistência. Não enviar sucesso fictício ou listas vazias para ocultar serviço obrigatório ausente.
- Logs em `.cache/verification`, arquivos editados em LF; uso de múltiplos núcleos autorizado.
- Preservar Fiona/marcador/schema e diretórios originais. Validar no cliente real antes de declarar compatibilidade.
- Commits locais por etapa verificada, sem push automático. Registrar novas lacunas aqui e no checkpoint do `MIGRATION.md`.

## Compilação e GitHub

O workflow `.github/workflows/verify.yaml`, que executava build/testes em todo push e pull request, foi removido a pedido do operador. A verificação local `scripts/dev/verify.ps1` permanece. A remoção afeta o GitHub quando o commit for enviado ao remoto; um commit local não dispara GitHub Actions. A causa das falhas históricas do workflow não foi diagnosticada neste lote.

## Histórico do B0

- P0-01: contrato do inventário inicial em [minimum-inventory-client.md](minimum-inventory-client.md).
- P0-02a–c: slots, loader de equipamento vestido e EquipItem457 em [minimum-equipment-client.md](minimum-equipment-client.md).
- P0-02d–f e P0-03a–d: detalhes e evidências no `MIGRATION.md`.
