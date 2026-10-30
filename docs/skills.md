# Skills do projeto

Instaladas em .agents/skills em 2026-10-09, usando o instalador oficial de skills do Codex. Disponíveis na descoberta do próximo turno; o SKILL.md pode ser lido diretamente neste turno. Não foi necessário plugin com acesso a contas externas.

- rust-best-practices: https://github.com/apollographql/skills/tree/main/skills/rust-best-practices — Apollo, MIT, versão declarada 1.1.2.
- supabase-postgres-best-practices: https://github.com/supabase/agent-skills/tree/main/skills/supabase-postgres-best-practices — Supabase, MIT, versão declarada 1.1.1.
- docker-build-strategies: https://github.com/docker/skills/tree/main/skills/docker-build-strategies — Docker, Apache-2.0.

Skill escrita no projeto (sem fonte externa):

- new-class-port: `.agents/skills/new-class-port` (SKILL.md e `references/`), escrita em 2026-10-30 a pedido do operador, a partir do porte da Delia (classe 10): `docs/future/delia-class-port.md`, `docs/future/new-classes.md` (8.1–8.15, 9.1), as entradas do `MIGRATION.md` do "Caminho C, etapa C2" ao "Passo C3-23", os roteiros `docs/client-tests/C2-delia.md` e `C3-criacao.md` e os patches `scripts/client/patch_class_table.py`, `patch_engine_class_flags.py` e `patch_class_routing.py` com seus testes. Ensina a portar a próxima classe do cliente NA (Miri, classe 11); aponta para o documento da Delia em vez de copiá-lo. Atualizar junto com o documento da classe quando o porte seguinte mudar a receita.

As referências das skills externas foram copiadas junto delas. AGENTS.md determina quando carregá-las. As skills são instruções de desenvolvimento; não substituem bibliotecas, compiladores, ferramentas de engenharia reversa ou evidência de testes.

Ferramenta instalada localmente: ILSpy CLI 9.1.0.7988 em .tools/dotnet, usando o SDK .NET x64 já existente. O PATH padrão apontava para dotnet x86 sem SDK; os scripts usam descoberta explícita do SDK x64 no Windows. Não foi necessário instalar outro SDK.
Ferramenta offline adicional: python-registry1.3.1 (Apache-2.0), instalado somente em .tools/registry-reader para leitura de cópias de hives, sem carregar o registro do sistema. Fonte: https://github.com/williballenthin/python-registry. Reinstalação: py -3.13 -m pip install --target .tools/registry-reader python-registry==1.3.1.
