# Roteiro de testes no cliente — C4 Miri (classe 11)

Porte da Miri do cliente NA (`docs/future/miri-class-port.md`, skill `new-class-port`). Cada passo com efeito visível acrescenta uma entrada aqui; o teste da classe em si (seleção, cidade, arma, habilidades, mecânicas, itens, grade) vem quando a entidade e o roteamento estiverem aplicados. Entradas `MI*`; origem no `docs/history/session-log.md` pelo passo `M<n>`.

## Preparação
1. Servidor: `docker compose ps` com `game` e `server` saudáveis (imagem do commit da última entrada pendente).
2. Cliente: cópia `.cache/play-client` com os pacotes da Miri e os scripts mesclados aplicados por `scripts/client/apply_lab_content_additions.py --apply` (passo M1; resultado em `.cache/verification/miri/m1c-apply.log`). Jogo fechado durante a aplicação.
3. `.cache/play-client/start-lab.bat`, conta `lab169_a`.
4. Relatórios de queda: `Documents\Vindictus EU GM\Report Error` (nome do mais novo e horário).

## Ordem de gameplay (uma sessão)
1. **MI1:** não regressão das classes antigas com os pacotes e scripts da Miri.
2. **MI2:** painel de administração com a Miri (não precisa do jogo aberto).
3. **MI3:** itens da Delia com o `engine.dll` novo (M4).
4. **MI4:** Miri de laboratório na seleção, na cidade, com a arma e os itens (M5).

## Como relatar
ID e número da linha (ex.: `MI1-2`), o que apareceu (captura se possível), se travou ou fechou, horário e relatório de queda. Na primeira falha que bloqueia as seguintes: anotar, voltar ao estado seguro e encerrar.

### MI1 — Classes antigas sem regressão depois do M1
Pré-condição: preparação feita. Nenhuma personagem Miri existe ainda; o M1 só acrescentou pacotes novos (75, 4.331 arquivos) e linhas da Miri em 21 scripts compartilhados (`player_costume.txt`, sons de jogador e de armas, `action_table_player.txt`, `projectile.txt`, `item_data.txt`, `skill_actionstate_match_table.txt`, `translated_spskill.txt`, `bodyshape_fig_list.txt`, entre outros).
- [ ] Abrir o jogo e logar → a tela de seleção abre normalmente, no mesmo tempo de antes (anotar se a carga ficou bem mais lenta).
- [ ] Entrar na cidade com uma Fiona (ou outra classe antiga) e com a `DeliaLab` → corpo, roupa, cabelo e arma certos; andar, correr e um emote.
- [ ] Equipar e tirar um inner da Fiona e o `fiona_inner_zero`/`evy_inner_zero` do editor de itens (se a personagem tiver) → o modelo de cada um continua o mesmo de antes (as entradas 9100–9102 da Miri foram renumeradas para não tomar as deles).
- [ ] Entrar numa fase com a Fiona e atacar algumas vezes → sons de golpe e de passos normais.
- [ ] A grade de criação (C3) abre e mostra as 11 classes como antes (a Miri ainda não aparece).
- [ ] (M2) Numa loja de NPC e no inventário, a dica de uma poção e de uma peça feminina comum não mostra classe nova nem "Cannot be equipped" para a Fiona; a forja do Ferghus abre sem erro (as receitas de Draker só aparecem quando a Miri existir).
Limites conhecidos: nenhum comportamento da Miri é esperado neste passo.

### MI2 — Painel com a Miri (M3)
Pré-condição: imagem do commit do M3 (`docker compose ps` saudável) e exportações refeitas (`scripts/content/refresh_item_content.py`). Painel em `http://127.0.0.1:27090`.
- [ ] Itens → buscar `draker` → aparecem as armas Draker (76) com nome em inglês e ícone; a dica de `draker_newbie` diz "For Miri".
- [ ] Itens → buscar `miri_inner` → inners da Miri com nome e ícone; abrir um no editor → a caixa **Miri** (e a **Delia**) aparecem entre as classes e só a Miri está marcada.
- [ ] Uma poção (`life_potion_lvl0`) → a dica não mostra restrição de classe.
- [ ] Personagens → uma personagem de classe 10 aparece como **Delia** (não "Classe 10"); se existir uma de classe 11, como **Miri**.
Limites conhecidos: a Miri ainda não entra no jogo (falta o lado nativo, M4–M8); dar itens da Miri a uma personagem antiga é aceito pelo servidor, mas o cliente mostra o item vermelho.

### MI3 — Itens da Delia com o `engine.dll` novo (M4)
Pré-condição: `engine.dll` da cópia `51ea90e70b93ad04` (aplicado por `patch_engine_class_flags.py --apply`; *stub* `.hflg` em `0x430d5000`). Personagem `DeliaLab` com uma arma e um inner da Delia no inventário; uma Fiona.
- [ ] Com a `DeliaLab`, abrir o inventário → itens da Delia sem fundo vermelho, dica sem "Cannot be equipped"; equipar e tirar a arma → funciona como antes (D22).
- [ ] Com a Fiona → itens da Fiona normais; um item só da Delia (se houver no inventário) aparece vermelho, como antes.
- [ ] (Opcional, conferência ao vivo do byte da Miri) com a `DeliaLab` na cidade e `lab_memory.py serve` num terminal de administrador: pedir a leitura do byte `+0x1ce` de um registro de item comum (poção) → `1` (a poção tem o bit 11); de `draker_newbie` → `1`; de uma arma da Fiona → `0`.
Limites conhecidos: os itens da Miri só deixam de ficar vermelhos para uma Miri depois do M5 (os testes de classe nos DLLs ainda param na Delia).

### MI4 — Miri de laboratório: seleção, cidade, arma, itens (M5)
Pré-condição: DLLs da cópia roteados com a Miri (`patch_class_table.py` com 12 classes e `patch_class_routing.py` deste commit, aplicados por mim com o jogo fechado; hashes no `docs/history/session-log.md`, passo M5). Imagem Docker do M3 ou posterior.
- [x] Na seleção, criar uma **Fiona** chamada `MiriLab` (qualquer aparência) → criada e listada. Fechar o jogo.
- [x] `./scripts/dev/lab-set-class.ps1 -Name MiriLab -Class 11` → imprime `cid=<número> class 1 -> 11` e o comando para desfazer.
- [x] Dar a arma: painel → Personagens → `MiriLab` → Dar item `draker_newbie` (ou `./scripts/dev/lab-give-item.ps1 -ItemClass draker_newbie -Count 1 -Cid <cid>`).
- [ ] Abrir o jogo → na seleção, `MiriLab` aparece com o **corpo da Miri** (rosto, cabelo, inner padrão), sem decalques, faixas ou pose T. Anotar o que aparecer errado (captura).
- [x] Entrar na cidade → corpo e animações da Miri; andar, correr, um emote; sem queda. A estamina recupera normalmente.
- [ ] Inventário → `draker_newbie` sem fundo vermelho e sem "Cannot be equipped"; equipar → uma arma, no lugar certo (aproximação: entidade `weapon_greatsword` com o modelo da Draker).
- [ ] Janela de habilidades → as habilidades da Miri (Dragonspine Mastery, Speed Move...) com ícones em posições distintas.
- [ ] Não regressão: `DeliaLab` e uma Fiona entram na cidade como antes (corpo, arma, guarda da Delia).
Limites conhecidos: as mecânicas próprias da Miri (transformação Dragon Knight, Dragon Smash, energia do dragão) ainda não funcionam (M7); a grade de criação ainda não mostra a Miri (M8); a guarda, a purga e os efeitos da Delia ficam só na Delia.

## Resultados

### 30/10/2026 — rodada 1 (MI4; MI1–MI3 não executadas)
O operador foi direto à MI4. Evidência em `.cache/verification/miri/mi4-*` (logs do cliente e do Docker, minidump, amostras de *threads*).

