# Viabilidade das extensões (B4.7)

Relatório de 27/10/2026. Para cada tipo de conteúdo novo (item ou equipamento, missão, fase, monstro e, como foco, classe nova), o documento separa três partes:
- o que vive no conteúdo SQLite;
- o que vive nos assets do cliente (`.hfs`, scripts, textos) e no binário nativo;
- o que exige código de servidor.

Cada conclusão traz a evidência e a estimativa de custo. **Hipótese** marca o que ainda não foi medido.

Os planos detalhados de três editores já existem e não são repetidos aqui: [item-editor.md](future/item-editor.md), [stage-editor.md](future/stage-editor.md) e [game-launcher.md](future/game-launcher.md).

## 1. Método e fontes

- **Conteúdo:** `heroes.db3` do laboratório (`.cache/play-client/sql/heroes.db3`, aberto somente leitura, 151 tabelas). As contagens vêm de `.cache/verification/feas-sqlite.log`.
- **Servidor original:** fonte recuperada em `legacy/decompiled/` (referência local, ignorada pelo Git).
- **Cliente:** a leitura pontual do `client.dll` 1.69 EN já registrada em `item-editor.md`, seção 5, mais as pastas de `.cache/play-client/hfs_extracted`. Nenhuma análise nova do binário inteiro foi feita, por causa do limite da máquina.
- **Prova de ponta a ponta já obtida:**
  - item novo por camada de adições e pacote HFS novo (commit `1439ed7`);
  - modelo editado no Blender funcionando no cliente (commit `733d691`);
  - pacote de pasta nova carregado pelo cliente (L0-g, `game-launcher.md`, seção 2.4).

Escala de custo usada nas tabelas: **baixo** é dias, só dados; **médio** é uma etapa de bloco com teste no cliente; **alto** é várias etapas com risco técnico; **proibitivo** exige alterar o binário nativo do cliente sem fonte.

## 2. Resumo

| Tipo | Conteúdo | Assets do cliente | Servidor | Binário do cliente | Veredito |
|---|---|---|---|---|---|
| Item / equipamento (modelo existente) | baixo | baixo (texto, ícone) | nenhum | nenhum | **viável agora** |
| Item / equipamento (modelo novo) | baixo | médio (`.mdl` v44 por classe) | nenhum | nenhum | **viável**, provado em uma peça |
| Missão em fase existente | baixo | baixo (textos) | baixo | nenhum | **viável** |
| Fase com mapa existente | médio | baixo | médio | nenhum | **viável** |
| Mapa novo (BSP) | médio | alto | médio | nenhum (hipótese) | possível, caro |
| Monstro: ajuste ou variação | baixo | baixo | nenhum | nenhum | **viável agora** |
| Monstro: tipo novo | médio | alto (modelo, animação, sons) | baixo | **hipótese:** IA no cliente | caro, não avaliado |
| Classe nova (11ª) | médio | alto | médio | **proibitivo** | **inviável** no cliente 1.69 |
| Variante de classe existente | médio | médio–alto | baixo | nenhum | **viável**, no lugar da classe nova |

## 3. Item e equipamento

- **Conteúdo:** `ItemClassInfo` (19.705 linhas) com `ClassRestriction` em máscara de bits, `EquipItemInfo`, `ItemStatInfo`, `EnhanceStatInfo` e `EnchantStatInfo`. A mesma linha precisa existir no `heroes.db3` do cliente.
- **Cliente:**
  - `scripts/player_costume.txt` liga o item ao modelo;
  - o ícone fica em `scaleform/item_icon`;
  - os textos ficam em `heroes_text_english_eu.txt`.
- **Servidor:** já lê tudo pelos exportadores (`item-editor.md`, seção 1.3). Item novo não exige código novo.
- **Estado:** caminho provado no cliente, do item clonado ao modelo novo (commits `1439ed7` e `733d691`). Faltam o editor no painel e a distribuição pelo launcher, nas etapas I1–I6 de `item-editor.md`.

## 4. Missão

- **Conteúdo:** `QuestInfo` (269 linhas: nível, drops, limite de tempo, `STAGE`, `QuestSet`, `Feature`, membros), mais `QuestGoalInfo`, `QuestRewardInfo`, `QuestConstraintInfo` e `QuestDifficultyInfo`. O cliente tem as mesmas tabelas.
  - **Hipótese já registrada em `stage-editor.md`, seção 1.2:** o quadro de quests e os níveis exibidos leem o banco do cliente, então uma quest nova é publicada nos dois lados.
