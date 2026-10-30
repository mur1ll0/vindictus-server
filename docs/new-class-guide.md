# Trazer uma classe nova para o cliente 1.69 e o servidor

Guia montado com a Delia (classe 10, outubro de 2026). Cada passo cita a ferramenta e o teste; os endereços são do cliente 1.69 EN de laboratório (`client.dll` `a9591d3a…` antes do `.hrte`, `server.dll` `af43431e…`). Histórico e evidência: `docs/history/session-log.md` (passos 4–31) e `docs/client-tests/C2-delia.md` (D0–D18).

## 1. O que a classe precisa ter no conteúdo

Antes de qualquer patch, confirme no conteúdo do 1.69 (cliente `heroes.db3`, servidor `heroesContents.db3`, arquivos HFS):

- `FeatureMatrix`: a *feature* da classe ligada no perfil do laboratório (`EN-EU-X-GM`, `CurrentVer` 3.09.00);
- `ClassRestriction` com o bit da classe (`1 << índice`) em itens e habilidades; `SkillInfo`/`SkillRankInfo`/`SkillArgumentInfo` da classe; `CharacterClassInfo`;
- arquivos do cliente: `models/player/<classe>/` (base, rostos), `models/player/_anim/<corpo>/<classe>/` (`town`, `battle`, `battle_ex`), `scripts/actionstate/player_action_state_<classe>[_ex].txt`, `scripts/expression/expression_table_<classe>*`, `scripts/skill/<classe>_skill_keyword_table.txt`, `csf/create_<classe>.csf`, `csf/default_<classe>.bsh`, `csf/bodyshape_<classe>_default.fig` (+ `bodyshape_fig_list.txt`), as seções `"<classe>"` do `scripts/player_costume.txt` e `scripts/translated_spskill.txt`;
- a arma: `EquipItemInfo.EquipClass` dos itens e a entidade de arma que a classe usa no NA.

O que faltar no 1.69 vem do NA (`F:\Games\vindictus\hfsExtracted`, somente leitura) por decisão do operador.

## 2. Patches dos DLLs (cópia de laboratório)

Ordem de aplicação, sempre com o jogo fechado:

1. `scripts/client/patch_class_table.py --client-copy .cache/play-client --apply`: tabelas de classe com N entradas (`.hcls`).
2. `scripts/client/patch_engine_class_flags.py --client-copy .cache/play-client --apply`: o `engine.dll` grava o 11º *flag* "classe pode usar" do item (`bool[11]` em `item+0x198`). **Limite:** só há 11 *flags*; uma classe acima de 10 precisa estender esse vetor (ainda não feito).
3. `scripts/client/patch_class_routing.py --client-copy .cache/play-client --apply`: tudo o que é por classe, dirigido pela tabela `EXTRA_CLASSES`. Para reaplicar, copie os `*.pre-routing.bak` sobre `bin/client.dll` e `bin/server.dll` antes.

### A linha da tabela `EXTRA_CLASSES`

```python
ExtraClass(10, 'delia', 'hagie', 'female', 'csf/bodyshape_delia_default.fig', (0.88, 0.96), 3,
           weapons=('weapon_greatsword',)),
```

| Campo | Uso |
|---|---|
| `index` | `m_iClass`, contíguo a partir de 10 e abaixo do pet (28) |
| `key` | nome nos scripts e caminhos (`delia`) |
| `clone_of` | classe antiga cujos casos de `switch` são copiados com os nomes trocados, e cuja coluna as tabelas de padrões leem (`CLASS_DEFAULTS`) |
| `body` | pasta do esqueleto em `_anim` |
| `figure`, `constants` | *slots* 264 e 177/178 da entidade `C_HeroesPlayer` (`ENTITY_SLOTS`) |
| `faces` | rostos pré-carregados |
| `shape_of` | vaga do gerenciador de forma do corpo emprestada (`SHAPE_CLASS`, experimento) |
| `weapons` | entidades de arma da classe (`CLASS_WEAPONS`); o NA tem a lista em `server.dll` ao montar a tabela por classe |

