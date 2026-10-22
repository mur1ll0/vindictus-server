# Roteiro de testes no cliente — B2 fases e combate

Bloco B2 (EXP/level up, saída para a fase, transporte de batalha, dungeons, monstros, combate, drops, progressão e robustez). Por decisão do operador, a sessão de cliente acontece só no fim do bloco e começa pelo roteiro do B1 (docs/client-tests/B1-cidade.md). A preparação, a ordem de gameplay e o formato de relato são completados no gate.

### B2.0 — EXP, level up e títulos por nível
Pré-condição: perfil `npc_scripts` reexportado com as tabelas de progressão (CharacterClassInfo, LevelUpExpInfo, TitleInfo/TitleGoalInfo). A Fiona ganha EXP no B1 só concluindo uma história com recompensa de EXP; no B2 o EXP vem principalmente das missões (B2.7).
- [ ] Concluir uma história com recompensa de EXP/AP (ex.: uma das histórias "Goodbye"/cut-scene de nível 1, se o NPC oferecer) → a barra de EXP sobe; o AP aumenta (até o máximo).
- [ ] Quando o EXP passa do necessário → efeito de level up na tela, novo nível na janela de personagem, atributos maiores (STR/DEX/INT/WILL/HP) e a lista de títulos ganha os títulos de nível (em progresso, não adquiridos).
- [ ] Após o level up → a lista de quests recebe as quests automáticas do novo nível e histórias de nível novas aparecem no diário (quando existirem).
- [ ] Relogar → nível, EXP, atributos e títulos preservados; a seleção de personagem mostra o novo nível.
Limites conhecidos: títulos adquiridos (com status de concluído e bônus de atributo), recompensa de título/skill de história, log de crescimento (AddGrowthLog), ranking (CharacterRank) e efeitos de guilda não migrados.

### B2.1 — Saída para a missão (navio, relay e início da fase)
Pré-condições: perfis privados reexportados (`npc_scripts` com a seção `mission`, economia formato 4) e `relay_bind` = `127.0.0.1:27005` no arquivo de assets do laboratório (porta do relay original); servidor iniciado pelo `start-lab-frontend.ps1`, que agora também abre o relay. Fiona selecionada que já recebeu a quest pelo login (ex.: `quest_gnoll_scout`/`quest_gnoll_watchman` da Colhen).
- [ ] No quadro de missões/barco de Colhen, escolher uma missão inicial em modo solo e confirmar (AcceptQuest) → a interface de grupo/navio aparece com a Fiona como líder (slot 1) e o nome da missão no navio.
- [ ] Em seguida o cliente conecta ao relay (porta 27005) sem erro de rede e mostra a tela do navio/preparação (LaunchShipGranted). No log do servidor: "mission accepted; relay waiting" e depois "mission relay join finished" com `joined=true`.
- [ ] Iniciar a missão (StartGame) → tela de carregamento da dungeon e entrada no primeiro setor (GameStartGranted). No log: "mission start processed" com `granted=true`.
- [ ] Missão com dificuldade/condição inválida (ex.: nível insuficiente para a dificuldade) → diálogo de erro "PartyInMicroPlay_AcceptQuest_…", sem navio.
- [ ] Sair do jogo dentro da missão e relogar → personagem volta à cidade; nada do inventário foi perdido.
Limites conhecidos: só solo (host local); sem drops (monstros não deixam itens/núcleos), baú do tesouro e mapa; missões de pesca, caça, torre, com item obrigatório ou com limite de equipamento são recusadas; moedas/pedras de bênção aparecem vazias; combate, conclusão e recompensas vêm nos passos seguintes (B2.3+).

### B2.3 — Carregamento da dungeon e troca de setor
Pré-condição: a entrada B2.1 chegou à primeira área da dungeon.
- [ ] Ao terminar o carregamento, a Fiona aparece na área inicial com os monstros do setor (são criados pelo host local a partir das entidades enviadas). No log: "mission all users joined" e "mission sector move processed" com `moved=true`.
- [ ] Atravessar a saída da área (portal/porta de fim do setor) → carrega o setor seguinte (ex.: a sala do chefe); no log, outro "mission sector move processed" com `moved=true`.
- [ ] Hora do dia na dungeon (iluminação) coerente entre os setores; nenhuma desconexão ao trocar de setor.
Limites conhecidos: sem detecção de pulo de setor (AntiSectorSkip desligado no perfil), sem diálogos de console de setor; o progresso de objetivos e a conclusão vêm no B2.7.

### B2.4 — Abates de monstros e morte da personagem
Pré-condição: dentro de um setor da missão (B2.3).
- [ ] Derrotar monstros comuns → morrem normalmente no cliente; no log, "mission monster kill processed" com `killed=true` uma vez por monstro (um segundo relato do mesmo monstro aparece com `killed=false`).
- [ ] Derrotar o chefe da missão → morte do chefe sem desconexão; o log registra o abate (`killed=true`).
- [ ] Deixar a Fiona morrer → tela de morte; no log, "mission player death processed" com `dead=true`.
Limites conhecidos: nenhum item, núcleo ou ouro cai dos monstros (drops no B2.5); o EXP de monstros é acumulado no servidor e só aparece com o resultado da missão (B2.7); reviver com pena/consumível não está migrado.

