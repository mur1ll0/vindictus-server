# B4 — plano do painel de administração e da análise de extensões

Rascunho de 25/10/2026, para aprovação do operador. Nada foi implementado. As decisões abertas estão na seção 6; os passos só começam depois das respostas.

## 1. Diagnóstico: contas e login hoje

### 1.1 O que o cliente envia e o que o servidor faz

| Etapa | Onde | O que acontece |
|---|---|---|
| Cliente | `.cache/play-client/start.bat` (`vindictus.exe -stage -dev -lang en-eu-x-gm -noupdate -threads 4`, `docs/client-compatibility.md:14`) | O operador digita um identificador na tela de login. O cliente envia `UserLoginMessage`. O único campo de identidade é `passport` (`src/protocol/login.rs:9-11`). Não existe campo de senha no contrato; os outros campos são endereço, HwID, MachineID, NexonID declarado etc. |
| Original, ramo `NGM_disable` | `FrontendServiceCore.States/PassportCheck.cs:146-215` | Passport não vazio → `NexonID = passport[..24]`, `NexonSN = passport.ToLower().GetHashCode()`, sem nenhuma verificação de segredo. Concede `LoginWhileMaintenance`, `LoginWithoutHackshield` e `Cheat` (linha 215). |
| Original, ramo normal | `PassportCheck.cs:260` → `LoginServiceCore.Processor/CheckPassportProcessor.cs` | `CheckPassport` valida um token de sessão Nexon pelo NMLink, que é nativo e não foi recuperado. Não dá para usar no laboratório. |
| Original, passport de desenvolvedor | `LoginServiceCore/LoginService.cs:198` (`TryCheckDeveloperPassport`, `DNS:<nome>`) | `EnableDeveloperPassport=False` no `ServiceCore.dll.config:260`. Não está ativo e não será usado. |
| Rust, lista estática | `src/infrastructure/lab_runtime_config.rs:49` (`allowed_passports`), `:174-193` (1 a 8 passports; colisões de NexonSN e de nome recusadas), `:235` (`permits`, comparação ASCII sem caixa) | A lista vem do perfil privado criado por `scripts/dev/prepare-lab-frontend-profile.ps1` (stdin, ACL protegida, `.cache/private-frontend/runtime-*.json`). No Docker ela entra como o secret `lab_runtime_profile` (`compose.yaml`, serviço `game`). Trocar a lista exige gerar o perfil de novo e reiniciar o `game`. |
| Rust, porta de entrada | `src/infrastructure/lab_frontend.rs:484` | `permits(passport)` roda antes de qualquer efeito SQL. Recusa → `LoginRejection::PermissionFailed`. |
| Rust, identidade | `src/protocol/identity.rs:87-112` (`derive_lab_identity`) | Reproduz o ramo `NGM_disable`: ASCII 32–126, `nexon_id` truncado em 24, `nexon_sn` = hash do .NET Framework (x86/x64) do passport inteiro em minúsculas. As permissões amplas do original **não** são concedidas (`lab_frontend.rs:497-502`: `allow_cheat: false`). |
| Rust, primeiro login | `lab_frontend.rs:550` → `application/local_login_session.rs:57` (`evaluate_scoped_lab_user`) → `infrastructure/user_login.rs:29` (`prepare_scoped_lab_user`) | Sob `LOCK TABLE game_users` (`:58`), cria `game_users` (`:131`) e o marcador `game_lab_account_origins` com o fingerprint do perfil (`:135`, migração `0013`). Recusa usuário existente sem marcador (`:71-81`) e UID/nome que já tenham estado órfão (`:90-118`). Corresponde ao `UserLoginProcessor.cs:77` e `:102-113` do original (busca por nome; cria `dbo.User` se não existir). |
| Rust, contas HTTP | `migrations/0001_foundation_accounts.sql` (`accounts`, argon2id), `src/application/accounts.rs:11`, `scripts/dev/create-account.ps1` | Conta com hash de senha num esquema separado. **Nada liga essa conta ao login do jogo** (`docs/client-compatibility.md:122`). O HTTP operacional (`src/http/mod.rs:14`) só tem health e capabilities. |
| Rust, bloqueio | `migrations/0004_login_policy.sql:19` (`game_user_bans`) | A tabela recuperada do `CheckBanType` já existe e o login já a consulta (mensagem de ban do cliente, com motivo se `NotifyBanReason`). |
| Rust, segunda senha | `src/application/second_password.rs:49-67` | A segunda senha (feature `SecondPassword`, com `SecondPasswordClear`) é o único segredo que o fluxo original tem no ramo `NGM_disable`. |