### O que cada patch cobre (e o defeito que corrigiu na Delia)

| Patch | Onde | Sintoma sem ele |
|---|---|---|
| `SWITCHES`, `TABLES` | caminhos por classe (estado de ação, expressões, palavras-chave, `csf/create_`, movimentos) | modelo da Fiona, queda, animação congelada |
| `BITS`, `GATES` | `m_iClass` em rede com 5 *bits*; configuração do jogador local | queda ao entrar |
| `MATCH` | `SkillActionStateMatchTable` (11º mapa) | queda no emote e no Mercenary Outpost |
| `FLAG_CHECKS`, `TOOLTIPS` | "classe pode usar" do item, só classe 10 | "Cannot be equipped" |
| `PRECACHE`, `COSTUME`, `EVENT_FILE`, `COSTUME_GATE` | modelos, coluna de roupa (sombra), arquivo de eventos, montagem do `CoStUmE_…` | corpo invisível |
| `CLASS_NAME` | nome → classe (`translated_spskill`) | sem habilidades SP |
| `ENTITY_SLOTS` | figura e constantes do corpo | — |
| `CLASS_BITS` | `IsAvailableClass` das habilidades (3 lugares) | janela de habilidades vazia |
| `CLASS_STRING_BOUNDS` + `TABLES['client.dll']` | nome/inicial da classe para a janela de habilidades | habilidades empilhadas |
| `CLASS_DEFAULTS` | tabelas de padrão de roupa `[slot][10]` | faixa branca na seleção (pintura do corpo com lixo) |
| `CLASS_WEAPONS` | lista de entidades de arma por classe | duas espadas |
| `SHAPE_CLASS` | gerenciador de forma do corpo | (sem efeito confirmado) |

Teste: `cd scripts/client && python -s -E -m unittest test_patch_class_routing` (inclui um plano com uma segunda classe fictícia, a Miri, 11).

## 3. Servidor Rust

- Personagem de laboratório: `scripts/dev/lab-set-class.ps1 -Name <nome> -Class <n>` troca a classe e leva junto as habilidades iniciais (acrescentadas, nunca renomeadas: a carga exige as linhas do inicializador), a roupa-base e a forma do corpo (tabelas `$defaults`, `$looks`, `$shapes`, `$figures` no script; acrescente a classe nova a cada uma).
- Maestria da arma: `src/domain/equipped_skills.rs` (`mastery`), com teste em `tests/equipped_skills.rs`.
- Limites de 10 classes ainda no Rust (criação, inicializador, verificações de posse): ver `docs/future/new-classes.md` 8.13.
- Reconstruir a imagem ao fechar o grupo: `COMPOSE_PARALLEL_LIMIT=1 docker compose build server`, depois `docker compose up -d --wait`.

## 4. Ferramentas de rastreio

- `scripts/dev/lab_memory.py` (leitor elevado, `serve`): `entities`, `heapdword` (objetos pela vtable), `studios`/`studio` (cabeçalhos de modelo, ossos, sequências), `ptrstr`, `save`, `poke` (experimento).
- `scripts/dev/lab_watch.py`: pontos de parada de **escrita** (confiáveis). Os de execução derrubam o jogo (as *threads* WOW64 ignoram RF/TF).
- Padrão recorrente dos defeitos: `cmp reg, 9`/`0xa` antes de indexar uma tabela de 10 classes; varra pelo padrão e confira cada leitor.

## 5. Pendências conhecidas para a próxima classe

- criação: a barra V4 tem 10 classes; a grade de 28 do NA pede o filme do NA e código novo;
- *flags* de item acima da classe 10 (`engine.dll` `bool[11]`);
- arma própria (`weapon_<arma>` do NA) quando o 1.69 não tiver uma entidade equivalente;
- mecânicas próprias da classe (E3–E5 da Delia: guarda, habilidade segurada, ataque forte).
