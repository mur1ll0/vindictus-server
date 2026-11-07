# Roteiro de testes no cliente — B5 cosméticos

Peças da aba avatar (cash shop, collabs e outras) equipadas por cima do equipamento (`docs/minimum-equipment-client.md`, item Avatar).

## Preparação
1. Docker reconstruído e cópia do cliente atualizada (`start-lab.bat`).
2. Uma personagem de cada corpo (por exemplo Miri ou Fiona, e Lann ou Karok) com peças de avatar no inventário (cash shop do laboratório ou troca dos collabs).

## Como relatar
ID da linha, classe, peça, o que apareceu (captura) e horário (para `docker compose logs game`).

### AV1 — Equipar peças de avatar
- [ ] Arrastar uma peça da aba avatar (por exemplo Asuna Knights of the Blood Uniform Top (Avatar) numa Miri) para o espaço de avatar → equipa sem recusa; o visual da parte troca para o da peça, com as cores dela, mesmo com uma túnica normal por baixo.
- [ ] Equipar as cinco partes (cabeça, túnica, calça, luvas, botas) → todas mudam; tirar uma → a parte volta ao visual da peça normal (ou ao corpo base).
- [ ] Botão de esconder de uma parte de avatar (cabeça, túnica, calça, luvas ou botas) → aquela parte volta ao visual da peça normal; clicar de novo → o avatar volta a aparecer; relogar → o estado (escondido ou não) se mantém.
- [ ] Peça que esconde partes no `ExtraData` (por exemplo avatares com `Foot`, `Hand`, `LowerBody`) → essas partes ficam sem peça normal visível.
- [ ] Atributos (C) → não mudam por vestir o avatar, salvo os do próprio item; durabilidade da peça normal continua.
- [ ] Relogar e voltar à seleção de personagem → o visual com avatar se mantém na cidade e na lista.
Limites conhecidos: bônus por número de peças de avatar (`AvatarSlotBonus`) não aplicado; peças sem modelo para o corpo da classe ficam invisíveis.