**Por que o operador não consegue criar uma conta jogável:** a única coisa que libera o login é a lista de até 8 passports no perfil privado. O `create-account` grava em `accounts`, que o login não consulta. Hoje uma conta nova exige gerar um perfil novo com mais um passport, reiniciar o `game` e entrar com esse passport. O `game_users` é criado no primeiro login.

### 1.2 Consequência para o desenho

No modo de laboratório o passport é ao mesmo tempo o nome da conta (`NexonID`) e a credencial. Isso vem do original. Hoje o operador entra com `lab169_a` sem senha. Decisão do operador (25/10/2026, D1): o jogo passa a exigir login e senha; a segunda senha continua no jogo; o painel exige só login e senha.

### 1.2a O que o cliente faz com a senha (desmontagem pontual, 25/10/2026)

- `client.dll` 0x3826ae19 (`LoginDialog`, botão Login): lê `LoginDialog.Frame_Login.InputText_ID.text` e `...InputText_Password.text` (até 0x100 cada), monta `TransitionArgs {ID, Password}` e dispara `LoginDialog_OK`.
- `client.dll` 0x38522400: chama a interface de login 0x38594900 (virtual, `[[0x38cd2f80]+0x10]`, slot 3) com `(ID, Password)`. Se `NGM_disable` estiver ativo, grava **só o ID** em 0x38e5d9c8, que é o passport. O `UserLoginMessage` recuperado não tem campo de senha (`src/protocol/login.rs:9-23`). Portanto, pelo caminho da tela de login, **a senha digitada não chega ao servidor**. O destino interno do slot 3 não foi seguido.
- `engine.dll` 0x422f8973/0x422f8aee/0x422f8e05: o argumento de linha de comando `-passport <valor>` existe (fluxo do launcher oficial). Com ele, o engine chama o NMManager (`[0x4286ad5c]`, slot 3) com o valor e, na falha, mostra "NMManager - Warning". Não se sabe se esse caminho funciona com `NGM_disable` sem os serviços Nexon; precisa de um teste no cliente (T2, abaixo).
- `Vindictus.exe` também contém `-passport`. O `Windictus.exe` (2023) não tem strings ASCII legíveis com "passport" (provavelmente empacotado); não foi investigado.
- Logs: `.cache/verification/dis-login*.txt`, `dis-engine-passport.txt`, `client-passport-strings.txt`.

Conclusão: sem alterar o binário do cliente, a senha só pode ser verificada **fora** do diálogo de login, antes de o cliente conectar. É o que o original faz no ramo normal: o launcher autentica login e senha, recebe um passport de sessão e abre o cliente com `-passport`. O frontend troca o passport pela sessão (`CheckPassport`, `PassportCheck.cs:260`; `CheckPassportProcessor`: passport → `nexonSN`/`nexonID` da sessão).

### 1.3 Proposta: conta jogável com login e senha

```
launcher (scripts/dev/play.ps1 ou página /jogar do painel)
  login + senha ──► POST /api/play/session ──► argon2 em accounts ──► passport de sessão
                     (limite de tentativas)                            aleatório, uso único, expira em 2 min
  Vindictus.exe ... -passport <token>     (ou, se T2 falhar: digitar o código no campo ID)
                                                ◄── UserLogin(passport = token)
  lab_frontend: PassportAuthority.redeem(token) → conta → NexonID = username, NexonSN derivado do username
                (como o CheckPassport: a identidade vem da sessão, não do texto do passport)
  → prepare_scoped_lab_user (como hoje) → segunda senha (já existente) → lista de personagens
```

- O passport de sessão substitui o passport permanente: o token não fica em `game_users`. A identidade (`NexonID` = username, `NexonSN` = o mesmo hash do Framework sobre o username em minúsculas) continua estável. As contas atuais derivadas de `lab169_a` mantêm o mesmo UID, porque o username `lab169_a` gera o mesmo `NexonSN`.
- Armazenado: `game_play_sessions(token_sha256, account_id, expires_at, redeemed_at, peer)`. Uso único, expira em 2 minutos.
- Se T2 mostrar que `-passport` não chega ao `UserLogin` com `NGM_disable`, o fallback é a mesma página/script mostrar um código de uso único (12 caracteres) para digitar no campo ID da tela de login, com a senha em branco. A segurança é a mesma; só muda a ergonomia.
- Testes no cliente necessários antes do B4.2 (sem mudar o servidor):
  - **T1:** na tela de login, `lab169_a` com uma senha qualquer → entra normalmente (confirma que a senha é ignorada).
  - **T2:** fechar o cliente e abrir pela linha de comando `vindictus.exe -stage -dev -lang en-eu-x-gm -noupdate -threads 4 -passport lab169_a` na `.cache/play-client` → anotar se pula a tela de login e entra, se mostra a tela de login, ou se aparece "NMManager - Warning".

