# Roteiro de cliente — launcher (trilha L)

Plano e evidência: `docs/future/game-launcher.md` (seções 4.1 e 4.4). A preparação completa, a ordem de gameplay e o modo de relatar entram aqui no gate L6.

### LA0 — botões AUTO1 e AUTO2 do diálogo de login (AutoLogin.txt, sem patch)
Resultado (02/11/2026): AUTO1 (`admin`) entrou; AUTO2 (`lab169_a`) foi recusado pelo servidor, como deve: `lab169_a` é o nome de jogo da conta `admin`, e um nome reivindicado por uma conta não entra digitado (`lab_frontend.rs`, admissão B4.2). O `AutoLogin.txt` foi apagado (guardava senha em texto); o caminho `TransitionArgs` → `LoginDialog_OK` ficou provado.
Pré-condição: Docker no ar; `.cache/play-client/AutoLogin.txt` (criado em 02/11/2026) com `Auto1` = `admin`/`admin` e `Auto2` = `lab169_a`/vazio; abrir pelo `start-lab.bat`.
- [ ] Start Game → diálogo de login → **AUTO1** → seleção de personagem da conta `admin`, sem digitar nada.
- [ ] Voltar ao título (ou reabrir) → Start Game → **AUTO2** → seleção da conta `lab169_a`.
Limites conhecidos: é o recurso de desenvolvedor do próprio cliente; a senha do arquivo não chega ao servidor (só o ID vira passport). Serve para provar o caminho `TransitionArgs` → `LoginDialog_OK` que o launcher vai usar.

### LA1 — Start Game direto para a seleção de personagem (login pelo launcher)
Tentativas de 02/11/2026 sem efeito: `+event_account` (`patch_client_auto_login.py`) e o stub no `GameTitleMenu_V2`; o cliente usa o LoginDialog clássico (captura do operador). Patch atual: o stub no `OnShow` do LoginDialog.
Pré-condição: Docker no ar (`docker compose up -d --wait`); cópia `.cache/play-client` com `patch_client_launcher_login.py` aplicado sobre `c0b5d416…` (hash novo registrado no histórico); contas `admin` e `lab169_a`; jogo fechado. O `start_login.bat` da cópia já usa `-launcher_ticket admin`.
- [ ] `start_login.bat` → tela de título → Start Game → **seleção de personagem sem preencher ID e senha** (o diálogo pode piscar) → segunda senha como sempre → entrar na cidade.
- [ ] Fechar e abrir pelo `start-lab.bat` (sem o parâmetro) → Start Game → o diálogo de login aparece como antes.
- [ ] Trocar `admin` por `conta_inexistente_la1` no `start_login.bat` → Start Game → recusa do servidor; anotar a mensagem e se o diálogo aparece para digitar.
Limites conhecidos: ainda sem ticket (a conta entra pelo nome, como hoje); depois de uma recusa, o próximo Start Game repete o mesmo valor. Se o diálogo continuar aparecendo sem entrar sozinho, relatar se ele piscou e o horário, para comparar com o log do `game`.

### LA2 — login seguro pelo launcher mínimo (ticket de 2 minutos)
Resultado (02/11/2026): o operador entrou pelo `play.ps1` com login e senha; o jogo abriu e foi direto à seleção. Pedido do operador: o login deve valer até fechar o launcher (LA3).

### LA3 — sessão do launcher: o login vale até fechar o launcher
Pré-condição: Docker reconstruído com a migração 0052; `client.dll` `59b60e7a…`; jogo fechado; PowerShell na raiz do repositório.
- [ ] `powershell -File scripts\dev\play.ps1` → login e senha → "Conectado como …" → esperar alguns minutos → Enter → o jogo abre → ficar na tela de título mais de 2 minutos → Start Game → **seleção de personagem sem o diálogo**.
- [ ] Fechar o jogo → o launcher volta a perguntar → Enter → o jogo abre de novo e entra **sem pedir a senha outra vez**.
- [ ] Com o jogo aberto na tela de título, digitar `S` no launcher (sair) → Start Game no jogo → recusa (sessão encerrada).
- [ ] Senha errada no login do launcher → "Login ou senha inválidos." e nada abre.
Limites conhecidos: fechar a janela do PowerShell sem `S` deixa a sessão aberta até o limite de 12 horas; o login por nome digitado continua aceito até a decisão D-L6.
