# Prompt — última etapa da Delia: criação de personagem (grade do NA no cliente 1.69)

Prompt para abrir uma sessão nova (Claude Code, pasta `D:\PROJETOS\VindictusPrivate\vindictus-server`, branch `main`). Escrito em 29/10/2026, ao fechar os testes C2 da Delia (`docs/client-tests/C2-delia.md`, "Fechamento do C2").

---

## Tarefa

Última etapa da Delia (classe 10) no laboratório Vindictus 1.69: **criação de personagem pela grade de classes do NA**, totalmente funcional no cliente 1.69, sem quebrar a criação e a seleção das classes 0–9.

Leia antes de mexer: `AGENTS.md`, `MIGRATION.md` (passos da Delia, do 21 ao 50, e as notas de criação: commits `8305d80`, `5aa5cf2`, `d90bfe4`), `docs/playable-game-roadmap.md`, `docs/future/new-classes.md` (seções 8.x, principalmente as linhas sobre `NewCharacterDialogS3`, o contrato filme/C++ e o plano de 364–376), `docs/client-compatibility.md` ("Clientes de referência") e `docs/client-tests/C2-delia.md` (D6, D9, D12–D17).

### O que já se sabe (não refazer)
- O 1.69 escolhe a tela em `client.dll` `0x38523690`: `S3_CharacterCreation` ligado → `NewCharacterDialogS3` (`NewCharacterDialog_V2.gfx`, a grade); senão `CharacterCreation_UI_V2` → `NewCharacterDialogV4` (a barra de 10 classes usada hoje, que nunca manda a classe 10).
- O D6 (commit `8305d80`, revertido em `5aa5cf2`) ligou a grade do 1.69: a Delia aparecia, mas o modelo 3D ficava o Kalok, "Create this character" não fazia nada e travava, e nenhuma mensagem de criação chegava ao servidor. Causa: o mapa `s3_game_create_character.bsp` só tem atores, câmeras e *triggers* `cc_create_character_*` de 9 classes (até a Arisha), e a cópia NA é idêntica. A grade do 1.69 é uma versão antiga, nunca lançada.
- A grade do NA é o mesmo `NewCharacterDialog_V2.gfx` num filme mais novo (28 classes), que toca `csf/createcharacter_<classe>.csf` em vez de atores do mapa. Contrato:
  - C++ → filme: `SetNewIconCharacter`, `SetCharacterName`, `SetCharacterDescription`, `SetCharaterAttribute`, `SetCharaterIntroMovie`, `SetCharaterSkillMovie`, `SetWeaponButton`, `SetScreenSize`, `ShowCharacterInfo`, `ClearCharacterInfo`, `ClearCharacterSkillMovieInfo`.
  - Filme → C++: `OnStart`, `OnReadyCreation`, `OnStartCreation`, `OnPlayRandomVoice`, `OnStopRandomVoice`, e o comando `Select <classe>`.
  - O `client.dll` 1.69 só conhece `OnStart`, `SetCharacterName` e `ShowCharacterInfo`.
- Endereços do NA:
  - `client.dll` NA 32 bits: despachante `0x389e06f0` (nome → índice por `0x3820d9c0`, `cmp < 0x1c`); `SetNewIconCharacter` em `0x389e07af`; `SetCharaterIntroMovie` em `0x389e0efd`/`0x389e1218`; `OnStartCreation` em `0x38bdf541`; `CharacterCreation_UI_V3` em `0x380921d9`, `0x381e4c1b`, `0x3888c200` e `0x3888cdee`.
  - NA x64 (`.cache/bin_x64`): laços de criação em `0x23b93e`, `0x23c94b` e `0x23d972`.
- Arquivos NA disponíveis:
  - `.cache/na-extract/scaleform`: `NewCharacterDialog_V2.gfx`, `NewCharacterDialogV3/V4.gfx`, `CreateCharacterDialog.gfx`, `CreateCharacterOptionDialog.gfx`, `CreateCharacterNameDialog.gfx`, `SelectCharacterDialog[_V2].gfx`.
  - `.cache/na-extract/scaleform/video/createcharacter/*_NewCharacter_<Classe>.usm`.
  - `.cache/na-extract/csf`: `createcharacter_*.csf` (28 classes, mais `_2nd`), `createcharacter_delia.csf`, `create_delia.csf`, `costume_default_delia.csf`, `costume_default_delia02.csf`, `bodyshape_*_delia.fig`.
  - Extração completa: `F:\Games\vindictus\hfsExtracted`; instalação NA: `F:\Games\vindictus\appdata\en-US` (somente leitura).
- Servidor Rust: já aceita `CreateCharacter` com a classe 10 (`src/domain/character_class.rs`: `is_class` < 28; `character_creation_core.rs:57`; `creation_lifecycle.rs:241` exige `supports_class`; dados iniciais da Delia em `scripts/content/class_additions.json` e `docker/lab/minimum-creation-assets.json` → `class-10-initializer.json`). Falta só o cliente mandar a criação.
- Seleção: o defeito da faixa branca está resolvido (`CLASS_DEFAULTS`, D17); `SHAPE_CLASS` continua aplicado sem efeito visível ("a revisar").
- Os patches do cliente/servidor ficam em `scripts/client/patch_class_routing.py` (tabela `EXTRA_CLASSES`, aplicação sempre a partir dos `*.pre-routing.bak`, testes em `scripts/client/test_patch_class_routing.py`); o `engine.dll` em `scripts/client/patch_engine_class_flags.py` (a partir do `pre-classflags.bak`).

