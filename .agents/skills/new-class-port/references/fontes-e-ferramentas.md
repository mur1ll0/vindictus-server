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
- `bin/`: DLLs patchados, *backups* `*.pre-hcls.bak`, `*.pre-classflags.bak`, `*.pre-iocpsign.bak`, `*.pre-routing.bak` e retratos
  `*.pre-<passo>.bak` (`docs/client-dll-patches.md`), `class-routing.json` (endereços dos *stubs*, lido pelo `lab_memory.py`).
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

## Conteúdo do NA (Miri; `docs/future/miri-class-port.md` 3.1–3.2)
| Ferramenta | Uso |
|---|---|
| `scripts/reverse/na_class_measure.py` | medição `db`, `models`, `tokens` da classe (passo 0) |
| `scripts/client/stage_na_class.py` | prepara arquivos e transformações em `.cache/private-assets/classes/<classe>`, manifesto `class-assets.json` |
| `scripts/client/mdl_v45_to_v44.py` | conversão de modelos v45 → v44 |
| `scripts/client/mdl_anim.py` | trilhas de animação v44: `pin_bone`, `retarget`, `with_included_model` |
| `scripts/client/kv_merge.py` | mesclagem KeyValues só com os nós da classe |
| `scripts/client/gfx_patch.py` | filmes GFx: atlas de retrato, quadro de classe, entrada de *array* no AS2 |
| `scripts/client/hfs.py` | pacotes HFS (`write_pack`, `sync_listed`, `pack_name`) |
| `scripts/client/apply_lab_content_additions.py` | único gravador da cópia: pacotes, índice, mesclagens, textos, `UIElements.xml`, `heroes.db3` |
| `scripts/content/class_imports.py` | linhas do NA no esquema 1.69 (`<classe>.db3`), textos, conjuntos, XE da doadora, bit de classe |
| `scripts/content/class_costumes.py` | renumeração de roupas, subtipos de arma `10100 + n`, rastros que cabem |
| `scripts/content/refresh_item_content.py` | exportações do servidor quando mudam os itens ou o `classes_sha256` (`start-lab.bat`) |
| `scripts/reverse/na_x64_miri_entity.py`, `na_x64_draker_weapon.py`, `na_x64_miri_dragon_energy.py`, `v169_rope_users.py` | modelos de medição de entidade, arma e mecânica no NA x64 e de usos no 1.69 |

## Patches e testes
Geração, versionamento e reconstrução dos DLLs: `docs/client-dll-patches.md` (os DLLs não vão para o Git; o código que os gera, sim).
- `scripts/client/patch_shop_list_limit.py` (`client.dll`, limite da lista do *cash shop*; primeiro da cadeia).
- `scripts/client/patch_class_table.py` (`.hcls`, 28 classes do NA, `ACTIVE_CLASSES` 12).
- `scripts/client/patch_engine_class_flags.py` (`engine.dll`, `.hflg`, `CLASS_FLAGS`).
- `scripts/client/patch_engine_iocp_sign.py` (`engine.dll`, IOCP com ponteiros acima de 2 GB; depois das *flags*).
- `scripts/client/patch_class_routing.py` (`.hrte`, `EXTRA_CLASSES`, grupos, `CREATION_GRID`).
- Testes: `test_patch_class_table.py`, `test_patch_engine_class_flags.py`, `test_patch_engine_iocp_sign.py`,
  `test_patch_class_routing.py` (inclui `test_miri_gets_her_own_entries`, `test_delia_mechanics_stay_on_delia` e
  `test_every_new_pointer_is_relocated`); conteúdo: `test_stage_na_class.py`, `test_mdl_v45_to_v44.py`, `test_mdl_anim.py`,
  `test_kv_merge.py`, `test_gfx_patch.py`, `test_apply_lab_hfs.py`, `test_class_imports.py`, `test_class_costumes.py`. As classes
  `LabCopyTest` são **puladas** sem a cópia de laboratório; teste pulado não é evidência.