### B2.5a — Núcleos dos monstros (drops por chute)
Pré-condição: missão iniciada (B2.1) fora do modo prática.
- [ ] Ao derrotar monstros, alguns deixam um núcleo brilhante (os escolhidos pela tabela de drop da quest); o chefe deixa vários.
- [ ] Chutar o núcleo → mensagem de item obtido com o nome da Fiona e o item aparece no inventário (ex.: materiais como couro/tecido). No log: "mission core looted".
- [ ] Chutar de novo um núcleo já vazio → nada acontece (log "mission core kick without loot").
- [ ] Relogar depois da missão → os itens obtidos continuam no inventário.
Limites conhecidos: itens com atributos (ex.: pergaminhos de encantamento) e inventário cheio (envio por correio) ainda são recusados, com o log "mission core loot refused"; mapas do tesouro, núcleos de evento/história, pontos fracos e HugeLucky não estão migrados.

### B2.5b — Ouro e ergs dos objetos quebráveis
Pré-condição: dentro de um setor (fora do modo prática).
- [ ] Quebrar barris/caixas/vasos → alguns soltam ouro e outros cristais/itens (ergs); objetos sem drop só quebram. No log: "mission sector props registered" ao entrar no setor.
- [ ] Recolher o ouro/erg → mensagem de item obtido e o ouro/item entra no inventário (log "mission erg picked"); o mesmo erg não pode ser recolhido duas vezes.
- [ ] Ergs de cura instantânea (HP) curam na hora sem entrar no inventário.
Limites conhecidos: props nomeados (NamedProp), bônus de habilidade em props e HugeLucky não migrados; inventário cheio ainda não manda por correio.

### B2.6 — Consumíveis, reviver e relatórios de combate
Pré-condição: Fiona com poções na barra de atalhos (comprar no B1 e equipar na barra) e, se houver, uma pena de fênix.
- [ ] Usar uma poção na missão → efeito no jogo e a quantidade no inventário diminui em 1 (UpdateInventoryInfo); log "mission consumable used".
- [ ] Usar mais vezes do que o levado para a missão → a barra para de aceitar (log "mission consumable use refused").
- [ ] Morrer e reviver com pena de fênix (se disponível) → a Fiona revive com invencibilidade breve; log "mission revive processed" com `alive=true`.
- [ ] Ao fim do combate o cliente envia o registro de combate (sem efeito visível agora; aparece no resultado no B2.7).
Limites conhecidos: a anti-trapaça de dano (MDRSystem) só marca rankings, que não estão migrados; caixas alquímicas na barra continuam recusadas.

### B2.7 — Conclusão da quest, recompensas e volta à cidade
Pré-condição: quest "quest_gnoll_watchman" (primeira quest), dificuldade normal; anotar EXP, ouro e AP da Fiona antes de entrar.
- [ ] Ao derrotar o alvo principal (o chefe) → aparece o resultado do objetivo no canto (QuestTargetResult) e a cena de sucesso começa (QuestSuccessSceneStart). Log "mission client event processed" para os eventos do cliente.
- [ ] Cerca de 30 s depois (MicroPlayFinishTime) → a tela de resultado abre com EXP de monstros, bônus de dificuldade, ouro e AP do objetivo; os núcleos do chefe ainda não chutados entram no inventário antes (mensagens de item obtido). Log "mission jobs processed".
- [ ] Se o chefe morrer cedo demais para o tempo mínimo da quest → mensagem "muito rápido" e a tela de resultado espera o tempo restante.
- [ ] Na tela de resultado: o botão de reiniciar não aparece (reinício pelo navio não migrado); "continuar" mantém a Fiona no mapa.
- [ ] Clicar em voltar à cidade → a Fiona volta para a cidade (KickedMessage + ReturnFromQuest). Log "mission left; returning to town".
- [ ] Na cidade: EXP/nível, ouro e AP aumentaram conforme a tela de resultado; a quest aparece concluída no diário (progresso 100, clear count 1) e o tempo de conclusão aparece.
- [ ] Se a conclusão avançar uma história (ex.: diálogo seguinte da Ellis/guia) → o NPC muda de fala depois da volta (eventos QuestCleared das histórias).
- [ ] Morrer de propósito numa quest com juramento "não morrer" (se escolher um) → falha por violação, tela de resultado com falha e só a EXP de monstros.
- [ ] Ficar além do tempo limite → falha por tempo (TimeOver) 5 s depois da checagem por minuto.
Limites conhecidos: reinício pelo navio, núcleos/drops de evento, AddMicroPlayTime/contagem diária de quests grátis, rankings, bônus de status/cafe/guilda/moedas/pedras de bênção e o OnEnterPhase das histórias após a quest não estão migrados; histórias com setor de história mandam ao setor de história e de lá para a cidade.
