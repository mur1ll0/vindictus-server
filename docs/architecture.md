# Arquitetura e limites atuais

A implementação nova é um monólito modular em Rust, com domínio, aplicação, infraestrutura PostgreSQL, HTTP operacional e protocolo legado em módulos distintos. Essa separação mantém invariantes testáveis sem a complexidade de vários serviços distribuídos no primeiro marco.

`domain` valida nomes e senhas da conta nova; `application` coordena criação e hashing; `infrastructure` usa pool SQLx e migrações embutidas; `http` informa saúde e capacidades; `protocol` porta somente contratos confirmados por engenharia reversa. A criação de contas nova não substitui o passport do cliente.

O container inicia somente depois de PostgreSQL saudável e da migração executada com sucesso. A imagem usa build em várias etapas, Cargo.lock e usuário sem privilégios. Portas de estudo são publicadas apenas em loopback. A configuração local utiliza credenciais aleatórias em .env ignorado. O usuário do aplicativo PostgreSQL não é superusuário; separar também o usuário de migração do usuário de runtime será um marco posterior.

O SQL Server opcional é uma referência do legado: restaura cópias study_* dos backups e exporta catálogos e procedures. Não é o banco consumido pelo serviço Rust atual. Nenhum banco ou arquivo original é convertido automaticamente. PostgreSQL é o alvo da migração nova, sujeito a validação de tipos, collation, IDs, transações e regras T-SQL.

A compatibilidade de rede requer recuperar framing, cifra ISAAC, registro de categorias, serialização de tipos e máquina de estados de autenticação. O framing isolado não permite conectar o cliente. ProudNet e bibliotecas nativas exigem investigação própria. O comando serve inicia somente HTTP; probe-frontend inicia separadamente um diagnóstico TCP local de bootstrap e recusa. Esse adapter usa códigos recuperados e limites explícitos e não cria sessão de jogo. Identidade e candidato de INSERT permanecem módulos puros. application::game_users coordena validação e armazenamento inicial em infrastructure::game_users, sem ligação à autenticação do frontend. game_users mantém NexonSN e nome separadamente de accounts UUID; veja docs/game-user-persistence.md.

A migração deve preservar comportamento, não tentar transliterar chamadas Windows, LINQ-to-SQL e WinForms indiscriminadamente. Bibliotecas de terceiros e ferramentas administrativas são inventariadas separadamente dos serviços de jogo. Componentes desnecessários ao servidor Rust serão substituídos por ferramentas novas com resultados equivalentes, com a decisão registrada.

O critério final é um cliente legado compatível completar login, criação de personagem, inventário, batalha, recompensa e reconexão com persistência. Até essa evidência, /capabilities informa game_client_compatible=false e migration_complete=false.