- **MI4-1 a MI4-3:** ok. Fiona `MiriLab` criada pela grade, classe 1 → 11, `draker_newbie` dada pelo painel.
- **MI4-4:** parcial. A seleção mostra o corpo 3D da Miri certo, mas o retrato da lista é de outra classe (rosto masculino): ícone de classe da lista, M8.
- **MI4-5:**
  - 19:55: falhou. O Start desconectou e o jogo caiu: `EXCEPTION_ACCESS_VIOLATION` em `server.dll+0x8b5472`, relatório `heroes_[2016-01-14][17_02]_[2026-10-30][19_55_14].mdmp`, copiado em `mi4-crash-1955/`.
  - Causa 1: o servidor Rust recusava a seleção porque o `skill-profile-v5.json` não tinha as habilidades da Miri. Corrigido com `export_skill_profile.py --all-skills`, agora no `refresh_item_content.py`.
  - Causa 2: `GetSkillKeywordTable` lia, para a classe 11, a vtable de um `ConCommand` como tabela de palavras-chave. Corrigido com `KEYWORD_TABLES` no roteamento (cliente e servidor).
  - Depois da correção (20:36): entra na cidade com o modelo 3D e o inner da Miri certos.
- **MI4-6:** parcial.
  - O item equipa sem "Cannot be equipped" ("Only for Miri").
  - O modelo é `greatsword_practice.mdl`, porque a entidade `weapon_greatsword` escolhe o modelo pelo subtipo (`CostumeType 1`), e a arma fica numa posição estranha.
  - Falta a entidade `weapon_draker` (M7).
- **MI4-7:** parcial.
  - As habilidades próprias da Miri aparecem certas.
  - Training e Restoration estavam vazias e Equipment só tinha a Dragonspine Mastery.
  - Corrigido depois da rodada: as 29 habilidades comuns do 1.69 ganharam o bit 11 (`class_imports.shared_skills`). Reteste pendente.
- **MI4-8:** parcial. A `DeliaLab` cria party e entra na mesma fase (20:57). A Fiona não foi testada. Rodada 2: ok.
- **Extra, party e fase com a Miri:**
  - 20:41 e 20:54: o cliente travou ("Not Responding") ao criar a party. Conectou no *relay* sem mandar a chave pública, ficou 60 s no laço de espera de conexão do `engine.dll`, desistiu e voltou à cidade.
  - 21:03: a terceira tentativa (depois de voltar à seleção) conectou e entrou no barco. É intermitente e não foi reproduzido com a captura armada.
  - No barco, a Miri fica presa na *idle* segurando a arma e não anda nem ataca. Hipótese: condições de estado de ação do NA não registradas no 1.69 (M7).


### 30/10/2026 — rodada 2 (MI4-7, MI4-8 e trava do barco)
- **MI4-7:** ok. Com as 29 habilidades comuns (bit 11), Training, Restoration e Equipment aparecem para a `MiriLab`.
- **Inventário da Miri:** ok. Nenhum "Cannot be equipped" indevido.
- **MI4-8 (Delia depois do M5 e das correções):** ok. Combate completo, transformação Rose Crush, habilidades e itens funcionam.
- **Trava ao criar o barco (Miri e Delia, intermitente):**
  - O cliente não manda a chave pública ao *relay*.
  - A memória baixa (até 2 GB) do jogo estava quase esgotada: 68–176 MB livres, pedaços de no máximo 15 MB (`mi4r2-memory-monitor.txt`). Desde o M1 o jogo usa ~2,0 GB de memória privada; antes eram 1,0–1,2 GB.
  - Com o Firefox fechado (1,8 GB), o barco abriu. Causa provável: pressão de memória da máquina somada ao volume de conteúdo da Miri. Não é defeito de protocolo.

### MI5 — Modelos Draker e diagnóstico da *idle* em batalha (M7a)
Pré-condição: jogo fechado durante `refresh_item_content.py --force` e `apply_lab_content_additions.py --apply` (feitos por mim). Os itens Draker usam o `CostumeType` 10100 + n, e o `weapon_greatsword.txt` da cópia ganhou os 45 modelos do `weapon_draker.txt` do NA nessas chaves. Firefox fechado ou com pouca memória antes de criar o barco.
- [ ] Cidade com a `MiriLab` e a `draker_newbie` equipada → o modelo é a Dragonspine de treino (não a *greatsword* de treino). Anotar onde a arma fica (mão, costas, solta).
- [ ] Outro item Draker (se houver, pelo painel ou `lab-give-item.ps1`) → modelo próprio.
- [ ] Na cidade, andar, correr e um emote com a Miri → anotar se ela anda ou fica na *idle*.
- [ ] No barco, **sem arma** (tirar a Draker) → anotar se anda e ataca de mãos vazias.
- [ ] No barco, com a Draker → anotar se anda, ataca ou fica na *idle* (com o horário, para a leitura de memória).
- [ ] `DeliaLab` e uma Fiona: armas e combate como antes (a *greatsword* e a espada bastarda não mudam).
Limites conhecidos: a entidade continua sendo `weapon_greatsword` (o 1.69 não tem `weapon_draker`), então o encaixe na mão segue o da *greatsword*. A Delia também usa `weapon_greatsword` com o `CostumeType` dos itens dela e pode mostrar modelos de *greatsword*; isso fica para um passo próprio.

Resultado da MI5, rodada 1 (30/10/2026, 23:33, base 20000):
- Draker equipada e nenhuma arma visível na cidade. Na memória não havia nenhuma instância de `C_WeaponGreatSword`. Causa: `CostumeType / 10000` escolhe a entidade na lista de armas da classe, e a posição 2 da Miri está vazia. Base trocada para 5000.
- Barco: travou de novo (02:33 UTC; o *relay* fechou aos 10 s sem a chave pública). A máquina tinha 10 GB livres, então a hipótese de memória da máquina perde força. Captura automática de *threads* armada para a próxima tentativa.

Resultado da MI5, rodada 2 (31/10/2026, 00:25, base 5000):
- Cidade: as animações e ações da Miri funcionam, mas a Draker equipada não aparece. Na memória continua sem instância de `C_WeaponGreatSword`.
- Barco: entrou sem travar. A Miri fica invisível, sem modelo, animação nem arma, com e sem a Draker equipada, mas se move. O modelo combinado `CoStUmE_11/0_90454/...` está na memória, e o inner 90454 existe no `player_costume.txt` mesclado.
- Causa da arma: a regra exata da lista de armas (servidor `0x40823590`): 10000–19999 → posição 0, 20000–29999 → 1, outro valor → `CostumeType / 64`, no máximo 4. Base trocada para 10100. O corpo invisível no barco começou junto com a arma em posição vazia; repetir com a base 10100.

Trava do barco, captura (31/10/2026, 01:07:43, `mi5e-trigger.txt`): nas pilhas de todas as *threads* aos 0, 3 e 8 s, a principal está no laço de espera da conexão (`engine.dll+0x3418f6`, que chama a rede e dorme 1 ms) e as *threads* de rede do `engine.dll` estão paradas em espera. Nenhuma está gerando a chave pública nem em biblioteca de criptografia. A camada ProudNet do cliente não reage ao *hint*. Acontece também com a Delia. As entradas que funcionaram vieram logo depois de voltar à seleção ou de abrir o jogo. Contorno para os testes: voltar à seleção antes de criar o barco. A causa fica para uma tarefa própria do *relay*.

Resultado da MI5, rodada 3 (31/10/2026, 01:00–01:12, base 10100):
- Cidade: ok. A Draker aparece nas costas, e as animações e ações da Miri funcionam.
- Pulo na cidade (Delia e Miri): a arma vai para a mão no pulo e volta para as costas ao pousar, de modo estranho. Comparar com Hurk e Fiona (pendente).
- Barco (01:11, depois de voltar à seleção): corpo visível, com e sem a Draker, mas só a *idle* de combate. WASD desliza a personagem; pulo, clique e Shift não fazem nada. Causa provável e correção no M7b, MI6.

### MI6 — Estados de batalha da Miri (M7b)
Pré-condição: jogo fechado durante `refresh_item_content.py --force` e `apply_lab_content_additions.py --apply` (feitos por mim); `player_action_state_miri[_ex].txt` sem os `include` de `common_state_rope.txt` e `common_state_interactive_object.txt`. Antes do barco, voltar à seleção (contorno da trava do *relay*).
- [ ] Barco sem arma: andar com animação, pular, atacar de mãos vazias.
- [ ] Barco com a Draker: andar, correr, pular, ataque básico (clique) e esquiva (Shift).
- [ ] Numa fase: combate contra monstros, ataque forte (botão direito) e uma habilidade da barra.
- [ ] `DeliaLab` no barco: sem regressão.
Limites conhecidos: as mecânicas próprias (transformação Dragon Knight, Dragon Smash, energia do dragão) e as condições do NA ainda não foram portadas. Se a Miri continuar só na *idle*, o próximo passo é registrar as condições (`CONDITION_NAMES`), como na Delia.