- **Cliente:** só textos (`heroes_quest_stage_name_<quest>` e os textos de NPC).
- **Servidor:** o B2 migrou entrada, recompensa e drop de missão. Uma quest nova numa fase existente usa o mesmo caminho. O custo é baixo: conferir que o `QuestSet` e o `Feature` novos passam pelos mesmos filtros.
- **Ponto de extensão:** `TuningPoint`s de `QuestInfo`/`QuestGoalInfo`/`QuestRewardInfo` (`b4-painel-plano.md`, seção 4).

## 5. Fase e mapa

- **Fase com BSP existente:** setores, entidades e o barco estão em `stage-editor.md`, seções 1 e 2. Mudar o que um setor contém é conteúdo mais servidor, sem asset novo.
- **Mapa novo:**
  - exige um BSP compilado para a engine Source do jogo;
  - exige o grafo `maps/graphs/<bsp>.ain`;
  - exige a partição em `map_partition`, quando o mapa for de cidade;
  - entra por pacote `.hfs` novo, o que já foi provado no L0-g.
  - Custo alto. É o tipo com mais trabalho de arte.
  - **Hipótese:** o binário não limita o número de mapas.
- No canal, o servidor monta os mapas recebidos e mostra o resultado de cada `Map.Build` em Mapas e canais (B4.6).

## 6. Monstro

- **Conteúdo:** `MonsterInfo` (1.381 linhas: `Class`, `Variation`, `Difficulty`, `Level`, `Exp`, `HP1`–`HP5`, `ATK`, `DEF`, `Critical`, bônus de herói), mais `MonsterDropItemInfo` e `PropItemWeightInfo`.
  - Ajustar HP, EXP ou dificuldade é edição de linha. O contrato está em "Bosses" e "Drops", na seção 4 do plano.
- **Cliente:**
  - `scripts/npc_variations.txt` (86.417 linhas) define modelo, tamanho, queda e invocações por `npc_<classe>` e variação;
  - os modelos ficam em `models/monster/<nome>`;
  - os sons ficam em `game_sounds_heroes_monster.txt`.
  - Uma variação nova de um monstro existente (outro tamanho, outra cor, outros números) é texto mais conteúdo. Custo baixo.
- **Tipo novo:** a missão é hospedada pelo cliente; o servidor só faz o relay (`battle-transport.md`).
  - **Hipótese:** comportamento, física e animação do monstro rodam no cliente host, no código nativo e nos scripts.
  - Um monstro com comportamento novo depende de ler esse código, o que ainda não foi feito. Fica fora do B4.

## 7. Classe nova (foco)

### 7.1 Servidor original

- **O enum.** `BaseCharacter` é um enum `byte` com Lethita 0, Fiona 1, Evy 2, Kalok 3, Kay 4, Vella 5, Hurk 6, Lynn 7, Arisha 8 e Hagie 9, mais `CHARACTER_COUNT = 10` e `ALL_CHARACTER = 255` (`ServiceCore.CharacterServiceOperations/BaseCharacter.cs`). 49 arquivos de `PlayerService`, `ServiceCore`, `PvpService` e `HeroesChannelServer` o referenciam.
- **Decisões por classe no código, e não no conteúdo:**
  - `VocationSkillInfo.Character`: o nome da classe é traduzido por um `switch` fixo (`CharacterContentsExtension.GetCharacterType`).
  - `RandomItemManager`: dois `switch` para o `characterCode` da receita, um em cada uma das linhas 198 e 839. Uma classe fora da lista registra "Unknown Character" e não gera item.
  - `StoryService.DataStructure/Switch.cs` (366 linhas): um campo por classe nas condições de história.
  - `SkillEffect`: fórmula de crítico própria para Evy e Arisha.
  - `PvpService`: o pareamento por stat trata Evy de forma própria.
- **Conclusão:** uma 11ª classe no servidor é trabalho médio, porque todo esse código é nosso no Rust.

### 7.2 Servidor Rust (B4.N2)

- A criação de personagem já é guiada por dados: o `class-initial.json` e os initializers das 10 classes.
- Restam limites explícitos de 0 a 9:
  - as guardas `class BETWEEN 0 AND 9` em `character_skills.rs`, `fiona_character_state.rs`, `initial_inventory.rs` e `minimum_character_list.rs`;
  - o `< 10` em `npc_talk.rs`.
