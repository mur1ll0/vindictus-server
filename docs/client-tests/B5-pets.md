# Roteiro de testes no cliente — B5 pets

Sistema de pets do servidor Rust (`docs/future/pet-system.md`, etapas P1–P6). Os pets são **da conta**: todas as personagens da conta veem os mesmos pets. O teste usa uma personagem já existente (Fiona ou outra classe de 0 a 9).

## Preparação
1. Runtime com os pets: depois do commit, `COMPOSE_PARALLEL_LIMIT=1 docker compose build` e `docker compose up -d --wait`; `docker compose ps` mostra `game` e `server` saudáveis. A migração `0050_pets` cria as tabelas na subida.
2. Conteúdo: `.cache/private-content/item-economy.json` tem a seção `pets` (gerada por `scripts/content/export_item_economy.py`; o arquivo anterior ficou em `item-economy.json.pre-pets.bak`).
3. Cliente: abrir pelo `.cache/play-client/start-lab.bat`, com a conta `lab169_a`.
4. Itens de teste, dados com a personagem **fora do jogo** (PowerShell 7, na pasta do projeto; o `cid` aparece no painel, na ficha da personagem):
   - `./scripts/dev/lab-give-item.ps1 -ItemClass pet_create_gremlin_young -Count 2 -Cid <cid>`
   - `./scripts/dev/lab-give-item.ps1 -ItemClass pet_feed_normal -Count 3 -Cid <cid>`
   - `./scripts/dev/lab-give-item.ps1 -ItemClass pet_feed_make -Count 5 -Cid <cid>`
   - `./scripts/dev/lab-give-item.ps1 -ItemClass pet_name_change -Count 1 -Cid <cid>`
   - `./scripts/dev/lab-give-item.ps1 -ItemClass pet_skill_atk_buff_min30 -Count 1 -Cid <cid>`
   (Alternativa: Personagens → ficha → Dar item, no painel.)
5. Relatórios de queda, se houver: `Documents\Vindictus EU GM\Report Error`.

## Ordem de gameplay (uma sessão)
1. **P1:** criar o pet com o item e conferir a janela de pets.
2. **P2:** relogar e conferir que o pet continua lá.
3. **P3:** invocar o pet na cidade. **Checkpoint:** o pet aparece? Se o cliente fechar, pare aqui e relate.
4. **P4:** ração de item (cheio e depois de uma missão) e ração compartilhada.
5. **P5:** renomear com o item.
6. **P6:** missão curta com o pet invocado. **Checkpoint:** o pet entra na fase com o índice 28 (`docs/future/pet-system.md`, seção 2.1). Se o cliente fechar ao carregar a fase, pare aqui e relate.
7. **P7:** skill de cash, *slot* e remoção.
8. **P8:** painel (aba Pets).

## Como relatar
Para cada linha marcada: ID da entrada e número da linha (ex.: `P3-2`), o que apareceu na tela (texto, captura se possível), se o cliente fechou ou travou, o horário aproximado e o nome do relatório de queda, se houver. Num checkpoint que falhar, pare e relate; as entradas seguintes dependem dele.

### P1 — Criar o pet
Pré-condição: preparação feita; personagem na cidade.
- [ ] Usar `pet_create_gremlin_young` (botão direito no inventário) → abre a janela de nome do pet.
- [ ] Digitar `Rex` e confirmar a checagem → diálogo de nome inválido (`GameUI_Heroes_Pet_Fail_PetCreate_AgainstNamingRule`): a regra EN-EU pede 4+ letras.
- [ ] Digitar `Bolt` → checagem aceita; criar → um `pet_create_gremlin_young` some do inventário e a janela de pets (tecla do menu de pets) mostra **Bolt**, nível 1, fome cheia (200), tempo ativo de 24 h, status disponível.
- [ ] Tentar criar outro com o nome `bolt` usando o segundo item → diálogo de nome repetido (`…PetCreate_HasSameName`); o item continua no inventário.

### P2 — Persistência
Pré-condição: P1.
- [ ] Voltar à seleção e entrar de novo → a janela de pets mostra Bolt com os mesmos valores.
- [ ] (Se a conta tiver outra personagem) entrar com ela → Bolt aparece também (pets da conta).

### P3 — Invocar na cidade (checkpoint)
Pré-condição: P2.
- [ ] Na janela de pets, invocar Bolt → status invocado; o pet aparece seguindo a personagem na cidade.
- [ ] Recolher (invocar de novo ou o botão de recolher) → o pet some; status disponível.
- [ ] Invocar de novo e relogar → o pet volta invocado.

### P4 — Ração
Pré-condição: Bolt invocado.
- [ ] Usar `pet_feed_normal` com a fome cheia → diálogo `GameUI_Heroes_Pet_Fail_Feed_AlreadyMaxHunger`; o item continua.
- [ ] Depois da P6 (fome abaixo do máximo): usar `pet_feed_normal` → a fome sobe 100 (até o máximo) e um item some.
- [ ] Na janela de pets, depositar a pilha de `pet_feed_make` como ração compartilhada → a pilha some do inventário e a lista de ração mostra 5.
- [ ] Retirar a ração compartilhada → volta como item vinculado à personagem.

### P5 — Renomear
Pré-condição: P1.
- [ ] Usar `pet_name_change` em Bolt e escolher `Bolty` → o nome muda na janela e o item some; relogar mantém `Bolty`.

### P6 — Missão (checkpoint)
Pré-condição: pet invocado; (opcional) ração compartilhada depositada.
- [ ] Entrar numa missão curta → **o pet aparece na fase** e acompanha a personagem (ou anote o que aconteceu: pet ausente, modelo errado, queda).
- [ ] Matar alguns monstros e terminar a missão → no fim, a janela de pets mostra EXP maior (20 % da EXP de monstros, até o máximo do nível), fome menor (1 a cada 36 s de uso) e tempo ativo menor.
- [ ] Com ração compartilhada depositada: ao fim da missão aparece a mensagem de alimentação automática e a lista de ração diminui.

### P7 — Skill de cash, slot e remoção
Pré-condição: pet invocado.
- [ ] Usar `pet_skill_atk_buff_min30` → a skill entra no pet (slot 101) com validade de 30 minutos; o item some.
- [ ] Usar o mesmo item de novo (se houver outro) → diálogo `…GiveSkill_AlreadyHasCashSkill`.
- [ ] Depois dos 30 minutos, remover a skill na janela → mensagem de sucesso; antes do prazo → mensagem de falha.
- [ ] Mover o pet para outro *slot* da janela → a posição muda e continua depois de relogar.
- [ ] Apagar o pet (criar um segundo antes, se quiser manter o Bolt) → some da lista; se era o invocado, some da cidade.

### P8 — Painel
Pré-condição: conta fora do jogo.
- [ ] Painel → Personagens → ficha → aba **Pets** → lista os pets da conta com tipo, nível, fome e tempo ativo.
- [ ] "Dar pet" com o tipo 518 e o nome `Spot` → aparece na lista; entrar no jogo → aparece na janela de pets.
- [ ] Com a personagem no jogo, o painel recusa dar ou remover pets (online).
- [ ] Remover `Spot` pelo painel → some da lista e do jogo no próximo login.
