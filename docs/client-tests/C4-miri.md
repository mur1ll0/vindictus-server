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