Resultado da MI6, rodada 1 (31/10/2026, 01:5x, M7b aplicado): sem mudança. No barco, só a *idle* de combate; a Miri desliza.
- Leitura de memória (`mi6-memfind*.txt`):
  - O método vale: as chaves de estados comuns (`LadderUpDuringJump`) e de outras classes (Delia, Hagie, Hurk e outras) estão na tabela de símbolos do `KeyValues`.
  - Nenhuma chave dos scripts da Miri foi lida: nem dos estados de ação (`AttackMelee_01_attack04`, `SpeedyMove_dragon`) nem da tabela de expressões (`DragonEnergy_transform`, `FirstRestoreDelay`).
  - O roteamento está certo: o caso 11 do *switch* pede `player_action_state_miri[_ex].txt` no cliente e no servidor. O carregador (servidor `0x407ee6b0`) faz `LoadFromFile` e, se falha, a classe fica sem estados.
  - O arquivo da Miri está são (chaves balanceadas, sem BOM, tokens curtos) e no índice.
  - Ele está em `scripts/actionstate` parte 1 (`661E4BD9`), um pacote novo depois da única parte do 1.69 (`DCB08EBB`). O mesmo vale para `scripts/expression` parte 1, `scripts/skill` parte 1 e `scripts` parte 1. Formato e atributo (0x20) iguais aos dos pacotes originais.
- O que funciona: modelos em pastas **novas** da Miri (`models/player/_anim/female/miri` parte 0) e o que foi mesclado em pacotes **originais** (`player_costume.txt`).
- Hipótese: o cliente não lê partes novas (N ≥ 1) de pastas que já existem no 1.69. O percurso das partes fica no `FileSystem_Stdio` (`0x10027cd0`, protegido por VMProtect).
- Experimento proposto: pôr os scripts da Miri na parte que o 1.69 já tem (`DCB08EBB` e similares), reconstruída a partir do *backup*, como já se faz com as mesclas.

### MI7 — Scripts da Miri nos pacotes originais (M7c) e seleção da Murael e da Tsuko
Pré-condição: imagem Docker com o registro dos passos da seleção (`minimum_selection.rs`, `traced`); com o jogo fechado, `apply_lab_content_additions.py --apply` põe os 7 scripts da Miri em pastas que o 1.69 já tem (`scripts/actionstate`, `scripts/expression`, `scripts/skill`, `scripts`) na última parte original da pasta, em vez de pacotes novos.
- [ ] Barco com a Miri (sem arma e com a Draker): anda com animação, pula, ataca, Shift.
- [ ] Murael (Fiona) e Tsuko (Evy): tentar entrar. Se recusar, anotar o horário; o log do `game` dirá o passo (`minimum selection step refused`, `step`, `kind`).
- [ ] `DeliaLab` e `MiriLab`: entram como antes.
Limites conhecidos: as mecânicas próprias da Miri ainda não foram portadas.

Resultado da MI7, rodada 1 (31/10/2026, 02:40):
- Tsuko recusada. O log do `game` mostrou o passo `stats.values` (`conflict`): os atributos gravados não batiam com os recalculados. O cálculo soma os títulos de **todas** as personagens da conta (`TitleQueryAcquiredByAccount`). A `DeliaLab` ganhou 12 títulos em 29/10 (progressão pelo painel), e os atributos gravados da Murael e da Tsuko ficaram velhos; a seleção só recalculava os da personagem que ganhava o título. Não tem relação com a Miri nem com o *inner zero*.
  - Correção: na seleção, `calculate_base_stat` grava de novo a cópia antes de conferir, como o `StatManager.CalculateBaseStat` na carga. A cópia gravada é só um *cache* do valor da classe no nível mais os títulos.
- Miri no barco: continua só na *idle*.
  - O jogo pede a variante `player_action_state_miri_ex.txt` (cópias do nome na memória do cliente e do servidor). As chaves desse `_ex` existem todas em arquivos do 1.69, então a memória não prova se ele foi lido.
  - Os cabeçalhos do `_ex` da Miri e da Delia são idênticos. O experimento dos pacotes originais é inconclusivo e fica: não piora nada e mantém os scripts com o pacote da pasta.
  - Próximo passo: ler na memória a máquina de estados de ação do jogador da Miri no barco (estado atual e tabela de estados carregada), comparando com a Delia.

Resultado da MI7, rodada 2 (31/10/2026):
- Murael (Fiona) e Tsuko (Evy) entram na cidade e lutam no barco; o *inner zero* da Evy funciona (recálculo dos atributos base na seleção).
- Hurk novo (Espadao): criação e cidade ok. Na cidade a *greatsword* fica nas costas ao pular; no barco, combate ok e a arma na mão ao pular.
- Hagie (Teste): na cidade a arma fica na cintura ao pular.
- Delia: na cidade a arma vai para a mão ao pular; no barco, combate e animações ok.
- Miri: na cidade a arma vai para a mão ao pular. No barco fica congelada numa pose de referência, sem animação ao andar, pular ou atacar. Causa: o `_ex` do NA é um casco de 1 sequência; ver MI8.
- Pulo na cidade: as duas usam o `JumpTown` original (`selectweapon weapon_defaultsword`); ver `docs/future/miri-class-port.md`.

### MI8 — Combate da Miri com o `_ex` completo (M7d)
Pré-condição: `pc_miri_battle_motion_ex.mdl` da cópia com 551 sequências (cópia do modelo de batalha), aplicado com o jogo fechado.
- [ ] Barco sem arma e com a Draker: andar, correr, pular, ataque básico (clique), ataque forte (botão direito), Shift.
- [ ] Fase: combate contra monstros e uma habilidade da barra.
- [ ] Anotar animações que faltarem (pose congelada num golpe, por exemplo), com o nome da habilidade ou do golpe.
Limites conhecidos: Dragon Knight, Dragon Smash e energia do dragão não foram portados; os golpes que dependem deles podem não sair.

Resultado da MI8, rodada 1 (31/10/2026): andar, atacar, combos e golpes funcionam no barco (M7d ok). Pendências:
- **Pulo no barco** não anima. O modelo de batalha da Miri **no próprio NA** não tem `jump_begin`/`jump_during`/`jump_end` (só escada e emotes). Os do 1.69 da Delia (`pc_delia_battle_motion_ex.mdl`) têm. Caminho: `includemodel` de um modelo feminino com o pulo, ou cópia das sequências.
- **Corrente da Draker invisível** no arremesso (Shift + botão direito). O modelo tem os pontos `chain_01`/`chain_town`; os eventos do NA `AE_ATTACH_ANIMATING_PROP`/`AE_DETACH_ANIMATING_PROP` (M0) não existem no 1.69. Patch nativo.
- **Arma flutuando no descanso de combate** (*idle* depois de atacar). O NA usa o modo de encaixe `battle_rest` (`TownAttachData` do `weapon_draker.txt`), que o 1.69 não lê.
- **Pulo na cidade** (pedido do operador: como o do Hurk): tirar o saque da arma do `JumpTown` (`selectweapon weapon_defaultsword`) ou a ativação dele nos arquivos de cidade da Delia (1.69) e da Miri.

