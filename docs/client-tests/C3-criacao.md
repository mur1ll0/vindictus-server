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
6. **G6:** na customização, as roupas de amostra (*Sample outfits*) da Arisha e da Delia (pode vir antes da G4, sem criar).

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
- [ ] Os botões de armadura da customização: ver **G6**.
- [ ] Concluir, nome `GradeDelia` → criada; entra na cidade como Delia (espada nas costas, combate básico como a `DeliaLab`).
- [ ] Sair para a seleção → `GradeDelia` aparece certa (modelo, rosto, cabelo, roupa) e entra de novo.

### G5 — Voltar sem criar
Pré-condição: na seleção.
- [ ] Abrir a grade e clicar em voltar → volta à seleção sem travar; a lista continua certa.

### G6 — Roupas de amostra (*Sample outfits*)
Pré-condição: `client.dll` `6a842ff5…` (passo C3-9); customização aberta pela grade.
- [ ] **Arisha** (controle): clicar nos 5 botões do seletor, um por vez → o modelo troca a cada clique como antes (sem armadura e 4 conjuntos).
- [ ] **Delia**: ao abrir, o modelo está com a roupa inicial (só o *inner*, como na primeira opção).
- [ ] Delia, botões 2 a 5 → um conjunto completo (cabeça, tronco, pernas, mãos, pés) a cada clique, nesta ordem: Crystal Rose, Training Cube, Infinity Crown, Lugh Lámhfhada (os do NA). Anotar o número do botão que não mudar nada, que mostrar peça faltando ou que mostrar modelo masculino.
- [ ] Delia, botão 1 → volta à roupa inicial (só o *inner*).
- [ ] As cores, o rosto e o cabelo escolhidos antes continuam depois da troca de roupa (anotar se o cabelo ou a maquiagem mudam junto: os conjuntos do NA trazem cabelo e maquiagem próprios, como os das outras classes).
- [ ] Concluir a criação depois de escolher um conjunto → anotar com que roupa a personagem nasce (comparar com a Arisha).

## Resultados (operador, 29–30/10/2026)
Detalhes por rodada no `MIGRATION.md` (passos C3-1 a C3-19).
- G1: a grade abre na cena `lobby_beautyshop_change` com as 11 classes; o vídeo de apresentação toca só depois do carregamento (C3-11).
- G2: cada classe vai ao centro; pose de apresentação parada (sem a sequência animada do NA, aproximação); Hagie e Delia sem pose T (C3-3).
- G4 (parcial): customização com o modelo da Delia (C3-6), botão de voltar para a grade (C3-12/C3-14), campo de nome e "Check" (C3-14), roupas de amostra (G6, C3-9), WASD travado e troca imediata de pose/ação (C3-18). Texto do nome em cinza (mantido, C3-19).
- G4 (até a cidade, 30/10/2026, "Funciona completamente agora"): "Create" → estalagem → fala da Tieve → "Go to town" → cidade (C3-20 a C3-23), com WASD travado e poses sem fila na customização.
- Pendentes: volta à seleção com a personagem criada pela grade (fim da G4), G3 (classe antiga até a cidade) e G5.