Resultado (25/10/2026): T1 entrou com uma senha qualquer (a senha é ignorada); T2 mostrou "Erro [Auth] SSO" e fechou. Implementado o código de uso único digitado no campo ID (B4.2b); a conta `admin` foi ligada ao usuário de jogo `lab169_a` (Fiona Murael) com `account-link-game`.

### 1.3b Detalhes da conta (válidos nas duas variantes)

- **Criar conta (painel):** `create_playable_account(username, senha)`. O username segue `[A-Za-z0-9_]{3,24}`, para caber inteiro no `NexonID`. A senha é guardada com argon2id na tabela `accounts` (já existente; o formato atual é 3–32 caracteres, então o limite cai para 24). O caso de uso deriva `NexonSN` e `name_key` com o mesmo `derive_lab_identity` e recusa colisão (hash de 32 bits) ou nome já usado em `game_users` ou nas tabelas que o `prepare_scoped_lab_user` confere como órfãs ("escolha outro nome"). Conta e auditoria vão na mesma transação. O usuário do jogo continua nascendo no primeiro login.
- **Login no jogo:** `lab_frontend.rs:484` troca `profile.permits` pela porta `PassportAuthority::redeem` (application; PostgreSQL em infrastructure). Ela consome o token (UPDATE ... `redeemed_at IS NULL AND expires_at > now()` RETURNING) e devolve a identidade da conta. Recusa → `PermissionFailed`, sem efeito SQL, como hoje.
- **Senha errada no launcher:** HTTP 401 genérico, limite por IP e por conta, bloqueio temporário depois de 10 falhas. Nada chega ao TCP do jogo.
- **Perfil privado:** `format_version: 2`, com `allowed_passports` opcional (0 a 8) só como transição (D2), removida no gate. Com a lista vazia, só entra quem tem token válido.
- **Contas existentes:** `admin-import-account --username lab169_a --password-stdin` (host) cria a conta para o `game_users` já marcado. O UID não muda, porque o `NexonSN` vem do mesmo username. A Fiona atual continua jogável, agora com senha.
- **Iniciar o cliente:** `pwsh scripts/dev/play.ps1 -Username lab169_a`. O script pede a senha sem eco, chama `POST /api/play/session` e abre a `.cache/play-client` com os argumentos do `start.bat` mais `-passport <token>`. Fallback, se T2 falhar: o script mostra o código e você o digita no campo ID. Depois, a segunda senha do jogo, como hoje.
- **Revogar acesso:** desativa a conta (`accounts.status = disabled`, motivo obrigatório). Os tokens pendentes são invalidados e, se a personagem estiver conectada, a sessão é derrubada pelo registro de sessões.
- **Bloquear:** grava em `game_user_bans` (prazo e motivo), o caminho que o cliente original sabe exibir. Desbloquear remove a linha.
- **Redefinir credenciais:** (a) senha da conta: o painel define uma nova e invalida tokens e sessões; (b) segunda senha: limpa a do jogo e o cliente pede uma nova (`SecondPasswordClear`).
- **Criação de personagem:** continua o caminho mínimo de Fiona.

## 2. Arquitetura

### 2.1 Onde o painel roda

**Proposta (D3): API de administração num listener próprio dentro do processo `lab-frontend`** (serviço `game`), em `127.0.0.1:27090`. No compose, a porta é publicada só em `127.0.0.1`. O HTTP do jogo continua separado: o TCP do jogo fica em 27015/27003/27005, a loja web em 27080 e o `serve` em 8080 continua sendo health/capabilities.

Justificativa: o ponto único de autoridade sobre sessões vivas está nesse processo. São `lab_frontend::Runtime` (`lab_frontend.rs:98-107`: registro de sessões, `web_shop::Sessions`, canal), `channel_server::Runtime::broadcast_from` (`channel_server.rs:109`) e o `is_connected` que o `lab_grant` confere dentro da transação (`minimum_give.rs:579-587`). A loja web (`lab_web_shop.rs`, 27080) já é um listener axum nesse mesmo processo e serve de precedente. No B5 o multijogador continua nesse processo, então o painel não precisa ser reescrito.

Alternativas descartadas:
- **Painel no `serve` (8080) gravando no banco:** não alcança as sessões. Uma mudança com o personagem online correria contra o estado em memória da conexão, e "desconectar" seria impossível.
- **Serviço separado com canal RPC para o jogo**, como o daemon do pw-universal (`web-admin/backend/painel/canal.py`, HMAC com desafio): exige mais um protocolo, mais autenticação e mais um container para uma única máquina. Pode voltar a ser útil se o jogo tiver vários processos.
- **Backend em Python/FastAPI como o pw-universal:** duplicaria as regras de domínio que já estão em Rust.

