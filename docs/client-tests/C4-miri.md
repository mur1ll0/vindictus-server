# Roteiro de testes no cliente — C4 Miri (classe 11)

Porte da Miri do cliente NA (`docs/future/miri-class-port.md`, skill `new-class-port`). Cada passo com efeito visível acrescenta uma entrada aqui; o teste da classe em si (seleção, cidade, arma, habilidades, mecânicas, itens, grade) vem quando a entidade e o roteamento estiverem aplicados. Entradas `MI*`; origem no `docs/history/session-log.md` pelo passo `M<n>`.

## Preparação
1. Servidor: `docker compose ps` com `game` e `server` saudáveis (imagem do commit da última entrada pendente).
2. Cliente: cópia `.cache/play-client` com os pacotes da Miri e os scripts mesclados aplicados por `scripts/client/apply_lab_content_additions.py --apply` (passo M1; resultado em `.cache/verification/miri/m1c-apply.log`). Jogo fechado durante a aplicação.
3. `.cache/play-client/start-lab.bat`, conta `lab169_a`.
4. Relatórios de queda: `Documents\Vindictus EU GM\Report Error` (nome do mais novo e horário).

## Ordem de gameplay (uma sessão)
1. **MI1:** não regressão das classes antigas com os pacotes e scripts da Miri.

## Como relatar
ID e número da linha (ex.: `MI1-2`), o que apareceu (captura se possível), se travou ou fechou, horário e relatório de queda. Na primeira falha que bloqueia as seguintes: anotar, voltar ao estado seguro e encerrar.

### MI1 — Classes antigas sem regressão depois do M1
Pré-condição: preparação feita. Nenhuma personagem Miri existe ainda; o M1 só acrescentou pacotes novos (75, 4.331 arquivos) e linhas da Miri em 21 scripts compartilhados (`player_costume.txt`, sons de jogador e de armas, `action_table_player.txt`, `projectile.txt`, `item_data.txt`, `skill_actionstate_match_table.txt`, `translated_spskill.txt`, `bodyshape_fig_list.txt`, entre outros).
- [ ] Abrir o jogo e logar → a tela de seleção abre normalmente, no mesmo tempo de antes (anotar se a carga ficou bem mais lenta).
- [ ] Entrar na cidade com uma Fiona (ou outra classe antiga) e com a `DeliaLab` → corpo, roupa, cabelo e arma certos; andar, correr e um emote.
- [ ] Equipar e tirar um inner da Fiona e o `fiona_inner_zero`/`evy_inner_zero` do editor de itens (se a personagem tiver) → o modelo de cada um continua o mesmo de antes (as entradas 9100–9102 da Miri foram renumeradas para não tomar as deles).
- [ ] Entrar numa fase com a Fiona e atacar algumas vezes → sons de golpe e de passos normais.
- [ ] A grade de criação (C3) abre e mostra as 11 classes como antes (a Miri ainda não aparece).
Limites conhecidos: nenhum comportamento da Miri é esperado neste passo.