### MI9 — Pulos sem a arma na mão, pulo em batalha e descanso de combate (M7e)
Pré-condição: com o jogo fechado, `stage_na_class.py --class miri --weapon draker` e `apply_lab_content_additions.py --apply` (feitos por mim; `.cache/verification/miri/m7e/`). Na cópia: `jump_town` da Miri e da Delia com a arma presa às costas (`mdl_anim.py`), `pc_miri_battle_motion_ex.mdl` incluindo o `pc_delia_battle_motion_ex.mdl` (pulo de batalha) e com a arma na mão em `battle_idle2rest[b]`/`battle_rest[b]`. Antes do barco não é preciso voltar à seleção (trava do *relay* corrigida no `engine.dll`, B2 RL1).
- [ ] Cidade com a `MiriLab` (Draker equipada): pular parado e correndo → a Draker fica nas costas durante todo o pulo, como a *greatsword* do Hurk (sem ir para a mão nem "voltar" ao pousar).
- [ ] Cidade com a `DeliaLab`: pular parado e correndo → a arma fica nas costas; andar, correr e um emote como antes.
- [ ] Barco com a Miri (sem arma e com a Draker): pular parado e correndo → animação de pulo (subida, ar, pouso), sem deslizar congelada; clique e botão direito no ar → ataque no ar.
- [ ] Barco com a Miri: atacar e esperar parada até a *idle* de descanso → a Draker fica na mão (não solta no ar).
- [ ] Barco: Shift + botão direito (arremesso) → anotar como a arma aparece (sem corrente ainda, ver limites).
- [ ] `DeliaLab` no barco: pulo, combate e Rose Crush sem regressão; uma Fiona ou o Hurk na cidade: pulo como antes.
Limites conhecidos: a corrente da Draker (braço esquerdo e cabo até a arma) é código nativo do NA (`C_WeaponDraker`) ainda não portado (M7f): nos ataques e no arremesso a arma continua se afastando da mão sem a corrente visível. Com `ExtremeMode` a Miri usa o conjunto de golpes da Delia (os `_ex` do NA são cópias dos dela); Dragon Knight, Dragon Smash e energia do dragão ainda não funcionam. Se a seleção ou o barco ficarem mais lentos, anotar (o modelo da Delia passa a carregar junto com a Miri).

### MI10 — Corrente da Draker (M7f)
Pré-condição: com o jogo fechado, `stage_na_class.py` (material `cable/heroes_metalchain_05` do NA), `apply_lab_content_additions.py --apply` e o roteamento do `client.dll` refeito a partir do `*.pre-routing.bak` com o grupo `CHAIN` (feitos por mim; hashes no histórico, passo M7f). Só o `client.dll` muda; o `server.dll` fica igual.
- [ ] Cidade com a `MiriLab` e a Draker equipada: uma corrente curta **na própria arma** (entre os pontos `chain_01` e `chain_town` da Draker, nas costas), sem cabo até a mão; anotar se ela acompanha ao andar, correr e pular. Vale nas cidades `lobby_multiplayer`, `_morvan`, `_rochest` e nas `s3_`.
- [ ] Corrente enrolada no braço esquerdo (`draker_chain.mdl`): aparece no antebraço e acompanha o braço (sem ficar parada no ar nem no chão), com a ponta solta balançando.
- [ ] Tirar a Draker e equipar de novo (inventário): o cabo e a corrente do braço somem sem a arma e voltam com ela; nenhuma corrente solta parada no chão ou no ar.
- [ ] Barco com a Draker: ataques 01–04 e o arremesso (Shift + botão direito) → a corrente liga a mão esquerda à arma enquanto ela se afasta e volta. Anotar se ela fica esticada, frouxa demais, atravessa o corpo ou pisca.
- [ ] Barco: descanso de combate (parada depois de atacar) → a arma na mão e a corrente curta.
- [ ] Sair do barco para a cidade e voltar: a corrente reaparece; nenhuma corrente fica para trás no lugar antigo.
- [ ] Cidade: um emote ou interação que esconde a arma (por exemplo sentar, ou uma interação de objeto) → o cabo e a corrente do braço somem junto com a arma e voltam quando ela volta.
- [ ] `DeliaLab` e uma Fiona: nenhuma corrente; combate e cidade como antes.
- [ ] (Opcional, com `lab_memory.py serve`) anotar o horário se algo der errado: a tabela da corrente está em `class-routing.json` (`class_conditions.chain.table`).
Limites conhecidos: a cidade é reconhecida pelo nome do mapa (as cinco cidades que o próprio `engine.dll` lista); outro mapa sem servidor de batalha usaria o cabo de batalha. O cabo e a corrente do braço seguem só o esconder da arma por evento (`visible`/`invisible`); uma arma escondida de outro jeito pode deixá-los à vista. Até 16 jogadores com corrente por mapa; a corrente de uma Miri que sai do mapa é removida no próximo *think* de outra Miri (sem outra Miri, fica até trocar de mapa).

Resultado da MI9/MI10, rodada 1 (31/10/2026, `MiriLab`, leitura de memória ao vivo):
- Cidade: a Draker fica nas costas no pulo, mas o corpo pula na pose de combate (como se segurasse a arma). Medido: o `jump_town` da Miri é o mesmo da Delia (NA e 1.69), com os braços na pose do pulo de batalha; o NA não tem pulo nenhum (estados comentados). Escolha do operador: testar o pulo de cidade da Fiona (mãos livres); se não ficar bom, voltar.
- Barco: a corrente não aparece no arremesso. Memória: o teste de cidade pelo nome do mapa estava invertido (Colhen e o porto são `t01`; o barco é `lobby_multiplayer`), então o barco recebia o cabo de cidade (preso só na arma); e a corrente do braço nunca foi criada (*handle* `-1`: modelo sem *precache*). Estado do jogo: Colhen 14, porto 17, barco 23.
- Descanso de combate: a Miri abre a mão de propósito (a arma fica pendurada na corrente); prender a arma na mão estava errado.
- Painel: a Miri não tinha o conjunto de nível 60 (`pythonshock_set`): as tabelas de conjunto não eram importadas.
- Draker de nível 60 equipada com maestria F: o NA tirou o requisito de maestria das armas.

### MI11 — Correções da rodada 1 (pulo da Fiona, corrente no barco, conjuntos, maestria)
Pré-condição: com o jogo fechado, preparação, `refresh_item_content.py --force` (reinicia o `game`), `apply_lab_content_additions.py --apply` e o roteamento do `client.dll` refeito dos `*.pre-routing.bak` (feitos por mim; hashes no histórico, passo M7f). A `MiriLab` precisa estar fora do jogo durante a aplicação.
- [ ] Cidade com a `MiriLab` e com a `DeliaLab`: pular parado e correndo → o pulo de cidade da Fiona (mãos livres), a arma nas costas o tempo todo. Dizer se fica bom (se não, eu volto ao pulo delas).
- [ ] Cidade e porto com a Miri: uma corrente curta presa só na arma (nas costas), sem ligação com a mão; a corrente enrolada no braço esquerdo aparece.
- [ ] Barco com a Draker: ataques e arremesso (Shift + botão direito) → a corrente sai da mão esquerda até a arma e acompanha o lançar e o puxar; a corrente do braço aparece.
- [ ] Barco: parada depois de atacar → a Miri abre a mão e a arma fica pendurada na corrente (sem ficar solta no ar).
- [ ] Um emote na cidade que esconde a arma → corrente e corrente do braço somem juntas.
- [ ] Painel → Itens → buscar `pythonshock` → as cinco peças de nível 60 da Miri, no conjunto `pythonshock_set`; no jogo, a dica de uma peça mostra o conjunto e os bônus.
- [ ] Inventário da Miri com maestria baixa (F): `draker_titan` ou `draker_colru` (nível 60) pede Dragonspine Mastery rank 6 e não equipa; `draker_newbie` equipa como antes.
Limites conhecidos: a corrente de cidade e a do barco seguem o estado do jogo (cidades 14, 17, 35); o pulo da Fiona é uma escolha provisória até comparar com um cliente EU. Requisitos de maestria acima do rank 8 caem para o 8 (a maestria da Miri não tem os ranks 9-12).

Resultado da MI11 (31/10/2026, `MiriLab`):
- Pulo de cidade da Fiona: aprovado pelo operador (fica).
- Corrente no combate: correta nos combos iniciais (habilidades avançadas não testadas).
- Fonte termal: a corrente e a corrente do braço ficavam visíveis sem a arma. O 1.69 esconde a arma ali pelo `ShouldDraw` dela (dono com o *byte* `+0x126e`, o mesmo das poções da fonte), não por `EF_NODRAW`.
- E num núcleo derrubado ("GET"): nada acontecia. O estado `SearchEvilCore` toca `search_evil_core`, que vem do modelo da Delia incluído, mas o roteiro de eventos da Miri não tinha o bloco (sem `Item_Loot`).
- Nenhuma SP da Miri e Tab (Fiery Evasion) depois de aprender Fiery Assault: com `ExtremeMode` a Miri jogava o `_ex` do NA, cópia do da Delia; o `translated_spskill.txt` do NA aninha as SP sob `"0"`; e as condições `As_Cond_Miri_*` não existiam no 1.69.
- Pedido novo: o HUD de Flamebreath (dragão que enche de vermelho), em estudo.