Custo aceito: o painel cai quando o `game` reinicia (por exemplo, ao aplicar conteúdo). A SPA mostra "servidor reiniciando" e reconecta sozinha.

### 2.2 Camadas e módulos

| Camada | Módulo novo | Conteúdo |
|---|---|---|
| domain | `domain/admin.rs` | `Role` (Admin, Gm, Leitura) e permissões por ação, `OperatorName`, `AuditReason` (1–500 caracteres, obrigatório em mudanças), `OperationId` (idempotência), `PassportPolicy` (D1). |
| domain | `domain/content_edit.rs` | `TuningPoint` (tabela, colunas-chave, coluna, tipo, faixa válida, consumidores servidor/cliente), gerado da lista de `scripts/content/content_overrides.py` e de `docs/content-tuning.md`; `EditSet` com versões. |
| application | `application/admin/{session,accounts,characters,shops,content,audit}.rs` | Casos de uso. Cada mutação recebe operador, motivo e `OperationId`, chama as funções de infraestrutura que o jogo já usa e grava a auditoria na mesma transação. |
| application | `application/passport_authority.rs` | Porta `PassportAuthority` usada pelo login. |
| infrastructure | `admin_store.rs`, `play_sessions.rs`, `content_edits.rs`, `admin_live.rs` | SQL preparado; `admin_live` é a fachada sobre o `Runtime` (sessão viva, desconectar). Variantes `*_in_tx` das funções existentes (`minimum_give::lab_grant`, `minimum_cash_shop::credit` etc.), para a auditoria entrar na mesma transação. As regras de domínio continuam as mesmas. |
| http | `http/admin/{mod,auth,csrf,limits,routes/*}.rs` | Router axum, extratores `RequireRole<R>`, cookie, CSRF, limite de corpo (8 KiB padrão), timeout, OpenAPI. |
| frontend | `web/admin/` | SPA. |

Regra: o painel **nunca** grava direto em tabela de jogo. Só os casos de uso tocam o banco, e eles reaproveitam `minimum_give`, `minimum_cash_shop`, `minimum_status_effect`, `game_user_bans`, `second_password` etc. Onde a função atual só serve à "primeira Fiona" (`lab_grant` escolhe a única personagem inicializada quando `cid` é `None`, `minimum_give.rs:530-540`), o passo generaliza para um `cid` explícito, conferindo o dono.

### 2.3 Contrato

- OpenAPI 3.1 gerado do código com `utoipa` + `utoipa-axum`. O comando `vindictus-server export-openapi` grava `web/admin/openapi.json` (versionado). Um teste Rust falha se o arquivo commitado divergir do gerado.
- Cliente tipado: `openapi-typescript` (tipos) + `openapi-fetch` (chamadas), regenerados por `npm run gen:api`.
- Erros: `application/problem+json` com `code` estável (`precisa_estar_offline`, `conflito_de_operacao`, `nome_em_uso`, `colisao_de_identidade`, `sessao_encerrada`, `csrf`, `papel_insuficiente` etc.).
- Toda mutação leva `operation_id` (UUID do cliente). Repetir o mesmo `operation_id` devolve o resultado gravado e não aplica a mudança de novo (padrão do pw-universal, `contas.py:319-343`).

### 2.4 Autenticação e segurança

- **Mesma conta do jogo com papel (D5, decisão do operador de 25/10/2026):** a conta de `accounts` (login + senha argon2id) ganha `panel_role` (nulo = sem acesso ao painel; `admin`, `gm` ou `leitura`), `failed_attempts`, `locked_until` e `password_changed_at`. O painel nunca pede a segunda senha. Ainda não existe criação de conta pelo painel, então o primeiro admin vem do CLI: `create-account --username X --password-stdin` (já existe) e `account-set-role --username X --role admin` (novo, sem argumento de senha), pelos scripts `create-account.ps1` e `account-set-role.ps1`. Não há credencial padrão. Só um admin pode mudar papéis pelo painel; a última conta admin não pode ser rebaixada.
- **Sessão:** token aleatório de 32 bytes no cookie `vindictus_admin` (HttpOnly, SameSite=Strict, Path=/; Secure configurável, porque `http://127.0.0.1` não é HTTPS). Só o SHA-256 do token vai para `admin_sessions`, com expiração absoluta de 8 h, ociosa de 30 min e a impressão do hash de senha. Trocar a senha ou desativar o operador invalida as sessões, como o `seguranca.py:227-245` do pw-universal.
- **CSRF:** token sincronizador por sessão, devolvido em `GET /api/session` e exigido em `X-CSRF-Token` nas mutações, mais checagem de `Origin`/`Sec-Fetch-Site` (mesma origem). A SPA é servida pelo mesmo listener, sem CORS.
- **Limites:** tentativas de login por IP (20/min) e por nome (10/min) em memória no processo; bloqueio persistente do operador depois de 10 falhas seguidas (15 min). A verificação argon2 roda em `spawn_blocking` com um semáforo de 2, para não saturar a máquina.
- **Papéis:** Leitura = GET; GM = contas, personagens, lojas (rascunho); Admin = tudo, inclusive papéis, publicação de conteúdo e revogação.
- **Logs:** nenhum token, senha, passport ou CSRF em log. Os tipos com segredo não implementam `Debug`/`Serialize`, como o `LabRuntimeConfig` já faz. Corpo limitado antes da leitura.

