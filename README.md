# Vindictus — servidor de preservação em Rust

Migração gradual do servidor legado .NET para Rust, PostgreSQL e Docker, para estudo e manutenção. **A migração está incompleta e nenhum cliente Vindictus consegue autenticar no servidor Rust atual.**

O projeto já oferece infraestrutura executável, HTTP operacional, contas de laboratório, formatos de protocolo recuperados, sessões internas e parte da persistência e das regras de login. O plano e o status verificável de cada etapa estão em [MIGRATION.md](MIGRATION.md). Código descompilado ou testes de infraestrutura não equivalem a um servidor de jogo completo.

## Executar em Docker

Requisitos: Docker Desktop com containers Linux, Docker Compose e PowerShell 7. Execute os comandos na pasta deste README.

~~~powershell
# Apenas na primeira execução. Se .env existir, preserve-o e pule este comando.
pwsh -File scripts/dev/init-env.ps1
docker compose up -d --build --wait server
Invoke-RestMethod http://127.0.0.1:8080/health/ready
Invoke-RestMethod http://127.0.0.1:8080/capabilities
docker compose ps
~~~

O Compose inicia PostgreSQL 17, aplica migrações e inicia o serviço Rust. A primeira compilação pode demorar. Não é necessário instalar Rust para executar a imagem Docker.

| Componente | Endereço padrão | Função |
| --- | --- | --- |
| HTTP Rust | 127.0.0.1:8080 | Readiness e capacidades operacionais |
| PostgreSQL | 127.0.0.1:5433 | Banco do servidor Rust |
| SQL Server opcional | 127.0.0.1:14333 | Referência legada de estudo |
| Diagnóstico TCP separado | 127.0.0.1:27015 | Handshake e recusa de login; não publicado no Compose |

As portas locais podem ser ajustadas em .env. Preserve credenciais e volumes. O processo Rust usa UID/GID 10001; o papel PostgreSQL da aplicação não possui privilégios de superuser, criação de bancos/papéis ou replicação. Hoje o mesmo papel aplica migrações; separar migrador e runtime é uma melhoria prevista antes de exposição pública.

~~~powershell
# Parar e voltar a iniciar, conservando os dados.
docker compose stop
docker compose up -d --wait server
# Depois de alterar código ou migrações.
docker compose up -d --build --wait server
~~~

Os volumes Docker guardam o banco. Não use down -v para parar: a opção remove os volumes. Um clone Git não contém os dados importados desta máquina; executar o Compose em outra máquina cria um banco novo. As migrações criam schema, sem restaurar .bak ou importar dados privados automaticamente.

## Criar uma conta de laboratório

~~~powershell
pwsh -File scripts/dev/create-account.ps1 -Username pesquisador
~~~

A senha é solicitada sem aparecer na tela e enviada ao CLI por stdin. O nome aceita 3–32 letras ASCII, números ou sublinhado, sem distinguir maiúsculas. A senha aceita 12–128 bytes UTF-8; armazenamento em Argon2id com salt aleatório.

**Essa conta ainda não permite entrar no jogo.** Ela não cria um passport legado nem ativa as credenciais dos usuários importados. Não há endpoint HTTP de cadastro/login de jogo.

## Cliente e endereço IP

O candidato identificado é o cliente legado **Vindictus 1.69 EN (2016)**; nove arquivos relevantes coincidem entre as duas cópias locais examinadas. A versão atual oficial não está comprovada como compatível.

A preparação de endereço é feita somente numa cópia do cliente, pela ferramenta scripts/client/prepare_lab_client.py: altera FeatureMatrix.EndPoints numa cópia SQLite e gera EndPoint.txt com backup. Veja parâmetros e limitações em [docs/client-compatibility.md](docs/client-compatibility.md). O IP do HTTP operacional não substitui o endereço frontend de jogo. Precedência e aceitação pelo cliente continuam pendentes.

Com Rust instalado, o diagnóstico separado pode ser executado:

~~~powershell
cargo run --locked -- probe-frontend --bind 127.0.0.1:27015
~~~

Ele envia a tabela recuperada de mensagens e recusa todos os logins. Não inicia autenticação, personagens ou batalha. As instruções definitivas de cliente/IP/criação de conta de jogo serão documentadas após o aceite real M24.