### MI12 — Correções da MI11 (fonte termal, E nos núcleos, SP, Tab)
Pré-condição: com o jogo fechado, preparação, `apply_lab_content_additions.py --apply` e o roteamento refeito dos `*.pre-routing.bak` (feitos por mim; hashes no histórico, passo M7g).
- [ ] Fonte termal com a `MiriLab` → ao entrar, a arma, a corrente e a corrente do braço somem juntas; ao sair, voltam.
- [ ] Fase: matar um monstro que derruba núcleo ("GET"), parar na frente e apertar E → a Miri se abaixa e o item entra no inventário (núcleo dourado também).
- [ ] Barco ou fase: as SP da Miri na barra (X/teclas de SP): Scorching Streak, Slice and Dice, Blazing Spine, Ancestral Garb (anima, sem a roupa de Dragon Knight), Call of the Dragon → cada uma sai e gasta SP.
- [ ] Combos da Miri (os dela agora, não os da Delia): ataques e Smash (Coiled Strike, Tail Impale etc.), Spinethrow (Shift + direito) → animações dela, sem congelar.
- [ ] Com Fiery Evasion e Fiery Assault aprendidas: Tab durante um ataque → esquiva; Tab + direito → Fiery Assault. Fiery Chainswing depois dos Smash (direito de novo).
- [ ] Pulo em batalha (barco) continua com animação e aterrissagem.
Limites conhecidos: Flamebreath (`skill_miri_dragon_energy`) ainda não existe; as habilidades Fiery ficam sempre liberadas (`As_Cond_Miri_Can_DragonSmash` sempre ligada) e não há HUD. As versões transformadas (Dragon Knight) ficam fechadas.

### MI13 — Flamebreath e o HUD do dragão
Pré-condição: com o jogo fechado, roteamento refeito dos `*.pre-routing.bak` (feito por mim; hashes no histórico, passo M7h). Pode ser jogado junto com a MI12. A `MiriLab` com Fiery Chainswing, Fiery Evasion e Fiery Assault aprendidas.
- [ ] Cidade → nenhum HUD de dragão.
- [ ] Barco ou fase → o dragão (`MiriSkillDialog`) aparece; ao entrar começa vazio e enche um nível a cada 7 s até cheio (8 níveis, cerca de 1 min). Dizer onde aparece na tela (posição) e se o desenho enche como no NA.
- [ ] Com o dragão cheio: Fiery Chainswing depois de um Smash, Tab (Fiery Evasion) e Tab + direito (Fiery Assault) → cada uma gasta um nível (o dragão baixa) e 7 s depois volta a encher.
- [ ] Gastar tudo (dragão vazio) → as habilidades Fiery e o Tab não saem até encher de novo.
- [ ] Voltar para a cidade → o dragão some.
Limites conhecidos: o dragão aparece para a Miri de qualquer nível (o NA mostra só depois de aprender Flamebreath, nível 20); começa vazio a cada entrada (reinício do 1.69); a forma Dragon Knight não existe. Na missão do título de poluição de Regina a barra de febre usa o mesmo campo e a energia para.

Resultado da MI12/MI13 (31/10/2026, `MiriLab`):
- Pulo na cidade e em batalha pararam: o roteiro completo da Miri inclui o `common_state_movement.txt` do 1.69, que tem todos os estados de pulo comentados; todo `_ex` do 1.69 inclui o `common_state_movement_ex.txt`.
- HUD de Flamebreath não apareceu: o 1.69 só mostra um filme declarado em `scaleform/UIElements.xml` (`BaseUI.setVisibleElement` pelo nome), e o `MiriSkillDialog` não estava lá.
- Corrente na fonte termal continua aparecendo: o `ShouldDraw` da arma não é o que a esconde ali (o *byte* `+0x126e` não é escrito por código; `m_isHostInHotSpring` é da entidade da fonte, `C_HotSpring +0x540`). Falta medir na memória com a Miri dentro da fonte.
- Regeneração de SP "bugada" e dano baixo no White Tyrant (Hard) com 12k: em análise. Os golpes da Miri usam fatores de dano do NA na mesma faixa da Delia (mediana 38 contra 30) e 43 de 44 ações existem (falta `lynn_kick_01`); o campo da barra de febre só alimenta a barra de febre e o veneno de Regina, não o SP. Faltam números do jogo.

### MI14 — Pulos de volta, HUD do dragão declarado
Pré-condição: com o jogo fechado, preparação e `apply_lab_content_additions.py --apply` (feitos por mim; passo M7i). DLLs iguais aos da MI13.
- [ ] Cidade e barco/fase: pular parado e correndo → pulo de cidade (Fiona) e de batalha voltam.
- [ ] Barco ou fase → o dragão (`MiriSkillDialog`) aparece no centro, à direita (x 300); enche um nível a cada 7 s; Fiery Chainswing, Tab e Tab + direito gastam um nível cada; vazio, não saem. Arrastar o dragão com o mouse funciona.
- [ ] Cidade → o dragão some.
- [ ] Fonte termal: entrar com a `MiriLab` e avisar, com o jogo aberto, para eu ler a memória (só leitura) enquanto a corrente aparece sem a arma.
- [ ] SP: descrever o que está errado (não enche ao bater? enche devagar? a SP usada não volta?) e o valor de SP de uma SP da Miri na dica (custo).
- [ ] Dano: no mesmo monstro (por exemplo um Goblin ou o White Tyrant), anotar os números de dano de 3–4 golpes da Miri (ataque normal e Smash) e, se puder, os da `DeliaLab` com ATK parecido; dizer se algum golpe não mostra número.

Resultado parcial da MI14 (31/10/2026, `MiriLab`, leitura de memória ao vivo):
- Fonte termal: só a corrente da arma aparece. Memória: o *byte* `+0x126e` do jogador vale 1 dentro (0 fora), o `ShouldDraw` da arma é falso; a arma é recriada ao entrar (mesmo endereço, série nova); o cabo tem `EF_NODRAW`, mas o `ShouldDraw` do `C_RopeKeyframe` ignora o próprio *flag* (só esconde com `EF_NODRAW` na entidade de início).
- Pulo em batalha não sai: os eventos próprios da Miri (NA sem pulo) nunca abrem `JumpBegin`.
- Pulo na cidade congela depois de cair até trocar de mapa: o 1.69 guarda os eventos do `jump_town` no mesmo roteiro `_ex`, e o dela não tinha o bloco.
- HUD de Flamebreath, SP e dano: não testados nesta rodada.

### MI15 — Corrente na fonte termal, pulos
Pré-condição: com o jogo fechado, preparação, `apply_lab_content_additions.py --apply` e o roteamento do `client.dll` refeito dos `*.pre-routing.bak` (feitos por mim; passo M7j).
- [ ] Fonte termal: entrar → a corrente some com a arma; sair → a corrente volta.
- [ ] Emote que esconde a arma na cidade → a corrente some e volta com a arma.
- [ ] Cidade: pular parado e correndo várias vezes → cai e volta a andar normalmente, sem congelar.
- [ ] Barco/fase: Espaço → pulo de batalha (subida, queda, aterrissagem) e volta ao combate; pulo depois de um ataque.
- [ ] Barco/fase: o dragão de Flamebreath aparece e enche (itens da MI14).
- [ ] SP e dano: as informações pedidas na MI14.

Resultado da MI15 (31/10/2026, `MiriLab`):
- Pulo na cidade e corrente na fonte termal: aprovados.
- Botões no ar não atacam; falta o XE: o NA não tem pulo nem XE para a Miri (a Nexon removeu os dois); o operador decidiu migrar da Delia e criar um XE como habilidade extra.
- HUD de Flamebreath não apareceu numa fase; Tab (Fiery Evasion) não sai.
- Giant Polar Bear com 13k de ATK: 37 de dano por golpe e nenhum SP ao atacar (só ao apanhar): as ações `miri_*` não estavam no `action_table_player_ex.txt` que o `ExtremeMode` lê.