Nota (B4.1): as tabelas do painel usam prefixo (`admin_*`, `content_*`) em vez de esquemas próprios, porque os testes PostgreSQL isolam cada execução por `search_path` num esquema aleatório; um esquema fixo seria compartilhado entre execuções.

### 2.5 Auditoria

`admin_audit_events` (identity bigint, `at` timestamptz, `account_id` (quem), `role`, `operation_id` único, `action`, `target_kind`, `target_id`, `reason` não nulo nas mudanças, `before` jsonb, `after` jsonb, `outcome`, `peer`). Gravada na mesma transação da mudança. A tabela é só de acréscimo: um trigger recusa UPDATE/DELETE. Login, logout e falhas de login também entram. A tela Auditoria filtra por operador, alvo, ação e período, com paginação por chave (id), sem OFFSET, e índices `(target_kind, target_id, id)` e `(operator_id, id)`.

### 2.6 Personagem online: regra por ação (D4)

| Ação | Regra no B4 | Motivo |
|---|---|---|
| Dar/remover/mover item, ouro, nível/EXP, AP, skills, títulos, status effects | Exige personagem **offline** (o `is_connected` é conferido dentro da transação, como no `lab_grant`). O painel mostra "Desconectar e aplicar". | Esses dados são carregados na seleção e mantidos pela conexão; gravar por fora diverge do estado em memória. |
| Cash (crédito/débito) | Permitido online se o roteiro confirmar que o Depot consulta o saldo ao abrir (`minimum_cash_shop::balance`); caso contrário, offline. | O saldo não fica em memória na sessão. |
| Desconectar | Online, pelo registro de sessões do `Runtime` (fecha a conexão com o cleanup normal). | É a autoridade da sessão. |
| Bloquear/revogar | Grava e, se online, desconecta. | — |
| Conteúdo (lojas, tuning) | Vale para todos após aplicar e reiniciar o `game`. | O conteúdo é carregado na partida. |

Entrega com o personagem online (mensagens de atualização enviadas pela sessão, como faria o GM do original) fica registrada como extensão do B5. Ela entra pela mesma fachada `admin_live` + uma caixa de comandos por conexão, no mesmo ponto único que o B5 usará para broadcast.

### 2.7 Conteúdo: edições versionadas e ciclo de aplicação (D6)

- Tabelas `content_edit_sets` (versão, estado `rascunho → publicado → aplicado`, autor, motivo) e `content_edit_rows` (tabela, chave jsonb, coluna, valor original, valor novo; ou linha adicionada/removida em `ShopInfo`), separadas do estado dos personagens.
- Validação na hora da edição: a tabela e a coluna precisam ser um `TuningPoint` conhecido, com o tipo e a faixa da coluna. A chave precisa existir no catálogo carregado. O painel mostra o valor original, o modificado e o novo lado a lado, como a tabela de `docs/content-tuning.md`.
- **Aplicar** (no host, porque o container só lê `.cache/private-content` e não alcança os SQLite originais nem a cópia do cliente): `scripts/content/apply-content-edits.ps1 -Version N`.
  1. `vindictus-server content-edits-export --version N` grava `.cache/private-content/content-edits.json`, a URL do banco vem do `.env`, como no `lab-give-item.ps1`.
  2. Os exportadores (`content_overrides.py` generalizado, `export_npc_scripts.py`, `export_item_*.py`, `export_cash_shop.py`) aplicam: originais → `lab_content_additions.json` → `content-edits.json`.
  3. Se algum ponto da versão é lido pelo cliente (coluna "Cliente" de `docs/content-tuning.md`: `ShopInfo`, `MaxStack`, `EnhanceInfo`, `EnchantInfo`, `TitleGoalInfo`…), `apply_lab_content_additions.py`/`apply_original_content.py` atualizam `.cache/play-client/sql/heroes.db3`, com backup e o cliente fechado.
  4. `docker compose restart game` (sem rebuild: o conteúdo é um volume).
  5. O painel compara o hash de manifesto que o `game` carregou com o da versão publicada e marca "aplicado". Se não bater, mostra "pendente".
