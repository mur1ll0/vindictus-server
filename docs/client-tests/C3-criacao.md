# Roteiro de testes no cliente — C3 criação pela grade de classes (Delia)

Última etapa da Delia (`docs/future/delia-character-creation-prompt.md`). A criação passa da barra V4 (10 classes) para a grade de classes (`NewCharacterDialogS3`) no fluxo `CharacterCreation_UI_V3` do NA: a grade abre na cena `lobby_beautyshop_change` (um ator por classe, a classe escolhida vai para o centro), e a customização roda em `lobby_beautyshop_body`. Detalhes e evidência: `MIGRATION.md`, passo C3-1.

## Preparação
1. Servidor: imagem `vindictus-server:local` recompilada com o commit deste passo (o servidor manda `S3_CharacterCreation = 3.09.00`); `docker compose ps` mostra `game` e `server` saudáveis.
2. DLLs: `client.dll` e `server.dll` da cópia de laboratório repatchados a partir dos `*.pre-routing.bak` com `scripts/client/patch_class_routing.py` deste commit (bloco `creation_grid` em `.cache/play-client/bin/class-routing.json`). Feito por mim com o jogo fechado; os hashes ficam no `MIGRATION.md`.
3. Cliente: `.cache/play-client/start-lab.bat`, conta `lab169_a`. Se a conta estiver sem vaga, exclua uma personagem de teste antes (não a `DeliaLab`).
4. Relatórios de queda: `Documents\Vindictus EU GM\Report Error` (nome do arquivo mais novo e horário).

## Ordem de gameplay (uma sessão)
1. **G1:** abrir a grade e olhar a cena.
2. **G2:** passar por todas as classes, Delia por último.
3. **G3:** criar uma classe antiga (Fiona) até a cidade.
4. **G4:** criar uma Delia até a cidade e voltar à seleção.
5. **G5:** abrir a grade e voltar sem criar.

## Como relatar
Para cada linha: ID e número (ex.: `G2-3`), o que apareceu (captura se possível), se travou ou fechou, horário aproximado e relatório de queda. Se uma linha falhar, anote o horário, tente a **G5** (voltar) e encerre a sessão; as entradas seguintes dependem da anterior.

### G1 — A grade abre na cena do NA
Pré-condição: preparação feita, na seleção de personagem.
- [ ] Clicar em criar personagem → tela escurece e abre a **grade de classes** (11 ícones, Delia incluída), não a barra V4. Anotar o fundo (loja de beleza?) e de onde a câmera olha.
- [ ] Sem clicar em nada → a primeira classe (Lethita) aparece no centro, de frente para a câmera, e o vídeo de apresentação toca no painel. Anotar se aparecem outros atores (em volta ou ao fundo).

### G2 — Troca de classe
Pré-condição: G1.
- [ ] Clicar em cada classe antiga (Fiona, Evy, Kalok, Kay, Vella, Hurk, Lynn, Arisha, Hagie) → a cada clique o ator da classe aparece no centro e o anterior some; vídeo e descrição mudam. Anotar classes em que o ator não aparece, fica no lugar errado ou com pose estranha (Kalok e as classes altas podem ficar um pouco deslocadas: aproximação conhecida).
- [ ] Clicar na **Delia** → a Delia aparece no centro com a roupa de apresentação; vídeo da Delia no painel.
- [ ] Anotar a animação dos atores (parados, idle, pose de apresentação). O NA toca uma pose "selecionada"; aqui o ator usa a animação própria (aproximação conhecida).

### G3 — Criar uma classe antiga (Fiona)
Pré-condição: G2, com a Fiona selecionada.
- [ ] "Create this character" → tela escurece e abre a **customização** (`CreateCharacterDialog`) numa sala da loja de beleza, com a Fiona no centro. Anotar se a câmera enquadra a personagem e se as abas (rosto, cabelo, cores, corpo) mudam o modelo.
- [ ] Concluir e dar o nome `GradeFiona` → criada; o jogo segue para a cidade ou para a lista como antes. Anotar o que acontece.
- [ ] Na seleção, `GradeFiona` aparece com a aparência escolhida.

### G4 — Criar uma Delia
Pré-condição: G3 concluída (ou, se a G3 falhou na criação mas a customização abriu, ainda assim tente).
- [ ] Na grade, escolher a **Delia** → "Create this character" → customização com a **Delia** (corpo, rosto, cabelo da Delia, não da Fiona). Mudar rosto, cabelo e cores → o modelo muda.
- [ ] Os botões de armadura da customização: anotar quantos há e o que acontece ao clicar (as armaduras de *preview* são a etapa seguinte, numa sessão separada).
- [ ] Concluir, nome `GradeDelia` → criada; entra na cidade como Delia (espada nas costas, combate básico como a `DeliaLab`).
- [ ] Sair para a seleção → `GradeDelia` aparece certa (modelo, rosto, cabelo, roupa) e entra de novo.

### G5 — Voltar sem criar
Pré-condição: na seleção.
- [ ] Abrir a grade e clicar em voltar → volta à seleção sem travar; a lista continua certa.
