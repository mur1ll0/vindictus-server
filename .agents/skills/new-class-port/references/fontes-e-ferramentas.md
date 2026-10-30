# Fontes e ferramentas do porte de classe

Convenções de endereço (iguais às do `docs/future/delia-class-port.md`): 1.69 EN de laboratório,
`client.dll` base `0x38000000`, `server.dll` local base `0x40000000`, `engine.dll` base `0x42000000`.
Os DLLs de laboratório não têm ASLR.

## Clientes e dados de referência (somente leitura)
Tabela completa em `docs/client-compatibility.md`, "Clientes de referência". Nada é executado nem
alterado neles por *scripts* do projeto.

| Fonte | Para quê |
|---|---|
| `../Vindictus1.69EN/Vindictus EU/en-EU` | cliente 1.69 original, origem da cópia de laboratório |
| `F:\Games\vindictus\appdata\en-US` (`bin/` 32 bits, `bin_x64/`) | cliente NA oficial: código e dados da classe nova |
| `.cache/bin_x64` | cópia idêntica do `bin_x64` do NA (tem tabela de exceções: delimita funções sem análise de fluxo) |
| `F:\Games\vindictus\hfsExtracted` | extração completa do NA (HFS v4, *scripts* decifrados, `sqlite/heroes.db3` em claro); MIGRATION "Extração completa do cliente NA" |
| `.cache/na-db/heroes-plain.db3`, `.cache/na-hfs` | `heroes.db3` do NA em claro (`8eb596f8…`, 495 tabelas) |
| `.cache/na-extract` | extração parcial anterior (Delia, `createcharacter_*`, filmes de `scaleform`) |
| `C:\Windictus 2023` | cliente de servidor privado mais novo; relatórios de queda em `Documents\Vindictus EU GM\Report Error` |
| `ServidorVindictus/Server/zh-tw-x-gm/Bin/heroesContents.db3` | conteúdo do servidor 1.69 (banco de referência, nunca sobrescrever) |

Ferramenta de extração revisada: `NPCodex/hfs-extract` (fonte em `.cache/hfs-extract-v4-src`,
`new-classes.md` 7.6). A segunda camada dos *scripts* e do `heroes.db3` já foi resolvida (7.8–7.9).

## Cópia de laboratório `.cache/play-client`
- `bin/`: DLLs patchados, *backups* `*.pre-hcls.bak`, `*.pre-classflags.bak`, `*.pre-routing.bak`,
  `class-routing.json` (endereços dos *stubs*, lido pelo `lab_memory.py`).
- `hfs_extracted/`: conteúdo do 1.69 extraído (o que já existe para a classe).
- `scaleform/`: filmes e imagens soltos, que têm prioridade sobre os pacotes (armadilha do C3-12).
- `start-lab.bat`: abre o cliente. Conta de teste `lab169_a`.
- A cópia nunca é o original: os patches recusam o original (`reject_original`).

## Ferramentas de laboratório
| Ferramenta | Uso | Cuidados |
|---|---|---|
| `scripts/dev/lab_memory.py` | leitura da memória do jogo: `entities`, `costume`, `studios`, `heapdword`, `finddword`, `rtti`, `save`/`diff`, `symbols`, `poke` (só laboratório) | o jogo roda elevado: use `serve` num terminal de administrador aprovado pelo operador e mande pedidos por `.cache/lab-memory/request.txt` |
| `scripts/dev/lab_watch.py` | ponto de parada de **escrita** (DR0–DR3), `--follow` para seguir a entidade | pontos de **execução** (`--exec`) não são confiáveis e derrubam o jogo; o antitrapaça pode abortar o cliente depois de desanexar: conte com reiniciar |
| `scripts/dev/lab-set-class.ps1` | troca a classe de uma personagem fora do jogo (habilidades acrescentadas, roupa-base, forma do corpo, figura padrão) | tabelas `$defaults`, `$looks`, `$shapes`, `$figures` e `ValidateRange` precisam da classe nova |
| `scripts/dev/lab-give-item.ps1` | dar itens da classe | personagem fora do jogo |
| painel admin, progressão "até o nível N" | nível e AP para testar habilidades de nível alto | aceita 0–27 |
| `install_lab_launcher.py` | console do motor em `lab_console.log` (`cfg/config_pendingkey_concmd.cfg`) | o `vindictus.exe` não repassa argumentos |

## Ferramentas de engenharia reversa (leitura pontual)
- `scripts/client/.deps` (capstone) para desmontar trechos; nunca análise do binário inteiro dos DLLs do
  cliente (limite da máquina, `AGENTS.md`).
- `scripts/reverse/v169_class_uses.py`: inventário das leituras de `m_iClass` (181 por DLL) e do código
  das *strings* da Hagie; lê o `*.pre-routing.bak` depois do patch.
- `scripts/reverse/na_x64_delia_entity.py`: fábrica → construtor → vtable da classe no NA x64 e *slots*
  trocados; modelo para `na_x64_<classe>_entity.py`.
- `scripts/reverse/na_x64_to_169_vtables.py`: mapa de *slots* NA x64 → 1.69 (pareamento pela Hagie,
  revisão manual em `REVIEW`); JSON local em `docs/evidence/client/`.
- `tools/na-delia-reference`: crate isolado com a reconstrução da Delia (fora do *build*); modelo para
  uma referência da classe nova.
- Minidumps: `Documents\Vindictus EU GM\Report Error`; copie para `.cache/<etapa>/` antes de analisar.

## Patches e testes
- `scripts/client/patch_class_table.py` (`.hcls`, 28 classes do NA, `ACTIVE_CLASSES`).
- `scripts/client/patch_engine_class_flags.py` (`engine.dll`, `DELIA_FLAG`).
- `scripts/client/patch_class_routing.py` (`.hrte`, `EXTRA_CLASSES`, grupos, `CREATION_GRID`).
- Testes: `test_patch_class_table.py`, `test_patch_engine_class_flags.py`, `test_patch_class_routing.py`
  (inclui `test_a_second_extra_class_gets_its_own_entries` com uma Miri fictícia e
  `test_every_new_pointer_is_relocated`). As classes `LabCopyTest` são **puladas** sem a cópia de
  laboratório; teste pulado não é evidência.