- Desfazer = publicar uma versão que volta ao valor anterior. O histórico nunca é apagado.

### 2.8 Frontend

React 19 + TypeScript + Vite; TanStack Router (rotas tipadas) e TanStack Query (cache e invalidação depois das mutações); shadcn/ui (Radix + Tailwind) para formulários, tabelas e diálogos acessíveis; formulários com `react-hook-form` + `zod`. Telas, tomando o pw-universal como referência de organização: Entrar · Contas (lista, criar, detalhe com passports, sessões, bloqueio, histórico) · Personagens (resumo, inventário por armazenamento, dar/remover/mover, ouro, cash, progressão) · Itens (catálogo com nome e descrição em inglês) · Lojas (NPC e cash shop) · Ajustes (tuning) · Conteúdo (versões e aplicação) · Auditoria · Papéis. Toda mutação abre um diálogo com o motivo obrigatório e mostra antes/depois.

Testes (D7, decisão do operador): sem e2e e sem suíte de frontend. O operador testa o painel manualmente. O servidor tem só os testes unitários e PostgreSQL necessários para as regras novas (autenticação, autorização, auditoria, tokens, recusas). `tsc --noEmit` e o build do Vite já validam os tipos gerados do OpenAPI.

### 2.9 Deploy

- Dockerfile: um estágio `node:24-bookworm-slim` (versão fixada com digest) com `npm ci` e `npm run build`, cache de `~/.npm` por BuildKit; o runtime recebe `--link` de `web/admin/dist` em `/app/admin-ui`; `.dockerignore` exclui `web/admin/node_modules`. Rust serve os estáticos com `tower-http` (`fs`) e fallback para `index.html`.
- compose: o serviço `game` ganha `--admin-listen 0.0.0.0:27090` dentro do container e `127.0.0.1:27090:27090` no host. Sem secret novo: o painel usa o mesmo `DATABASE_URL` (role sem privilégios de cluster).
- Nativo (depuração): `start-lab-frontend.ps1 -AdminPort 27090`, e `npm run dev` com proxy `/api` para `127.0.0.1:27090`.
- Carga: `cargo` limitado pelo `.cargo/config.toml`; `npm` e `cargo` nunca ao mesmo tempo; Docker rebuild só no fechamento de B4.2, B4.4 e no gate.

## 3. Passos do B4

Cada passo tem testes focados (alvos Rust afetados e testes PostgreSQL filtrados em esquema isolado), commit próprio e atualização do `MIGRATION.md`. A suíte completa e o smoke Docker ficam para o gate.

### B4.1 — Login de admin e auditoria
- Migração `0042_admin_panel.sql` (feita): colunas de papel e bloqueio em `accounts`; `admin_sessions` e `admin_audit_events` (+ trigger só-acréscimo; `operation_id` único serve à idempotência). CLI e script `account-set-role`. Listener `--admin-listen`. Rotas `POST /api/session/login`, `GET /api/session`, `POST /api/session/logout`, `GET /api/audit`. OpenAPI e teste de divergência.
- SPA mínima: Entrar, layout, Auditoria. Instalação do `web/admin` (Vite, Router, Query, shadcn/ui).
- Testes: Rust (domínio de papéis/motivo, parse de cookie/CSRF, `RequireRole`); PostgreSQL `admin_login_*` (senha errada conta tentativas, bloqueio, expiração, sessão invalidada ao trocar a senha, CSRF ausente recusado, Leitura recusada em POST, UPDATE/DELETE de auditoria recusados, `operation_id` repetido)..
- Cliente: não necessário.

### B4.2 — Conta jogável com login e senha
- Pré-requisito: resultado de T1/T2 no cliente (seção 1.3), que decide entre `-passport` e o código digitado.
- Migração `0043_accounts_play.sql`: `accounts.status`, username até 24, `game_play_sessions`. Perfil `format_version 2`; `PassportAuthority::redeem` no login; `POST /api/play/session` (público em loopback, só login+senha, limite de tentativas); `scripts/dev/play.ps1`; `admin-import-account`. Rotas de contas no painel: criar, listar, detalhe, desativar, bloquear/desbloquear (`game_user_bans`), nova senha, limpar segunda senha, sessões ao vivo (registro), histórico (auditoria + `game_session_events`, tabela de laboratório, não legada) e desconectar.
- Telas: Contas.
- Testes (só os necessários): Rust, colisão de `NexonSN`/nome; PostgreSQL `play_session_*` (senha certa → token; errada → recusa e contagem; token de uso único e expirado recusados; conta desativada recusada; primeiro login com token cria `game_users` + origem com o UID derivado do username); um teste TCP do `lab_frontend` com token e lista vazia.
- Roteiro: entrada B4.2 em `docs/client-tests/B4-painel.md` (criar conta no painel, entrar pelo `play.ps1` com a senha, segunda senha, criar Fiona; senha errada recusada; desativar e ver a recusa; `lab169_a` importada entra com senha).
- Docker rebuild ao fim do passo (primeiro grupo).