## Desenvolvimento e testes

Requisitos adicionais: Rust 1.98.0 com rustfmt/Clippy, PowerShell 7 e Python 3.13 via py no Windows. rust-toolchain.toml fixa o toolchain; Cargo.lock fixa dependências.

~~~powershell
# PostgreSQL já deve estar rodando.
$env:TEST_DATABASE_URL = (Get-Content .cache/database-url -Raw).Trim()
try {
    pwsh -File scripts/dev/verify.ps1 -DatabaseIntegration
} finally {
    Remove-Item Env:TEST_DATABASE_URL
}
pwsh -File scripts/dev/smoke-docker.ps1
~~~

O gate executa formato, Clippy com avisos tratados como erro, testes Rust, doctests, build, integrações PostgreSQL e testes Python. As integrações usam schemas temporários isolados. O modo sem -DatabaseIntegration não executa os testes de banco.

O smoke reinicia apenas PostgreSQL/servidor deste projeto e preserva contas e snapshots sintéticos como evidência. Logs completos ficam em .cache/verification; o terminal recebe resumos. Resultados ficam em [relatório](docs/evidence/verification/REPORT.md), gate.json e docker-smoke.json. O workflow .github/workflows/verify.yaml executa verificação em CI com PostgreSQL efêmero.

Em outro sistema, configure TEST_DATABASE_URL com o banco de teste local e use os comandos Cargo/Python do workflow. Não publique URLs de conexão ou exportações privadas.

## Arquitetura

Monólito modular com limites explícitos:

| Pasta | Responsabilidade |
| --- | --- |
| src/domain | Tipos, invariantes e regras recuperadas |
| src/application | Casos de uso e coordenação de sessões/importação |
| src/infrastructure | PostgreSQL e transporte |
| src/protocol | Framing, cifra, mensagens e identidade legada |
| src/http | Operação e diagnóstico HTTP |
| migrations | Schema PostgreSQL incremental |
| tests | Testes diferenciais, integração e fixtures sintéticos |
| scripts | Docker, importação, engenharia reversa e cliente de laboratório |
| docs | Contratos, decisões, evidências sanitizadas e limitações |

SQL Server permanece referência; PostgreSQL é o alvo novo. Datas conservam precisão SQL, strings problemáticas ficam em bytes UTF16, rowversion é preservado separadamente da revisão operacional. Protocolo de jogo é separado de HTTP. Não inventar opcodes, relógios, flags ou observações ausentes.

Detalhes: [AGENTS.md](AGENTS.md), [arquitetura](docs/architecture.md), [banco](docs/database-decision.md), [protocolo](docs/protocol.md) e [skills](docs/skills.md).

## O que já foi implementado

- Inventário com hashes e recuperação estática de 95 assemblies managed, sem executar serviços originais.
- Restauração preservada de seis backups SQL Server, CHECKDB e catálogo de dependências.
- Base Rust/Docker/PostgreSQL, migrações, readiness, CLI de contas e CI.
- Framing Devcat, ISAAC Core/Heroes, tabela de 649 mensagens e subconjunto de codecs comparados a oracles recuperados.
- Diagnóstico TCP loopback, identidade legada e preparação de uma cópia de cliente.
- Persistência dos 15 campos de User; 29 usuários reais importados privadamente e comparados após reinício, sem ativar credenciais.
- Políticas de ban/IPLock e quantização de relógio, sessões internas e exclusividade de usuário no processo.
- LimitedAccount, contrato interno de UserLogin e planejador de dormência/retorno/recompensas.
- Snapshots completos de lealdade/achievements e executor restrito de efeitos de conta com transação, revisão e replay idempotente.
- Preservação dos 64 campos de personagens, conexões e fila legada, com consultas de lista/última conexão: [contrato e importação](docs/character-login-storage.md).
- Criação de entradas de recompensas de retorno, identity explícito e replay durável: [contrato e inicialização](docs/dormant-item-queue.md). Entrega de itens ainda pendente.

Cada afirmação tem escopo e evidência em MIGRATION.md. O executor restrito continua recusando dependências não migradas. O novo executor de contas existentes insere recompensas na fila com transação, revisão e recibo; não está ligado ao cliente. Números de testes atualizados ficam no relatório.