- Ampliar esses limites é barato. Eles ficam onde estão de propósito: um valor fora do cliente seria aceito pelo servidor e quebraria o cliente.

### 7.3 Conteúdo

- `ClassRestriction` usa 11 bits, de Lann 1 a Delia 1024 (`item-editor.md`, seção 1.1). O valor 2047, que cobre as 11 classes, aparece em 9.088 itens, e 12.445 itens têm o bit 1024. O conteúdo do 1.69 já traz a Delia.
- A máscara também é usada em `SkillInfo` (384), `TitleInfo` (1.233), `StoryLineInfo` (762), `Synthesis*`, `Cashshop_PreviewInfo` e `ItemClassOverrideInfo`.
- Uma classe nova precisaria de:
  - linhas em `DefaultItemInfo` e `CharacterClassInfo` (stats por nível);
  - skills próprias em `SkillInfo`, `VocationSkillInfo` e afins;
  - títulos e a história.
- O bit 1024 está ocupado pela Delia. Uma 12ª classe pediria o bit 2048, que nenhuma linha usa. Custo médio, só de dados.

### 7.4 Cliente: o bloqueio

- **Tabela fixa de 10 classes no `client.dll`.** A tabela de nomes fica em `0x38cafd50`, com o limite `cmp eax, 0xa` em `0x3879ec68` (`item-editor.md`, seções 1.1 e 5). O parser do `player_costume.txt` só reconhece blocos de classe para esses 10 nomes. Delia não existe neste cliente.
- **Comportamento de combate.** Skills, animações e o controle do personagem são decididos por classe no código nativo do cliente (`b4-painel-plano.md`, B4.7). A missão é hospedada pelo cliente (seção 6).
- **Assets.** `models/player/` do 1.69 tem pastas para as 10 classes, para `delia`, para `darkknight` e `paladin` (vocações, `VocationEnum`) e as `_anim`. **Hipótese:** a pasta `delia` é asset antecipado sem código no binário; o conteúdo dela não foi aberto.
- **Conclusão:** acrescentar uma classe ao cliente 1.69 exige patch nativo no `client.dll` sem fonte:
  - estender a tabela e o limite;
  - acrescentar todo o caminho de combate, interface de criação e animação da classe.
  - Custo **proibitivo**, com alto risco de travar o cliente.
  - Trazer a Delia do cliente NA oficial (`F:\Games\vindictus`) também não resolve: o binário que a conhece é o do cliente NA, de outra versão de protocolo. Usá-lo seria trocar o cliente do projeto, não estender o 1.69.

### 7.5 Caminho viável: variante de classe existente

- Uma "classe nova" para o jogador pode ser um personagem de uma das 10 classes com aparência, equipamento inicial, skills e títulos próprios. Tudo isso vive no conteúdo e nos assets já provados (item novo, modelo novo, pacote HFS).
- O servidor precisa de um marcador da variante:
  - por exemplo, um `Feature` no `class-initial.json` e no initializer;
  - mais a escolha na criação. **Hipótese:** pelo preset ou pela aparência, porque a tela de criação do cliente só oferece as 10.
- **Limite:** os golpes base e a arma continuam os da classe de origem. Skill nova de verdade tem o mesmo bloqueio nativo da seção 7.4, a menos que reaproveite efeitos existentes do `item_data.txt` e de `StatusEffectGroupInfo`.

## 8. Recomendações

1. **Itens:** seguir o editor (I1 → I2 → I5), que já tem o caminho provado no cliente, e o launcher (L1+) para distribuir.
2. **Missões e fases com mapas existentes:** seguir o editor de fases depois do B5, usando os contratos de `QuestInfo`/`MonsterInfo` da seção 4 do plano.
3. **Monstros:** liberar no painel o ajuste de `MonsterInfo` e dos drops (baixo custo). Adiar tipo novo até ler o lado host do cliente.
4. **Classe nova:** não planejar uma 11ª classe sobre o cliente 1.69. Se o operador quiser identidade nova, planejar a variante da seção 7.5 como recurso futuro. A decisão de trocar a base de cliente para ter a Delia ou outras classes é do operador e sai do escopo do B4.

## 9. Lacunas registradas

- Como o cliente trata um `ItemClass` ausente do próprio `heroes.db3` (`item-editor.md`, seção 5).
- Onde rodam a IA e a física do monstro (seção 6).
- O conteúdo da pasta `models/player/delia` no 1.69 (seção 7.4).
- Se a tela de criação permite escolher a variante sem patch (seção 7.5).