### MI16 — Dano, SP, ataques no ar
Pré-condição: com o jogo fechado, preparação e `apply_lab_content_additions.py --apply` (feitos por mim; passo M7k). DLLs iguais aos da MI15.
- [ ] Fase (o mesmo Giant Polar Bear): golpes normais e Smash → dano compatível com 13k de ATK (anotar 3–4 números) e a barra de SP enche ao acertar.
- [ ] Barco/fase: pular e apertar esquerdo no ar → ataque no ar; direito no ar → ataque forte no ar (animações da Delia).
- [ ] Fase: o dragão de Flamebreath aparece? Se não, avisar com o jogo aberto, na fase, para eu ler a memória (nível, estado do HUD).
- [ ] Fase, com Fiery Evasion aprendida e o dragão com nível: Tab durante um ataque → esquiva. Se não sair, avisar junto com o item anterior.
Limites conhecidos: XE ainda não existe para a Miri (habilidade extra a criar).

Resultado da MI16 (31/10/2026, `MiriLab`, memória ao vivo):
- Dano e SP: resolvidos (operador).
- Dragão de Flamebreath: aparece no barco e na fase e enche sozinho (memória: estado de jogo 25, mostrado, nível 5), como no NA (um nível a cada 7 s). Na cidade aparecia ao falar com um NPC: o filme pré-carregado começa visível e a interface o mostra de novo no diálogo, e o *think* só mandava a visibilidade quando ela mudava.
- Ancestral Garb (Dragon Knight) não faz nada além da animação: a transformação não está portada (efeito de estado `dragonknight_transform`, nativo do NA).

### MI17 — Dragão escondido na cidade
Pré-condição: com o jogo fechado, roteamento do `client.dll` refeito dos `*.pre-routing.bak` (feito por mim; passo M7l).
- [ ] Cidade e estalagem: falar com NPCs e fechar o diálogo → o dragão não aparece (no máximo um piscar de meio segundo).
- [ ] Barco e fase → o dragão aparece e enche como antes.

### MI18 — Dragon Knight (mecânica, sem o visual)
Pré-condição: com o jogo fechado, roteamento dos dois DLLs refeito dos `*.pre-routing.bak` (feito por mim; passo M7m). A `MiriLab` com Ancestral Garb (SP4) e pelo menos 200 SP.
- [ ] Fase: usar Ancestral Garb → a animação de transformação toca; o ícone do efeito (70 s) aparece; a roupa ainda não muda (visual a portar).
- [ ] Transformada: Fiery Chainswing, Tab e Tab + direito várias vezes seguidas → o dragão não baixa (Flamebreath ilimitado); Scorching Streak e Blazing Spine saem nas versões de dragão (animações diferentes das normais).
- [ ] Transformada: tentar Ancestral Garb de novo → não sai.
- [ ] Depois de 70 s → volta ao normal: o dragão volta a gastar, Ancestral Garb sai de novo.
- [ ] Delia (`DeliaLab`): guarda e golpe na guarda funcionam como antes (os números das condições dela mudaram).
- [ ] Repetir a MI17 (dragão escondido na cidade com NPC) e o Tab, se ainda não foram jogados.
Limites conhecidos: sem o visual de Dragon Knight; a transformação não gasta SP (só exige 200); custo menor das SP transformadas não portado.

### MI19 — Visual do Dragon Knight
Pré-condição: com o jogo fechado, roteamento refeito dos `*.pre-routing.bak` (feito por mim; passo M7n). A `MiriLab` com Ancestral Garb e 200 SP, vestindo roupas e cores conhecidas (anotar ou tirar print antes).
- [ ] Fase: Ancestral Garb → durante a animação (por volta do quadro 180) a Miri passa a vestir a armadura de Dragon Knight; a arma continua a mesma.
- [ ] Transformada: andar, atacar e usar as SP → a roupa de Dragon Knight fica.
- [ ] Depois de 70 s (ou ao morrer) → a roupa, as cores e o corpo voltam exatamente como antes (comparar com o print); armadura quebrada continua quebrada.
- [ ] Transformar de novo na mesma fase → veste e devolve outra vez.
- [ ] Trocar de mapa transformada → no mapa novo a roupa é a normal.
- [ ] Se algo ficar errado (parte trocada, cor diferente, corpo deformado), anotar qual parte e me avisar com o jogo aberto (leio a memória).
Limites conhecidos: sem a troca de textura do NA (`AE_CL_TEXTURE_TRANSITION`); a transformação não gasta SP.

### MI20 — Retrato da Miri na lista de seleção e conjuntos únicos
Pré-condição: com o jogo fechado, preparação e `apply_lab_content_additions.py --apply` (feitos por mim; passo M7o).
- [ ] Seleção de personagens: a `MiriLab` mostra o retrato da Miri (cabelo preto, fundo escuro) na lista, no lugar do rosto de homem; as outras classes (Delia inclusive) continuam com os retratos delas.
- [ ] Painel → Itens → buscar `pythonshock` e `dragon_rise` → cinco peças de cada conjunto único da Miri (níveis 60 e 54); dar à `MiriLab` e vestir → a dica mostra o conjunto e os bônus ativos conforme o número de peças.

Resultado da MI17–MI20 (31/10/2026, `MiriLab`, capturas do operador):
- Seleção: retrato ainda de homem. O filme mapeia o número da classe para o rótulo por `characterImageLabel = new Array("Lethita", .., "Delia")`; o quadro `Miri` sozinho não bastava.
- Dragão de Flamebreath aparece ao falar com NPC na estalagem: o teste de cidade só via 14, 17, 35; a conversa troca o estado (subestados de cidade).
- Dragon Rise e Pythonshock em vermelho, sem ícone: o NA exige `skill_common_armor_mastery`, que o 1.69 não tem, e arquiva a armadura como parte/tipo; os ícones não tinham `miri` no nome e não vieram.
- Dragon Knight no barco: modelo branco em *wireframe* (material `Heroes_PBS_Texture_Transition` do NA, sem *shader* no 1.69); o `.csf` também nomeia a arma do NA.
- Pedido novo: efeitos de Blazing Spine (chama azul) e Slice and Dice não aparecem: os materiais citados pelos `.efx` não eram preparados (40 faltando, entre eles `effects/fire_ani_03`, `effects/dark_02`).

### MI21 — Correções da MI17–MI20 e efeitos das SP
Pré-condição: com o jogo fechado, preparação, `refresh_item_content.py --force` (reinicia o `game`), `apply_lab_content_additions.py --apply` e o roteamento do `client.dll` refeito (feitos por mim; passo M7p).
- [ ] Seleção: a `MiriLab` com o retrato da Miri (cabelo preto, fundo escuro); as outras classes como antes.
- [ ] Cidade e estalagem: falar com NPCs, abrir lojas e entrar em prédios → o dragão não aparece; barco e fase → aparece.
- [ ] Inventário: Dragon Rise (54) e Pythonshock (60) com ícone, pedindo Light Armor Mastery (rank 5 e 6) em vez de algo vermelho impossível; com a maestria, vestem; bônus de conjunto na dica. Painel: as peças na categoria Light Armor.
- [ ] Barco/fase: Ancestral Garb → a armadura de Dragon Knight aparece com textura (não branca); a arma continua a dela; ao fim, a roupa volta.
- [ ] Blazing Spine → chama azul saindo da arma; Slice and Dice → efeito escuro de corte; outras habilidades com efeitos próprios (Fiery Chainswing, Call of the Dragon) também mostram efeito.
- [ ] Draker de nível alto (ex.: `draker_orna`, 115) → modelo com textura (antes podia ficar branco).
Limites conhecidos: o material da armadura de Dragon Knight e das Draker novas usa o *shader* de armadura do 1.69 (sem o PBS do NA); `effects/fire_ani_03_blue` não existe no NA; a transformação mantém a arma atual.

Resultado da MI21 (01/11/2026, `MiriLab`):
- Retrato na seleção e dragão escondido nas conversas: aprovados.
- Dragon Rise equipa, mas nenhum modelo aparece: nem os modelos (`models/armor/dragon_rises/`) nem as entradas do `player_costume.txt` citam a Miri, e não vieram. Nome do conjunto aparece como `HEROES_ITEM_SETITEM_NAME_dragon_rise_set`.
- Efeitos: Blazing Spine sem chama azul; transformada, sem asa e sem fogo nas correntes ao girar. 37 efeitos das habilidades dela usam `AE_CL_PLAY_CUSTOM_EFFECT`, que nenhum roteiro do 1.69 usa; o fogo da arma vem do `weapon_draker.txt` do NA (rastros `miri_trail_dragon_*`), que a cópia não usa (a Draker é `weapon_greatsword`).

