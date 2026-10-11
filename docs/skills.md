# Skills do projeto

Instaladas em .agents/skills em 2026-10-09, usando o instalador oficial de skills do Codex. Disponíveis na descoberta do próximo turno; o SKILL.md pode ser lido diretamente neste turno. Não foi necessário plugin com acesso a contas externas.

- rust-best-practices: https://github.com/apollographql/skills/tree/main/skills/rust-best-practices — Apollo, MIT, versão declarada 1.1.2.
- supabase-postgres-best-practices: https://github.com/supabase/agent-skills/tree/main/skills/supabase-postgres-best-practices — Supabase, MIT, versão declarada 1.1.1.
- docker-build-strategies: https://github.com/docker/skills/tree/main/skills/docker-build-strategies — Docker, Apache-2.0.

As referências dessas skills foram copiadas junto delas. AGENTS.md determina quando carregá-las. As skills são instruções de desenvolvimento; não substituem bibliotecas, compiladores, ferramentas de engenharia reversa ou evidência de testes.

Ferramenta instalada localmente: ILSpy CLI 9.1.0.7988 em .tools/dotnet, usando o SDK .NET x64 já existente. O PATH padrão apontava para dotnet x86 sem SDK; os scripts usam descoberta explícita do SDK x64 no Windows. Não foi necessário instalar outro SDK.
Ferramenta offline adicional: python-registry1.3.1 (Apache-2.0), instalado somente em .tools/registry-reader para leitura de cópias de hives, sem carregar o registro do sistema. Fonte: https://github.com/williballenthin/python-registry. Reinstalação: py -3.13 -m pip install --target .tools/registry-reader python-registry==1.3.1.
