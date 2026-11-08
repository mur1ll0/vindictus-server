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

### LA4 — launcher Rust + Win32 (`vindictus-launcher.exe`)
Resultado (03/11/2026): login pelo launcher → Jogar → Start Game → seleção sem o diálogo, mas só com o launcher marcado como administrador (o `Vindictus.exe` pede administrador no manifesto). Go Back na seleção → Start Game → "We are currently in the Premium Open period" (ticket já gasto); fechar e Jogar de novo funcionou. Corrigido em LA5.
Pré-condição: Docker reconstruído em 03/11/2026 (panel com `?cadastro=1`); `client.dll` `59b60e7a…`; `.cache/play-client/vindictus-launcher.exe` (compilado por `tools/launcher/build.ps1`, 32 bits, win7) ao lado do `Vindictus.exe`; jogo fechado.
- [ ] Abrir `.cache/play-client/vindictus-launcher.exe` → janela "Vindictus" → "Servidor: online" em verde; a pasta do jogo já preenchida.
- [ ] Login e senha → Entrar → "Conectado como …" → Jogar → o jogo abre → esperar na tela de título → Start Game → **seleção de personagem sem o diálogo** → segunda senha → cidade.
- [ ] Fechar o jogo → o botão volta a "Jogar" → Jogar de novo **sem digitar a senha**.
- [ ] Senha errada → "Login ou senha inválidos." em vermelho.
- [ ] "Criar conta" abre o painel já na aba de cadastro; "Painel" abre o painel.
- [ ] Com o jogo aberto na tela de título, fechar o launcher → pergunta → Sim → Start Game no jogo é recusado.
- [ ] (Opcional) Copiar o exe para um Windows 7 → a janela abre sem pedir nenhuma instalação.
Limites conhecidos: instalar, atualizar e verificar arquivos ainda não existem (L1–L3); o login por nome digitado continua aceito até a decisão D-L6.

### LA5 — Go Back e jogo sem administrador
Resultado (03/11/2026): aceito pelo operador. O jogo abriu pelo launcher sem administrador (sem marcar a opção), Go Back → Start Game voltou à seleção sem erro.
Pré-condição: Docker reconstruído com a migração 0053; `.cache/play-client/vindictus-launcher.exe` novo (`3d4b278e…`); **tirar o "Executar como administrador"** das propriedades do `vindictus-launcher.exe` (aba Compatibilidade), que foi marcado no LA4; jogo fechado.
- [ ] Abrir o launcher (sem administrador) → login → Jogar → o jogo abre **sem pedir permissão do Windows** → Start Game → seleção.
- [ ] Na seleção, **Go Back** → tela de título → Start Game → **seleção de novo, sem erro**.
- [ ] Entrar com um personagem e jogar um pouco (cidade, inventário, missão curta): anotar qualquer erro que só aconteça sem administrador.
- [ ] Se o jogo não abrir ou falhar sem administrador: marcar "Abrir o jogo como administrador" → Jogar → o Windows pede permissão só para o jogo → o resto igual.
- [ ] Fechar o launcher com o jogo na tela de título → Start Game → recusa (sessão encerrada).
Limites conhecidos: instalar, atualizar e verificar arquivos ainda não existem (L1–L3).

### LV1–LV3 — versões montadas pela receita (07/11/2026)
Pré-condição: as pastas `.cache/client-versions/v1`, `v2` e `v3` montadas por `scripts/client/build_client_version.py` (relatórios `.cache/client-versions/v<N>.report.json`); Docker rodando com o servidor atual; o launcher (`vindictus-launcher.exe`) copiado para a pasta da versão testada ao lado do `Vindictus.exe` (ele não faz parte da versão); `.cache/play-client` não muda. Fechar o jogo entre uma versão e outra.
- [ ] **LV1 (v1, sem classe nova)**: launcher → login → Jogar → Start Game → seleção sem o diálogo → cidade com um personagem de classe 0–9 → loja de NPC, cash shop (lista carrega), uma missão curta até o fim (IOCP). Não criar Delia nem Miri nem entrar com personagens dessas classes: a v1 não tem as classes, e o servidor ainda não restringe classes pela versão do cliente (anotar se a grade oferece a Delia). Os collabs SAO/Fairy Tail aparecem com texto em inglês.
- [ ] **LV2 (v2, + Delia)**: o mesmo que o LV1, e criar uma Delia pela grade (botão voltar e nome na customização, Create → estalagem → cidade), pular na cidade (arma nas costas) e uma missão com a guarda. A Miri não aparece na grade.
- [ ] **LV3 (v3, + Miri)**: o mesmo que o LV2, e entrar com uma Miri (corrente, Flamebreath, transformação). Deve ser igual à cópia de laboratório, sem os itens de teste do editor (`fiona_inner_lab`, `fiona_inner_fringe`, `fiona_inner_zero`, `evy_inner_zero`).
Relato: versão, passo e o que apareceu de diferente; uma queda junto com o `.mdmp` de `Documents`.