### MI22 — Modelos dos conjuntos, nomes e efeitos das habilidades
Pré-condição: com o jogo fechado, preparação, `refresh_item_content.py --force` (reinicia o `game`) e `apply_lab_content_additions.py --apply` (feitos por mim; passo M7q). DLLs iguais aos da MI21.
- [ ] Vestir as cinco peças de Dragon Rise (e, se quiser, Pythonshock) → as armaduras aparecem no corpo; a dica mostra o nome do conjunto (sem `HEROES_ITEM_SETITEM_NAME_...`).
- [ ] Outras roupas e avatares da Miri do painel → modelo aparece.
- [ ] Blazing Spine, Slice and Dice, Fiery Chainswing e Call of the Dragon → efeitos aparecem (versão normal).
- [ ] Ancestral Garb → efeitos da transformação, inclusive a asa.
Limites conhecidos: transformada, os efeitos são os normais (não os de Dragon Knight); o fogo nas correntes (rastros da `weapon_draker.txt`) ainda não foi portado.

Resultado da MI22 (01/11/2026, `MiriLab`): efeitos das habilidades aparecem; transformada, a chama sai azul (efeito normal) onde o NA usa a de Dragon Knight; Dragon Rise ainda sem modelo (os modelos de roupa importados vieram sem `.vvd`/`.vtx`); painel sem detalhes do conjunto e sem modelo no editor.

### MI23 — Modelos dos conjuntos completos, chamas de Dragon Knight
Pré-condição: com o jogo fechado, preparação e `apply_lab_content_additions.py --apply` (feitos por mim; passo M7r).
- [ ] Vestir Dragon Rise e Pythonshock → as armaduras aparecem no corpo.
- [ ] Ancestral Garb → as chamas da transformação e das habilidades transformadas saem escuras (Dragon Knight); sem transformar, azuis.
- [ ] Painel → editor de itens com Dragon Rise → modelo e detalhes do conjunto; anotar se ainda faltar.
- [ ] (M7s) Painel → dica de uma peça de Dragon Rise e de Pythonshock → nome do conjunto ("Dragon Rise Set", "Serpent Shock Set") e bônus por número de peças; no jogo, o nome do conjunto sem `HEROES_ITEM_SETITEM_NAME_...`.

### MI24 — Fogo da Draker, gasto de SP da transformação e XE *move*
Pré-condição: com o jogo fechado, importação, preparação, `refresh_item_content.py --force` (reinicia o `game`) e `apply_lab_content_additions.py --apply` (feitos por mim; passo M7t). DLLs sem mudança. Pode ser jogada junto da MI23.
- [ ] Combate sem transformar: golpes, giros das correntes e habilidades → rastros de fogo azul da Draker na arma e nas correntes (antes: os rastros da espada grande do 1.69, ou nenhum).
- [ ] Ancestral Garb com 200 SP ou mais → a barra de SP cai 200 no momento da transformação (antes: não gastava). Com menos de 200, não ativa.
- [ ] Transformada: golpes fortes e habilidades transformadas (sequências `_dragon`: Scorching Streak, golpes fortes, *speed move*) → rastros de fogo escuro (Dragon Knight). Golpes básicos transformados continuam com o fogo azul (as sequências são as mesmas do normal).
- [ ] Transformada: Blazing Spine custa 100 SP (normal: 250).
- [ ] XE *move*: janela de habilidades → **XE Move** (texto e ícone da Delia) disponível no nível 8; treinar para o rank 1 (0 AP) ou dar pelo painel. Em combate, ataque + golpe forte juntos (como a Delia) → a Miri faz o XE (`super_power`, gasta 20% do HP, recarga). Anotar se a animação, o dano ou a recarga falharem.
Relatar: um item por linha, com o que apareceu (ou não) e capturas das chamas transformadas.

Resultado parcial da MI24 (01/11/2026, `MiriLab`): Dragon Rise mostrou só elmo, botas e luvas; tirar o peitoral mostrava a calça e vice-versa; trocando peças, o cliente caiu (`EXCEPTION_ACCESS_VIOLATION` em `client` `0x38828069`, rastros da arma: o M7t passou das 12 alças de rastro do 1.69). Corrigido no M7u.

### MI25 — Dragon Rise inteiro e sem queda ao trocar peças
Pré-condição: com o jogo fechado, preparação e `apply_lab_content_additions.py --apply` (feitos por mim; passo M7u). DLLs sem mudança. Os itens da MI24 continuam valendo, menos as chamas escuras transformadas (retiradas: não cabem no 1.69).
- [ ] Vestir as cinco peças de Dragon Rise → corpo inteiro (peitoral e calça juntos, como peça única); tirar uma das duas → a outra aparece sozinha.
- [ ] Equipar e desequipar peças várias vezes, na cidade e numa fase → sem queda.
- [ ] Combate → rastros de fogo azul da Draker nos golpes (rastros 0–3).

Resultado da MI25 (01/11/2026, `MiriLab`): Dragon Rise inteiro e efeitos aprovados; transformação sem dreno contínuo de SP (correto: o NA gasta 200 uma vez). XE Move aparece na janela de habilidades com as chaves de texto (`HEROES_SKILL_NAME_skill_miri_xe_move`, `HEROES_SKILL_DESC_skill_miri_xe_move_ex`) e a Ceara não oferece a história que o libera; a história principal quebrou: Marrec → Story → `GameUI_Heroes_NPCTalkEntity_Talk_NullError,start_storyline`. Causa: as 822 histórias e 1.029 títulos do 1.69 para todas as classes têm a máscara 2047 (classes 0–10), sem a Miri; o "atingir nível 61" do painel completou 183 missões e nenhuma história principal. Corrigido no M7v.

### MI26 — Histórias, XE Move e título
Pré-condição: com o jogo fechado, `refresh_item_content.py --force` e `apply_lab_content_additions.py --apply` (feitos por mim; passo M7v). DLLs sem mudança. Depois, com a `MiriLab` desconectada, no painel: personagem → **Atingir nível** 61 de novo (o mesmo nível é aceito; completa as histórias principais até o 61 e abre as seguintes; as missões já feitas ficam como estão).
- [ ] Entrar com a `MiriLab` → Marrec e os outros NPCs com `?` abrem a história sem o erro `NullError,start_storyline`.
- [ ] Janela de habilidades → XE Move com nome e descrição (textos da Delia), sem chaves `HEROES_...`.
- [ ] Ceara → história **XE Move** (`skill_all_xe_move`) oferecida; aceitar e falar com ela → XE Move liberado para treinar (rank 1, 0 AP); treinar e usar em combate (ataque + golpe forte).
- [ ] Diário de histórias → as principais até o nível 61 concluídas e a seguinte aberta.
Relatar: o que cada NPC ofereceu e se algum erro de NPC apareceu.

Resultado parcial da MI26 (01/11/2026, `MiriLab`, depois de refazer "Atingir nível 61"): Marrec sem o erro; a Ceara não deu o XE Move (a história `skill_all_xe_move` ficou aberta na fase A); usar o livro **History of the Gods Book** (`storyscroll_EPD_30raid_elclous`) não fez nada (servidor: "minimum item use rejected"). Causas: o `Switch` do servidor só tem ramos para as classes 0–9 (como a referência), e a Delia e a Miri caíam no `Fail` vazio; o ramo `storyscroll_` do uso de item não tinha sido migrado. Corrigido no M7w.

### MI27 — Livros de história e XE Move da Ceara
Pré-condição: com o jogo fechado, imagem Docker reconstruída e `refresh_item_content.py --force` (feitos por mim; passo M7w). DLLs e cópia do cliente sem mudança.
- [ ] Inventário → usar **History of the Gods Book** → mensagem de história revelada; o livro some; a história `EPD_30raid_elclous` aparece no diário com o NPC (Ti). Usar outro livro de uma história que já tem → mensagem "já possui" e o livro fica.
- [ ] Ceara → história **XE Move** → ela fala, dá o XE Move e a história termina; janela de habilidades → XE Move liberado (treinar rank 1 com 0 AP) e usável em combate (ataque + golpe forte).
Relatar: a mensagem de cada livro usado e se a Ceara deu a habilidade.

