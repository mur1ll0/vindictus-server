# Roteiro de testes no cliente — B5 social (sussurro, bloqueio, guilda)

Sistemas sociais do servidor Rust (`docs/social-systems.md`). Este roteiro cresce a cada etapa (SO1 sussurro, SO2 bloqueio, G1–G6 guildas); a preparação, a ordem e o relato finais são completados no fechamento do bloco. Precisa de **duas contas e dois clientes** (A e B no jogo ao mesmo tempo).

## Preparação
1. Runtime: depois do commit, `COMPOSE_PARALLEL_LIMIT=1 docker compose build` e `docker compose up -d --wait`; `docker compose ps` mostra `game` e `server` saudáveis.
2. Clientes: `.cache/play-client/start-lab.bat` com `lab169_a` (personagem A) e um segundo cliente com `lab169_b` (personagem B).
3. Relatórios de queda, se houver: `Documents\Vindictus EU GM\Report Error`.

## Como relatar
Para cada linha marcada: ID e número da linha (ex.: `SO1-2`), o que apareceu na tela (texto, captura se possível), se o cliente fechou ou travou, o horário aproximado (para comparar com `docker compose logs game`) e o nome do relatório de queda, se houver.

### SO1 — Sussurro
Pré-condição: A e B no jogo (cidade), cada um num cliente.
- [ ] A sussurra para B (modo Sussurro do chat ou comando de sussurro com o nome de B) → B recebe a mensagem com o nome de A como remetente; A vê a própria mensagem no chat; nenhum erro.
- [ ] B responde a A → A recebe.
- [ ] A sussurra para B escrevendo o nome com maiúsculas/minúsculas diferentes → B recebe.
- [ ] Texto com acentos (ex.: `olá, ação`) → chega igual.
- [ ] A sussurra para um nome que não existe → aviso de nome de personagem inválido (`GameUI_Heroes_Chat_Whisper_Invalid_Character_Name`).
- [ ] B sai para a seleção de personagem (ou fecha o cliente); A sussurra para B → aviso de que B não está online (`GameUI_Heroes_Chat_Whisper_Not_Online`).
- [ ] B volta ao jogo; A sussurra de novo → B recebe.
- [ ] Com A numa missão (navio/dungeon) e B na cidade, um sussurra para o outro → chega nos dois sentidos.