### O que fazer
1. Ler no NA (DLLs 32 bits e x64, filmes `.gfx` e `.csf`) como a grade funciona de ponta a ponta: preencher a lista, selecionar uma classe, tocar a apresentação (`createcharacter_<classe>.csf`, vídeos `.usm`), botões de arma, entrar na customização (`CreateCharacterOptionDialog`/`CreateCharacterDialog`), nome e envio do `CreateCharacter`.
2. Decidir com evidência o caminho no 1.69 e registrar no `MIGRATION.md`: trazer o filme NA para a cópia de laboratório (`.cache/play-client`, nunca nos originais) e implementar o contrato no `client.dll` 1.69 como código novo na seção `.hrte`, ligando `S3_CharacterCreation`/`CharacterCreation_UI_V3` pelo servidor. Se faltar asset no 1.69 (`.csf`, `.usm`, modelos), copiar do NA para a cópia de laboratório, registrando arquivo, origem e hash (sem *commitar* assets do jogo).
3. A grade precisa funcionar para todas as classes que o 1.69 já cria (0–9) e para a Delia (10): apresentação, troca de arma, customização, nome, criação, entrada na cidade. Classes sem conteúdo no 1.69 (11–27) ficam ocultas ou desligadas, sem quebrar a tela.
4. Seleção de personagem: a Delia criada pela grade aparece certa na seleção (modelo, rosto, cabelo, roupa) e entra no jogo.
5. Seguir o `AGENTS.md`: passos pequenos e verificados, testes focados por passo, *commit* local por passo com `MIGRATION.md` e entrada de teste do cliente (bloco de criação: criar `docs/client-tests/C3-criacao.md` ou usar o script do bloco certo), um *job* pesado por vez, Docker só ao fechar um grupo, jogo fechado para aplicar patches e uma aprovação de UAC para leitura de memória (`.cache/lab-serve.ps1` + `.cache/lab-ask.sh`), sem pontos de parada de execução (`--exec`).

### Sessão filha: armaduras de preview na customização da Delia
Quando a grade já abrir a customização da Delia, inicie **a partir desta sessão** uma sessão nova (`start_session`, com *worktree* própria) com este objetivo:
- Na customização da Delia há botões de troca de armadura: **4 opções com armadura e 1 opção sem armadura**. Ao clicar em cada um, o modelo deve vestir a armadura de *preview* que o jogo original mostra (e, na opção sem armadura, ficar só com a *inner*).
- Descobrir no NA quais são essas armaduras (provavelmente no `CreateCharacterOptionDialog.gfx`, nos `csf/createcharacter_delia*.csf`/`costume_default_delia*.csf` ou numa tabela do `heroes.db3`, como `CustomizeInfo`/`CostumeInfo`) e como o cliente aplica o *preview* no modelo de criação.
- Fazer funcionar no 1.69 para a Delia (e confirmar que as outras classes continuam iguais), com testes focados, entrada de teste do cliente e *commit* local na branch dela, juntando à `main` quando o operador aprovar.

### Fechamento: registro de tudo o que a Delia exigiu
Depois que a sessão filha das armaduras terminar, escrever um documento único (sugestão: `docs/future/delia-class-port.md`) com todo o conhecimento para trazer uma classe nova ao 1.69, usando a Delia como exemplo completo:
- **Arquivos de conteúdo:** quais `.csf`, `.fig`, `.gfx`, `.usm`, modelos, scripts de animação (`pc_delia_motion*.txt`) e entradas do `heroes.db3`/`scripts/*.txt` foram necessários, de onde vieram (1.69 ou NA) e como entram na cópia de laboratório.
- **Banco de dados (PostgreSQL):** tabelas e linhas novas ou alteradas (classe, *initializer*, inventário e roupa iniciais, habilidades, `FeatureMatrix`, perfis de laboratório), com os scripts que as aplicam.
- **DLLs:** cada patch do `patch_class_routing.py` e do `patch_engine_class_flags.py` (`client.dll`, `server.dll`, `engine.dll`), com o endereço, o defeito que corrige e a evidência (1.69/NA), em ordem de dependência.
- **Servidor Rust:** módulos e regras alterados para a classe 10 (limites de classe, criação, *initializer*, efeitos, barra de atalhos, uso de itens etc.).
- **Testes de cliente:** quais roteiros validaram cada parte (`C2-delia.md`, criação, armaduras de *preview*).
- **Pendências e aproximações:** o que ficou aproximado (por exemplo `As_Cond_Can_Armor_Purge` sempre ligada, `SP3_Activated` não portado, visual de purga no barco, `ArmorBrokenMessage` sem *handler*).
- **Receita resumida:** a ordem para portar a próxima classe (Miri, 11, e as seguintes).
Registrar o fechamento no `MIGRATION.md` e fazer o *commit* local.

Fim: perguntar ao operador pelo teste de criação no cliente (roteiro de criação completo) antes de declarar a Delia concluída.