Relato da MI27 (01/11/2026, `MiriLab` e `Murael`): E não pega o que caiu no chão; Parrying Attack não defende; ao parar de correr em combate a Miri dá um pequeno *dash* (a Delia e a Fiona não); Call of the Dragon sem o dragão atacando; o dragão do Flamebreath fica na seleção e com a Delia depois de sair da Miri; destruir item do inventário (Murael) não faz nada. Corrigidos no M7x: E, *dash*, dragão do HUD, destruir item. Pendentes de decisão: Parrying Attack e o dragão do Call of the Dragon (mecânicas nativas do NA).

### MI28 — Pegar do chão, corrida, HUD e destruir item
Pré-condição: com o jogo fechado, preparação, `apply_lab_content_additions.py --apply` e imagem Docker reconstruída (feitos por mim; passo M7x). DLLs sem mudança.
- [ ] Miri numa fase: monstro solta núcleo/itens → E perto do que caiu → animação de vasculhar e o item entra no inventário.
- [ ] Miri em combate: correr e soltar a tecla (também um toque rápido) → para sem o *dash* para frente, como a Delia.
- [ ] Sair da Miri para a seleção de personagens → o dragão do Flamebreath some; entrar com a Delia → sem o dragão. Voltar com a Miri numa fase → o dragão volta a aparecer.
- [ ] Qualquer personagem: arrastar um item comum para a lixeira (destruir) → confirma e o item some. Item equipado ou no atalho → mensagem "não pode destruir item equipado"; item indestrutível (ex.: livro de história) → mensagem "não pode destruir".
Relatar: o que aconteceu em cada item e a mensagem mostrada nas recusas.

### MI29 — Parrying Attack e o dragão do Call of the Dragon
Pré-condição: com o jogo fechado, `patch_class_routing.py --apply` (DLLs novas), preparação e `apply_lab_content_additions.py --apply` (feitos por mim; passo M7y). Servidor sem mudança. Pode ser jogada junto da MI28.
- [ ] Parrying Attack: numa fase, deixar um monstro atacar e apertar o ataque básico logo no começo do golpe dele → a Miri não leva dano, o ataque dela continua e aparece o efeito de *endure* dela (fagulha/chama no corpo). Fora dessa janela (no fim do ataque dela) → leva dano normal.
- [ ] Call of the Dragon (SP5) → o dragão aparece junto da Miri e ataca, com fogo e fagulhas; some no fim da habilidade (e se ela for interrompida).
- [ ] Depois do dragão: andar e atacar perto de onde ele esteve → nada invisível bloqueando a Miri ou os monstros.
- [ ] Delia: guarda dela continua igual (bloqueia e o HP reservado funciona como antes).
Relatar: se o *parry* funcionou e com qual golpe; se o dragão apareceu, para que lado ele olhava e se os efeitos dele saíram.

### MI30 — Histórias com fala por classe (Delia e Miri)
Pré-condição: `refresh_item_content.py --force` (feito por mim; passo M7z). DLLs e cópia do cliente sem mudança. Personagem Delia ou Miri com as histórias da temporada 2 abertas (painel → **Atingir nível**, se preciso).
- [ ] Seguir as histórias `S2E01_M01_Milled` (Milled no Crescent), `S2E01_S06_Snake` (Ahir), `S2E02_M10_Idahl_Meet` (Eire) → o NPC fala o texto feminino (o da Fiona) e a conversa continua até o fim, sem travar; a Eire dá a missão `quest_desert_ep2_07` e abre a história seguinte.
- [ ] Histórias com ramo `Fail` (texto genérico) e o tutorial (`GuidePlay_1_2_Alert`: a Delia e a Miri ganham o mesmo item da Fiona) → como antes.
Limites conhecidos: onde a Fiona ganha arma ou habilidade dela (eventos, `GuidePlay_6_Chieftain`), a Delia e a Miri ficam no `Fail`.
Relatar: a fala que apareceu e se a história avançou.

### MI31 — Parrying Attack: Hydra e efeito visto pelo servidor
Pré-condição: com o jogo fechado, `class_imports.py`, `refresh_item_content.py --force`, preparação, `apply_lab_content_additions.py --apply` e `patch_class_routing.py --apply` (feitos por mim; passos M8a–M8c, aplicação conjunta). Miri com Parrying Attack (`skill_miri_attack`) aprendido.
- [ ] Numa fase, defender um golpe com o ataque básico no começo do golpe do monstro → sem dano, efeito de *endure* dela e o som `miri.guard`; aparece o estado **Hydra** (ícone) e a barra de stamina sobe (Hydra STA, +20).
- [ ] Golpe forte do dragão (Fiery Chainswing, `miri_strong_dragon_attack_*`) com Hydra ativo → dano maior que sem ele.
- [ ] Transformada (Dragon Knight) → defender ainda dá stamina, mas não dá Hydra.
- [ ] Monstro que acerta por decisão do servidor (golpe em área, projétil) defendido → o efeito aparece também (feito pelo servidor).
Limites conhecidos: o Hydra some ao levar dano pelos eventos de sequência do NA; o *latch* de tempo do cliente do NA (`miri_endure_latch_time`) não foi portado.
Relatar: se o ícone apareceu, quanto de stamina subiu e se o golpe forte ficou mais forte.

### MI32 — Olhos do dragão do Call of the Dragon
Pré-condição: a mesma da MI31 (preparação e aplicador; passo M8b).
- [ ] Call of the Dragon (SP5) → o dragão aparece com os dois olhos brilhando em vermelho durante todo o ataque; os brilhos seguem a cabeça e somem com ele (também se a habilidade for interrompida).
Relatar: se os dois olhos brilharam, o tamanho do brilho (pequeno/grande demais) e se algum brilho ficou no ar depois.

### MI33 — Flamebreath só com a habilidade, começando em 4
Pré-condição: a mesma da MI31 (DLLs novas; passo M8c). Uma Miri abaixo do nível 20 (ou sem `skill_miri_dragon_energy`) e a `MiriLab` com ela aprendida.
- [ ] Miri sem Flamebreath numa fase → sem o dragão do HUD; golpes de Flamebreath (Dragon Smash) indisponíveis.
- [ ] `MiriLab` (Flamebreath aprendido) ao entrar numa fase → o dragão do HUD já começa com 4 casas e enche até 8 com o tempo; gastar e recarregar como antes; transformada, gasto ilimitado.
- [ ] Com Parrying Attack aprendido, ao entrar na fase → estado **Hydra** já ativo.
Relatar: quantas casas o HUD mostrou ao entrar e se a Miri sem a habilidade viu o HUD.

### MI34 — Efeitos e rastros do Dragon Knight (sem mudança; decisão pendente)
Pré-condição: nenhuma (passos M8d e M8e só investigados; opções no `MIGRATION.md`, "Decisões pendentes").
- [ ] Transformada, golpes básicos → efeitos normais (azuis), como hoje; nas sequências só transformadas, os efeitos escuros. Sem rastros pretos nem fogo na corrente.
Relatar: se quer a aproximação dos rastros (efeitos de laboratório na arma) e qual caminho para os efeitos pelo estado.

### MI35 — Destruir itens pela classe e itens vencidos
Pré-condição: imagem Docker reconstruída (feito por mim; passo M8f).
- [ ] Destruir um item comum pela lixeira → some (como na MI28).
- [ ] Item indestrutível com prazo vencido (ex.: item de evento de N dias, depois do prazo) → destruir pela lixeira funciona; indestrutível dentro do prazo → mensagem "não pode destruir".
- [ ] Onde o cliente destrói pela classe e quantidade (`DestroyItemMessage`; ex.: descartar parte de uma pilha, se a interface oferecer) → só a quantidade pedida some.
Relatar: o que a interface ofereceu e o que sumiu.

### MI36 — XE Move com texto e ícone próprios
Pré-condição: `class_imports.py`, `refresh_item_content.py --force` e `apply_lab_content_additions.py --apply` (feitos por mim; passo M8g).
- [ ] Janela de habilidades da Miri → XE Move com nome "XE Move", descrição do golpe (a mesma da Delia, genérica) e o ícone genérico de XE do 1.69 (o das outras classes), não o da espada da Delia.
Relatar: o ícone e o texto mostrados.
