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
- [ ] Na seleção, criar uma **Fiona** chamada `MiriLab` (qualquer aparência) → criada e listada. Fechar o jogo.
- [ ] `./scripts/dev/lab-set-class.ps1 -Name MiriLab -Class 11` → imprime `cid=<número> class 1 -> 11` e o comando para desfazer.
- [ ] Dar a arma: painel → Personagens → `MiriLab` → Dar item `draker_newbie` (ou `./scripts/dev/lab-give-item.ps1 -ItemClass draker_newbie -Count 1 -Cid <cid>`).
- [ ] Abrir o jogo → na seleção, `MiriLab` aparece com o **corpo da Miri** (rosto, cabelo, inner padrão), sem decalques, faixas ou pose T. Anotar o que aparecer errado (captura).
- [ ] Entrar na cidade → corpo e animações da Miri; andar, correr, um emote; sem queda. A estamina recupera normalmente.
- [ ] Inventário → `draker_newbie` sem fundo vermelho e sem "Cannot be equipped"; equipar → uma arma, no lugar certo (aproximação: entidade `weapon_greatsword` com o modelo da Draker).
- [ ] Janela de habilidades → as habilidades da Miri (Dragonspine Mastery, Speed Move...) com ícones em posições distintas.
- [ ] Não regressão: `DeliaLab` e uma Fiona entram na cidade como antes (corpo, arma, guarda da Delia).
Limites conhecidos: as mecânicas próprias da Miri (transformação Dragon Knight, Dragon Smash, energia do dragão) ainda não funcionam (M7); a grade de criação ainda não mostra a Miri (M8); a guarda, a purga e os efeitos da Delia ficam só na Delia.