### B4.3 — Personagens: ver, itens, ouro e cash
- B4.3a — Leitura: resumo (classe, nível, EXP, ouro, AP, online), inventário por armazenamento com atributos `classe[ATTR:valor]`, armazenamento pessoal, status effects, títulos, skills. Catálogo de itens com nome e descrição em inglês: `export_item_catalog.py` passa a ler `HEROES_ITEM_NAME_/HEROES_ITEM_DESC_` do `heroes_text_english_eu.txt`, como já faz `export_cash_shop.py:18-32`.
- B4.3b — Mudanças: dar item (ItemClassEx, `lab_grant` generalizado para `cid` explícito), remover item (o caminho de destruição que o jogo já usa, com journal), mover item entre bolsas/armazenamento (`minimum_move`), ouro +/−, cash crédito/débito (`minimum_cash_shop::credit`). Os scripts `lab-give-item.ps1` e `lab-give-cash.ps1` passam a avisar que o painel os substitui.
- Testes: PostgreSQL `admin_character_*` (dar com atributo, remover, mover, ouro, cash, recusa online sem efeito, auditoria antes/depois, idempotência)..
- Roteiro: B4.3 (dar item com atributo e ouro pelo painel, logar, conferir; tentar com a Fiona online → recusa; "Desconectar e aplicar").

### B4.3c — Progressão: nível/EXP, AP, skills, títulos, status effects
- Casos de uso sobre `minimum_growth`, `minimum_ap`, `minimum_skills`, a progressão de títulos e `minimum_status_effect` (VIP/VVIP com prazo). Offline obrigatório.
- Testes PostgreSQL `admin_progression_*`.
- Roteiro: B4.3c (subir nível, dar VIP 7 dias, conferir no cliente).

### B4.4 — Lojas de NPC e cash shop (e o ciclo de conteúdo)
- Migração `0044_content_edits.sql` (tabelas `content_*`); `content-edits-export`; `apply-content-edits.ps1`; `content_overrides.py` lendo o overlay.
- Lojas de NPC: ver `ShopInfo` (original + adições do laboratório + edições), adicionar/alterar/remover linha (item, preço, ordem, quest de aparição, limite), com a cópia do cliente atualizada por `apply_lab_content_additions.py`.
- Cash shop: categorias e preços (`cash_shop_prices.json` → edição), produtos VIP 7/30/90 dias, prévia do ShopList que o `lab-export-shop-list.ps1` gera.
- Testes: Python (`test_content_overrides`, `test_export_npc_scripts`, `test_export_cash_shop` com overlay), PostgreSQL `content_edit_*` (versão, validação de faixa/chave, publicação, só-acréscimo).
- Roteiro: B4.4 (editar o preço do Enhancement Elixir do Ferghus e acrescentar um item, aplicar, ver na loja; mudar o preço de uma categoria do Depot).
- Docker rebuild ao fim do passo.

### B4.5 — Valores ajustáveis do content-tuning
- Todos os pontos da tabela de `docs/content-tuning.md` (drops por fase, MaxPlayCount, MinLevel, EXP de história, TargetCount, MaxStack, ExpireIn, RequiredLevel, Enhance/Enchant, receitas) e as taxas do FeatureMatrix, com o consumidor de cada ponto mostrado na tela.
- Testes: Python (overlay por tabela), PostgreSQL de validação.
- Roteiro: B4.5 (mudar TargetCount de uma meta de título e o MaxStack de um item, aplicar, conferir o contador e a pilha).

### B4.6 — Mapas e canais (D8)
- Somente leitura: canal 1, membros, relay, portas e versão de conteúdo carregada. Nenhum controle novo, porque o laboratório tem um canal só.
- Cliente: não necessário.

### B4.7 — Relatório de viabilidade das extensões
- `docs/extension-feasibility.md`. Para cada tipo (equipamento/item, missão, fase, monstro e, como foco, **classe nova**): o que vive no conteúdo SQLite (por exemplo `CharacterClassInfo`, `ItemClassInfo`, `QuestInfo`, `MonsterInfo`, `MonsterDropItemInfo`), o que vive nos assets do cliente (`.hfs`: modelos, animações, mapas, textos localizados, ícones; `client.dll` nativo) e o que exige servidor. Ponto de partida para classes: `BaseCharacter` é um enum `byte` com `CHARACTER_COUNT = 10` (`ServiceCore.CharacterServiceOperations/BaseCharacter.cs`), usado em criação, skills, equipamento e PvP; o cliente decide por classe em código nativo.
- Método: leitura dos SQLite e da fonte recuperada; inspeção pontual de recursos e símbolos, sem análise do binário inteiro do `client.dll` (limite da máquina). Cada conclusão leva a evidência e um custo estimado (conteúdo / assets / servidor / binário do cliente).
- Cliente: não necessário.

