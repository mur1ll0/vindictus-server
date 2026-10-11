# Histórico Git e política de continuidade

O repositório local está em vindictus-server, branch main; arquivos originais irmãos não pertencem ao Git. Autoria usa a identidade já configurada pelo usuário. Nenhum remoto foi criado ou push realizado.

O histórico inicial agrupa o estado existente, sem inventar commits datados ou alegar que cada snapshot intermediário era uma versão jogável:

1. Configuração do projeto, ferramentas de desenvolvimento, Docker, CI, README, instruções e skills locais.
2. Implementação Rust e migrações PostgreSQL já verificadas.
3. Ferramentas de estudo/importação/preparação de cliente e testes/fixtures sintéticos.
4. Contratos, memória de migração e evidências sanitizadas do progresso existente.

A verificação completa do estado combinado executa scripts/dev/verify.ps1 -DatabaseIntegration. Os commits iniciais são blocos do trabalho preexistente; a comprovação de build/testes refere-se ao conjunto final, não a cada prefixo do histórico.

A partir daqui cada etapa concluída recebe commit com código, testes, evidências e MIGRATION.md juntos, após gate completo. Falha ou dependência pendente não recebe status concluído. Não publicar credenciais, exports privados, fontes descompiladas, clientes, backups ou catálogos SQL brutos. Revisar git diff --cached --check e a lista de arquivos antes de cada commit.

As evidências guardam timestamps e hashes da execução local, não asseguram equivalência a um clone com bancos vazios. Oracles novos exigem a referência privada e podem ser regenerados no laboratório; testes comuns usam fixtures sintéticos versionados.
