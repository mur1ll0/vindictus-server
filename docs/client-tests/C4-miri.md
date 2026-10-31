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
- [ ] Cidade com a `MiriLab` e a Draker equipada: anotar se aparece uma corrente da mão esquerda até a arma (nas costas), e se ela acompanha ao andar, correr e pular.
- [ ] Tirar a Draker e equipar de novo (inventário): a corrente some sem a arma e volta com ela; nenhuma corrente solta parada no chão ou no ar.
- [ ] Barco com a Draker: ataques 01–04 e o arremesso (Shift + botão direito) → a corrente liga a mão esquerda à arma enquanto ela se afasta e volta. Anotar se ela fica esticada, frouxa demais, atravessa o corpo ou pisca.
- [ ] Barco: descanso de combate (parada depois de atacar) → a arma na mão e a corrente curta.
- [ ] Sair do barco para a cidade e voltar: a corrente reaparece; nenhuma corrente fica para trás no lugar antigo.
- [ ] `DeliaLab` e uma Fiona: nenhuma corrente; combate e cidade como antes.
- [ ] (Opcional, com `lab_memory.py serve`) anotar o horário se algo der errado: a tabela da corrente está em `class-routing.json` (`class_conditions.chain.table`).
Limites conhecidos: na cidade o NA usa outro cabo, preso só na arma (`chain_01` a `chain_town`); aqui é o mesmo cabo da batalha. O prop da corrente enrolada no braço esquerdo (`draker_chain.mdl`) e o sumiço do cabo quando a arma fica invisível (emotes, interações) ainda não foram portados. Até 16 jogadores com corrente por mapa.