### B4.8 — Pontos de extensão registrados e gate
- Seção "Pontos de extensão" deste documento atualizada com os contratos desenhados (abaixo) e nenhuma implementação.
- Gate: reiniciar o PostgreSQL de teste se o WAL estiver alto; `scripts/dev/verify.ps1 -DatabaseIntegration` e o smoke Docker (com `npm run build` do painel e a criação de conta pelo painel); roteiro `B4-painel.md` completo; commit de fechamento; sessão do operador: criar uma conta, ajustar a Fiona, editar uma loja, efeito no cliente.

## 4. Pontos de extensão (desenhados, não implementados)

| Módulo futuro | Contrato previsto | Dependência |
|---|---|---|
| Quests | `TuningPoint`s de `QuestInfo`/`QuestGoalInfo`/`QuestRewardInfo` + "dar/completar quest" por `minimum_quest::give` | ciclo de conteúdo do B4.4 |
| Bosses | edições de `MonsterInfo` (HP/EXP/dificuldade) por fase | relatório B4.7 |
| Missões | rotação/TodayMission quando o sistema for migrado | sistema ainda não migrado |
| Drops | `MonsterDropItemInfo`, `PropItemWeightInfo`, `NamedProp*` como edições de linha | ciclo de conteúdo |
| Mapas | estado por canal quando houver mais de um canal (B5) | B5 |
| Entrega online | caixa de comandos por conexão na fachada `admin_live` | B5 (mesmo ponto de broadcast) |

Cada um entra como uma rota OpenAPI nova, um caso de uso em `application/admin/` e um `TuningPoint`/tipo de edição. Auth, auditoria e ciclo de aplicação não mudam.

## 5. Ferramentas substituídas

| Hoje | No painel | Destino do script |
|---|---|---|
| `scripts/dev/create-account.ps1` | Contas → Criar (login + senha) | substituído por `admin-import-account`/painel; a conta criada passa a ser jogável |
| `prepare-lab-frontend-profile.ps1` (lista) | contas com senha no banco | mantido para o perfil; a lista vira opcional e sai no gate |
| `lab-give-item.ps1`, `lab-give-cash.ps1` | Personagens → Dar item / Cash | mantidos para depuração |
| `lab-export-shop-list.ps1` | Lojas → Cash shop (prévia) | mantido |
| `content_overrides.py`, `export_*.py`, `apply_original_content.py`, `apply_lab_content_additions.py` | chamados pelo `apply-content-edits.ps1` | passam a ler o overlay |

## 6. Decisões do operador

| # | Pergunta | Recomendação |
|---|---|---|
| D1 | Credencial do jogo | **Decidido (25/10/2026):** login e senha no jogo (launcher com passport de sessão, seção 1.3), mais a segunda senha no jogo; o painel exige só login e senha. Depende de T1/T2 no cliente para escolher entre `-passport` e o código digitado. |
| D2 | Lista estática do perfil | Manter como transição (importada para o banco no B4.2) e removê-la no gate. |
| D3 | Onde roda a API admin | No processo do jogo (`game`), listener 127.0.0.1:27090. |
| D4 | Personagem online | Exigir offline no B4, com "Desconectar e aplicar"; entrega online no B5. |
| D5 | Contas do painel | **Decidido (25/10/2026):** a mesma conta do jogo, com papel `admin`/`gm`/`leitura` em `accounts.panel_role`. |
| D6 | Onde ficam as edições de conteúdo | PostgreSQL (tabelas `content_*`) com export para `.cache/private-content`; o `lab_content_additions.json` versionado no git continua para adições permanentes. |
| D7 | Testes | **Decidido (25/10/2026):** sem e2e; só testes unitários/PostgreSQL necessários no servidor; o painel é testado pelo operador. |
| D8 | Mapas/canais | Somente leitura. |
| D9 | Ferramentas do frontend | npm (já instalado, Node 24) e shadcn/ui. |
| D10 | Ordem | B4.1 → B4.2 → B4.3a/b → B4.3c → B4.4 → B4.5 → B4.6 → B4.7 → gate. O relatório B4.7 pode subir para logo depois do B4.2, se as novas classes forem mais urgentes que o painel completo. |