Contratos: [persistência](docs/legacy-user-storage.md), [importação](docs/user-import.md), [políticas](docs/login-policy.md), [sessões](docs/frontend-session.md), [ownership](docs/session-registry.md), [LimitedAccount](docs/limited-account-return.md), [planejador](docs/login-effects.md) e [efeitos](docs/login-account-effects.md).

## Plano restante

| Etapas | Resultado esperado |
| --- | --- |
| M11–M13 | Frontend/autenticação completos, UserLogin, token, ownership e login real |
| M14–M15 | Dependências/personagens e criação/lista/seleção/exclusão com persistência |
| M16 | Inventário, equipamentos, moedas, atributos e operações atômicas |
| M17 | Conteúdo SQLite, classes, skills, quests, NPCs, receitas e mapas |
| M18 | Lobby, canais, posição, chat, grupos, convites e reconexão |
| M19 | Transporte nativo de batalha/ProudNet/relay/host com compatibilidade comprovada |
| M20 | Missões, combate, drops, recompensas e persistência idempotente |
| M21 | Postal, trade, guild, ranking, cashshop e demais serviços necessários |
| M22 | Administração, backups, métricas, auditoria e limites de recursos |
| M23 | Paridade, falhas, reconexão, integridade e carga comparadas ao legado |
| M24 | Aceite com cliente: login, personagem, batalha, recompensa e reconexão |

A política de recompensas agora possui exportador privado, qualificação por personagem e diagnóstico Rust/Docker somente leitura, comparados ao Framework. Consulte [política de recompensas](docs/dormant-reward-policy.md) para executar a comparação. Consultas reais, política e recibos estão integrados ao executor interno de contas existentes. A próxima ação exata está em MIGRATION.md. Processamento, inventário e correio ainda pendentes.

Depois da compatibilidade básica, o projeto poderá estudar novas armas, fases e classes. Isso exige regras do servidor e dados/assets/animações/mensagens suportados pelo cliente. Extrair uma classe atual não garante que o cliente de 2016 consiga carregá-la; formatos, executável e protocolo podem exigir adaptação. Essa extensão não é uma etapa concluída.

## Referência SQL Server e engenharia reversa

O serviço Rust normal não depende de SQL Server. Para restaurar/comparar o legado, mantenha os arquivos obtidos legitimamente fora deste repositório, na estrutura local esperada:

~~~
VindictusPrivate/
  vindictus-server/       # este repositório Git
  ServidorVindictus/      # referência original, somente leitura
    database/            # .bak
  Client/                # cliente original
  Vindictus1.69EN/        # outra cópia de referência
~~~

~~~powershell
docker compose --profile legacy up -d --wait legacy-sqlserver
# Exemplo: escolha nomes reais; não substitua bases existentes.
pwsh -File scripts/database/Restore-Legacy.ps1 -BackupName heroes.bak -DatabaseName study_heroes
~~~

Consulte [migração de banco](docs/database-decision.md) e parâmetros dos scripts antes de restaurar/catalogar. Scripts de oracle exigem catálogos locais e ferramentas .NET/ILSpy; não são requisitos de execução normal do servidor nem de testes com fixtures já versionados. Catálogos SQL completos podem conter código/dados sensíveis e ficam ignorados pelo Git. Evidências compartilháveis contêm metadados, hashes, contagens e fixtures sintéticos.

## Git e continuidade

O repositório cobre apenas esta pasta. .gitignore exclui credenciais, caches, exportações privadas, backups, clientes/binaries, fontes descompiladas, catálogos SQL brutos e artefatos de compilação. .gitattributes normaliza textos em LF e .editorconfig registra convenções.

Ao terminar cada etapa: executar o gate completo, verificar o escopo da entrega, atualizar MIGRATION.md e evidências sanitizadas, revisar o diff e criar um commit local com código/testes/documentação juntos. Commits não implicam publicação em remoto. Backups do banco e originais são mantidos separadamente do Git.



O executor interno de contas existentes agora reúne as observações de dormência/lealdade, o catálogo de recompensas e a inserção da fila em uma transação com CAS e recibo UUID. Consulte [o contrato e os limites](docs/login-reward-executor.md). A integração à sessão de jogo e a entrega por inventário/correio continuam pendentes; ainda não há login de cliente disponível.