### LV4–LV6 — instalar, atualizar e verificar pelo launcher (L3, 07/11/2026)
Pré-condição: o serviço `updates` no ar (rebuild do Docker com o compose de 07/11, ou `vindictus-server client-dist serve --store .cache/client-dist --bind 127.0.0.1:8090` no host) com as versões 1–3 publicadas; o launcher novo (`tools/launcher/build.ps1`) copiado para uma pasta própria, por exemplo `D:\VindictusLauncher\`; ≈ 16 GB livres. A cópia `.cache/play-client` não é usada.
- [ ] **LV4 (instalar):** abrir o launcher → "Escolher…" → uma pasta vazia → a linha da versão mostra "Publicada: versão 3" → "Instalar o jogo aqui" → a barra avança (verificando, baixando) → no fim "Cliente na versão 3: … baixados …" → login → Jogar → Start Game → seleção → cidade. Fechar o launcher no meio e abrir de novo: "Instalar o jogo aqui" continua de onde parou.
- [ ] **LV5 (verificar e reparar):** com o jogo fechado, apagar `bin\engine.dll` da pasta instalada e mudar um byte de um `.hfs` qualquer → "Verificar arquivos" → no fim "2 baixados" → Jogar funciona. Mudar `cfg\config_env.cfg` (opções) não é desfeito.
- [ ] **LV6 (já tenho o cliente):** escolher uma cópia do 1.69 já usada (sem a pasta `.vindictus-update`) → "Já tenho o cliente" → baixa só o que difere (≈ 1,9 GB para a v3 a partir da v1 montada) → Jogar.
Relato: passo, o texto da linha de mensagem e, se falhar, o `vindictus-launcher.exe --update <pasta> --log l3.log` do mesmo caso.

### LV7–LV8 — notícias, autoatualização e painel "Versões do cliente" (L5, 07/11/2026)
Pré-condição: Docker reconstruído em 07/11 (L5); launcher 0.2.0 (`tools/launcher/build.ps1`) já publicado no armazém (`client-dist launcher`); conta de admin no painel.
- [ ] **LV7 (painel):** painel → "Versões do cliente" → as versões 1–3 com tamanhos de instalação e de atualização, mínima 1 e launcher 0.2.0 → "Nova notícia" (título e duas linhas) → aparece na lista → abrir o launcher: a notícia aparece no quadro do topo → "Ocultar" no painel → reabrir o launcher: some. (Opcional, só com cópias de teste) "Versão mínima" = 3 e conferir que um cliente na versão 1 não abre Jogar até atualizar; depois voltar a 1.
- [ ] **LV8 (autoatualização):** com um launcher mais antigo que o publicado (o operador publica um exe novo com `client-dist launcher --exe … --version 0.2.1`), abrir o launcher sem fazer login → ele se fecha e reabre sozinho na versão nova; com login aberto, a mensagem "Launcher … baixado: entra na próxima vez" e a troca na próxima abertura. Ao lado do exe não deve sobrar `.old` depois da segunda abertura.
Relato: o que aparece na linha de mensagem e na lista do painel; para o LV8, o log de `vindictus-launcher.exe --self-update --log l5.log` no mesmo caso.

### LV9 — outro computador da rede de casa (07/11/2026)
Pré-condição: `.env` com `PUBLISH_ADDRESS=0.0.0.0` e `PUBLIC_HOST=192.168.1.13` (Docker recriado); launcher 0.3.0 publicado (`scripts/dev/package-launcher.ps1 -PublicHost 192.168.1.13`), que grava o endereço do servidor no cliente instalado; `scripts/dev/open-firewall.ps1` executado como administrador neste PC; `.cache/launcher-package/VindictusLauncher.zip` copiado para o outro PC (pendrive ou pasta compartilhada); ≈ 16 GB livres lá.
- [ ] No outro PC, extrair o zip, abrir o `vindictus-launcher.exe` → "Servidor: online", notícias no topo, "Publicada: versão N".
- [ ] "Escolher…" → pasta vazia → "Instalar o jogo aqui" → download pela rede (≈ 12 GB) → "Cliente na versão N". Depois, "Verificar arquivos" termina com 0 baixados (o `heroes.db3` ajustado para 192.168.1.13 não é "reparado").
- [ ] "Criar conta" abre o painel do servidor no navegador do outro PC; criar a conta → Entrar no launcher → Jogar → Start Game → seleção → criar um personagem → cidade.
- [ ] Loja do cash shop abre e lista os produtos; uma missão curta até o fim (relay).
- [ ] (Com este PC também no jogo) os dois personagens se veem na cidade; party e uma missão juntos (roteiro B5 multijogador).
Relato: passo e mensagem; se não conectar, o resultado de `Test-NetConnection 192.168.1.13 -Port 27015` (e 8090, 27090) no outro PC.

### LV10 — BlackCipher na instalação limpa (08/11/2026)
Relato do operador: instalação nova pelo launcher em `F:\Games\Vindictus1.69`; ao clicar em Jogar, "Launcher Error: Please add an exemption for Nexon Game Security (BlackCipher.aes) to your firewall or anti-virus settings and try again". Causa: a pasta `BlackCipher/` estava nas exclusões das versões, e o `NGClient.aes` que o `Vindictus.exe` carrega exige os `.aes` dela (a cópia `.cache/play-client` tem a pasta). Corrigido nas regras da receita e do `client-dist` e publicado como versão 4.
Pré-condição: versão 4 publicada (`scripts/dev/publish-client-version.ps1`, 08/11); jogo fechado.
- [ ] Abrir o launcher da pasta `F:\Games\Vindictus1.69` → "Publicada: versão 4" → atualizar → baixa só a pasta `BlackCipher` (≈ 18 MB) → "Cliente na versão 4".
- [ ] Jogar → o jogo abre sem o aviso do BlackCipher → Start Game → seleção → cidade.
Relato: a mensagem do launcher e, se o aviso voltar, o conteúdo de `BlackCipher\NGClient.log` da pasta instalada.

### LV11 — atualização obrigatória no outro PC (launcher 0.4.0, 08/11/2026)
Pedido do operador: com atualização publicada, o launcher não deixa jogar; "Jogar" vira "Atualizar" (`docs/future/game-launcher.md` 4.10).
Pré-condição: Docker reconstruído (`COMPOSE_PARALLEL_LIMIT=1 docker compose build`, depois `docker compose up -d --wait`); launcher 0.4.0 publicado para a autoatualização (`scripts/dev/package-launcher.ps1 -PublicHost <ip>`); no outro PC, o launcher 0.3.0 com o cliente instalado.
- [ ] Abrir o launcher do outro PC → ele baixa o 0.4.0 e reabre sozinho (sem login aberto) → título ou mensagem com a versão 0.4.0.
- [ ] Com o cliente na versão publicada → botão "Jogar" → entra no jogo normalmente.
- [ ] Publicar uma versão nova pelo painel (LV12) com o launcher do outro PC aberto → em até 30 s o botão grande vira "Atualizar"; clicar baixa só o arquivo mudado → "Cliente na versão N" → o botão volta a "Jogar".
- [ ] Publicar outra versão e clicar em "Jogar" antes dos 30 s → o jogo **não** abre; a mensagem diz "Há uma versão nova do cliente (N): clique em Atualizar antes de jogar" e o botão vira "Atualizar".
- [ ] Uma pasta copiada à mão (sem `.vindictus-update`) → o botão diz "Atualizar"; clicar confere tudo e baixa só a diferença; depois joga.
Relato: a versão de cada passo, o texto do botão e, se o jogo abrir desatualizado, o `estado.json` de `.vindictus-update`.

### LV12 — versão nova pelo painel, por arquivo (08/11/2026)
Pré-condição: Docker reconstruído (o painel tem a seção "Próxima versão"); conta admin.
- [ ] Painel → Versões do cliente → "Adicionar ou trocar arquivo" → escolher um arquivo pequeno (por exemplo uma `nmcogame.dll` nova, ou um `.txt` de teste em `cfg/lv12-teste.cfg`) → aparece no rascunho como "novo" ou "alterado", origem "manual".
- [ ] "Remover arquivo" → buscar `lv12` → marcar a remoção de um arquivo de teste publicado antes → aparece como "removido".
- [ ] "Publicar a versão N+1" com título → a lista de versões mostra N+1 com "atualizar da anterior" de poucos KB e "1 removidos"; o `Vindictus1.69EN` não foi usado (a publicação leva segundos).
- [ ] No launcher: atualizar baixa só esses arquivos e apaga o removido.
Relato: o tamanho de "atualizar da anterior", o tempo da publicação e qualquer recusa (a mensagem vem do servidor, por exemplo "já está publicado com este conteúdo").

### LV13 — importação única e "Gerar a partir do conteúdo publicado" (08/11/2026)
Pré-condição: espaço livre para ≈ 29 GB em `${CLIENT_SERVER_DIR}` (padrão `.cache/client-server`); Docker em execução; `tools/nmcogame/build.ps1` rodado.
- [ ] `.\scripts\dev\import-client-base.ps1` (≈ 15 min de cópia e ≈ 25 min de montagem; logs em `.cache/verification/client-builder/`) → "Pronto" → `docker compose up -d --wait client-builder`.
- [ ] Painel → Versões do cliente → o rascunho mostra o que a montagem no contêiner difere da versão publicada (esperado: nada, ou só o conteúdo publicado no painel depois dela). Se aparecerem muitos arquivos, relatar a lista antes de publicar.
- [ ] Mudar o preço de um item de loja no painel e publicar o conteúdo (manutenção B4.4c) → "Gerar a partir do conteúdo publicado" → em alguns minutos o rascunho traz `sql/heroes.db3` alterado (origem "conteúdo do painel") → publicar → atualizar pelo launcher → o preço novo aparece na loja do NPC.
- [ ] Clicar de novo em "Gerar a partir do conteúdo publicado" sem mudar nada → rascunho vazio (a reaplicação não muda bytes). Se `sql/heroes.db3` voltar como alterado, relatar (hipótese de não determinismo do SQLite).
Relato: a mensagem do construtor (linha "Construtor do cliente" do painel) e, em falha, `build-state/refresh.log` ou `seed.log` da pasta do cliente do servidor.
