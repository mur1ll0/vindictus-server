# Criação da Miri (classe 11) pela grade do laboratório 1.69

Registro dos passos `MG<n>` (criação pela grade de classes da Miri). O fluxo genérico da grade (registro do *handler*, `changemap lobby_beautyshop_change`, `Select<Classe>` → `npc_character_create_select <n>`, estado 11, customização, estalagem, byte de prólogo, "Go to town") é o da Delia (`docs/future/delia-class-port.md` 4.5 e 4.5.1) e já vale para a classe 11 pelo `EXTRA_CLASSES` (`creation_names()`, `TABLES`, `SERVER_CLASS_BOUNDS`, `SWITCHES` 0x384A3308). Este documento cobre só o que dependia da classe. Evidência detalhada e *hashes* no `docs/history/session-log.md` (busca por `MG1`).

## 1. O que faltava (medido em 02/11/2026)

| Peça | 1.69 | NA | Escolha |
|---|---|---|---|
| Filme da grade `scaleform/NewCharacterDialog_V2.gfx` | 11 classes fixas no AS2: `characterTypeArray`, botões `MC_SelectCharacter.MC_Button_<Classe>` (grade de 3 colunas em x 0/72/144 e linhas y −274/−136/2/140; a casa x 144, y 140 está livre), painéis `MC_Video.MC_<Classe>` (todos o *sprite* 42), *switches* por nome em `addCharacterInfo` (comentário), `isEnableCharType`, `showRolloverPanel` e `showCharacterSummary` (nome e arma) | O mesmo filme com 28 classes (`MC_Button_Miri`, *sprite* 141), mas textos, vídeos e armas vêm do C++ (`SetCharaterIntroMovie`, `SetCharaterSkillMovie`, `SetNewIconCharacter`…; `scripts/new_character_info.txt`), que o `client.dll` 1.69 não tem | **Editar o filme do 1.69** (seção 2): mudança só de conteúdo, reversível pelo *backup* do pacote, sem código nativo novo. O filme do NA exigiria implementar no `client.dll` 11 chamadas C++ → filme e 5 tratadores (`new-classes.md` 8.8), risco bem maior |
| Ator da cena `maps/lobby_beautyshop_change.bsp` | 11 `npc_character_creation_actor` (`npc_<classe>`) | 28, inclusive `npc_miri` com `csf/createcharacter_miri.csf` (e `variation npc_vella`) | Copiar o bloco do ator da Delia com o `.csf` e o nome da Miri (seção 3) |
| Pose `ACT_CHARACTER_CREATION_SELECTED_S3` | — | presente no `pc_miri_town_motion.mdl` do NA (convertido no M1) | Nada a fazer: o modelo preparado já tem a atividade (e `..._IDLE_S3`) |
| Vídeo de apresentação | o filme monta `video/createcharacter/_eneu_NewCharacter_<Classe>.usm` (cliente EN-EU) | só `_usa_` e `_deu_` da Miri | Cópia do `_usa_` (inglês) com o nome `_eneu_` (seção 4) |
| Roupas de amostra | `switch` 0x384A3308 clonado pela tabela (`CsfChanged.create_miri`, `trial_miri1..4`) | `create_miri.csf`, `queen_after_trial_miri1..5.csf` | Já preparados no M1; nada a fazer |
| Modelo na customização | chave de nascimento (`TABLES` 0x40CFA1B0, `SERVER_CLASS_BOUNDS`) com a Miri desde o M5 | — | Nada a fazer |
| Servidor Rust | criação da classe 11 pelos dados do M3 (`class-11-initializer*.json`, `minimum-creation-assets.json`) | — | Conferido (seção 5) |

## 2. Filme da grade (`gfx_patch`, aplicado por `apply_lab_content_additions.GRID_CLASSES`)

O pacote do 1.69 que tem o filme é reconstruído do *backup* (mecanismo `ORIGINAL_FIXES`, o mesmo do retrato da seleção no M7o). Duas mudanças:
- `with_class_button`: botão `MC_Button_Miri` (cópia do *sprite* do botão da Delia, mesmos quadros e *hit box*, com a forma do ícone trocada) na casa livre (144, 140), profundidade seguinte da grade; ícone num atlas próprio `NewCharacterDialog_V2_IMiri.dds` (128×256, DXT5) com o recorte 70×136 que o `MC_Button_Miri` do NA mostra (atlas `NewCharacterDialog_V2_IA.dds`, (895, 207)–(965, 343)); painel `MC_Video.MC_Miri` (o *sprite* dos painéis, profundidade logo acima da Delia).
- `with_root_actions` + `grid_class_actions`: AS2 acrescentado no fim do bloco de ações da raiz (depois dos laços do quadro; nenhum deslocamento anterior muda), montado por um montador AVM1 mínimo (`As2`):
  - `characterTypeArray[11] = 'Miri'`;
  - `LocalizedText` das chaves `GameUI_Heroes_NewCharacterDialog_Type_Miri`, `..._Type_Comment_Miri` e `Heroes_Item_Equip_Class_Name_DRAKER` (textos importados do NA no M2);
  - o que o primeiro laço faz por botão: `applyButtonEventEx(botão, OnChoose)`, `charactorType = 11` e os *handlers* `onRollOut`/`onRollOver` do botão da Delia (eles leem o próprio botão por `this`);
  - o que o segundo laço faz por painel (oculto, `addCharacterInfo('Miri')`), mais o comentário (o *switch* de `addCharacterInfo` o deixa vazio para um nome que não conhece) e os cinco vídeos de habilidade do `new_character_info.txt` do NA (`dragon_striker`, `sp3`, `sp2`, `sp4`, `sp5`; legendas `GameUI_Heroes_CreateCharacter_SkillPreview_Miri_01..05`);
  - `showRolloverPanel` e `showCharacterSummary` embrulhadas: chamam a original e, para a Miri, escrevem o nome e a arma (os *switches* deixariam os da última classe mostrada).
- `isEnableCharType` não é chamada depois dos laços; o vídeo de apresentação vem do laço genérico do `onLoad`, que já percorre o `characterTypeArray` com a Miri.
- O C++ já trata `SelectMiri` (`CREATION_GRID`, `creation_names()`), então nenhum DLL muda neste passo.

## 3. Ator da cena (`bsp_entities.with_creation_actors`)

Bloco do `npc_delia` copiado com `target_csf_name csf/createcharacter_miri.csf` e `targetname npc_miri`, acrescentado ao *lump* de entidades; o *lump* aumentado vai para o fim do arquivo e só a entrada 0 do cabeçalho muda (nenhum outro *lump* nem os deslocamentos absolutos do *game lump* se movem). `variation` fica `npc_delia`: nenhum dos dois clientes tem `npc_miri` em `scripts/npc_variations.txt` (por isso o NA usa `npc_vella`, que traz o modelo da Vella); a da Delia não tem modelo, só os ajustes de ator, e a aparência vem do `.csf`. Origem e ângulo são os da Delia; o servidor teleporta os atores aos pontos `npc_character_create_select`/`_nonselect` de qualquer forma.

## 4. Preparação (`stage_na_class.py`)

- `GRID_ICON_ATLAS`: atlas do ícone a partir do filme e do atlas do NA (`gfx_patch.button_icon`, `class_icon_dds`), registrado no `class-assets.json` com origem, SHA-256 e o retângulo.
- `GRID_INTRO`: `_eneu_newcharacter_miri.usm` = cópia do `_usa_newcharacter_miri.usm` do NA (`copied_from`).
- Os vídeos de habilidade, o `createcharacter_miri.csf`, o `create_miri.csf` e os `queen_after_trial_miri*.csf` já vinham da preparação do M1.

## 5. Servidor

Nada mudou no servidor: a criação da classe 11 usa os dados do M3 (`class_additions.json`: `draker_newbie`, `skill_miri_draker_mastery` e `skill_miri_speedmove` em rank 1, `miri_inner_free`; `class-11-initializer.json`; `minimum-creation-assets.json` com `class_initializers` 0, 2–11), montados no contêiner `game` a partir de `.cache/private-content` e `docker/lab/`. `LAB_SKIN=16775155 LAB_EYE=14210736 cargo test --test private_class_content -- --ignored` passou para as classes 0–11 (inventário composto, admissão no Gear, aparência inicial e itens de customização). A persistência e a volta à seleção são as mesmas da Delia; o teste de cliente MG1 confirma.

## 6. Aplicação e testes

- Testes: `python -m unittest test_gfx_patch test_bsp_entities test_stage_na_class test_apply_lab_content_additions` (em `scripts/client`): 44 OK (log `.cache/verification/miri-grid/mg1-tests-2.log`).
- Aplicado com o jogo fechado: preparação (`stage_na_class.py --class miri --weapon draker`, 5.320 arquivos) e `apply_lab_content_additions.py --apply` (2 pacotes de classe construídos; correções do 1.69: `NewCharacterDialog_V2.gfx`, `lobby_beautyshop_change.bsp`, além das já existentes). Conferido lendo os pacotes da cópia: o filme tem o `MC_Button_Miri` com o atlas novo e o AS2 no fim da ação raiz; o mapa tem o `npc_miri` com `createcharacter_miri.csf`; o ícone, o vídeo `_eneu_` e os vídeos de habilidade estão no índice.
- DLLs: sem mudança.
- Cliente: entrada MG1 de `docs/client-tests/C4-miri.md`.

## 7. Pendências e aproximações
- Atores da grade continuam sem `EF_NODRAW` e sem a sequência animada da pose selecionada (aproximação geral da grade, Delia 7).
- Vídeo de apresentação em inglês americano (`_usa_`) com o nome EU.

## 8. MG2 — correções da primeira sessão de cliente (MG1, 02/11/2026)

Relato: criação até a cidade e itens iniciais certos; problemas abaixo.

| Relato | Causa medida | Correção |
|---|---|---|
| Texto `video/createcharacter/_eneu_` sobre o vídeo da Miri | o `onLoad` do filme esconde o `Text_CharacterPreviewUsm` (o caminho do vídeo) classe por classe, pelo nome | `grid_class_actions` esconde o da Miri |
| Cabelos pagos selecionáveis na criação; grátis por último | linhas importadas de `CustomizePriceInfo` com preços negativos do NA (marcas de evento, `-202102`) e `Feature 'Miri'` ligada; o EU do 1.69 não tem nenhum preço negativo. O NA numera os grátis 99994–99999 e o cliente lista por `Category, [Order]` | `class_imports.customize_rows`: sem preços negativos importados; grátis da classe numerados 1..n antes dos pagos |
| 23 cabelos carecas (Laura's Bob Cut, Voluminous Ponytail, Pinned Bangs…), 5 com modelo de erro (Side Ponytail, Belly Dancer Ponytail…), 4 com variantes faltando | a entrada `hair` do `player_costume.txt` guarda um prefixo (`pc_fiona_hair_laura_`); o cliente desenha `prefixo + NN.mdl` (uma variante por corte de capacete: 01–04, 08–10). A preparação só seguia valores `.mdl` e a mesclagem só entradas que citam "miri": nem as entradas nem os modelos vieram | `stage_na_class.imported_costumes` aceita prefixos e `costume_prefix_models` traz as variantes do NA; os prefixos são agulhas da mesclagem (validação do aplicador ampliada) |
| *Body type*: Height sem efeito; dois controles `GAMEUI_` (abaixo de Waist e de Shoulders) sem efeito | Height: o `CreateCharacterDialog.gfx` escolhe as faixas de Height/Bust num *switch* 0–10; a 11 caía em 97–103 (altura natural 100), o servidor escala por `valor × 100 / 92` (Miri, como a Delia) e o cliente limita a escala da Miri a 0,865–0,945: ficava sempre no teto. Os dois controles: o `default_miri.bsh` do NA tem Waist 2 e Shoulders 2 com `bodyshape_spine2_thin.fig`/`bodyshape_neck_thin.fig` (sem "miri" no nome, não preparados) e textos ausentes no 1.69 | `gfx_patch.class_alias_actions` (no fim das ações da raiz do `CreateCharacterDialog.gfx` solto, *backup* `.pre-classes.bak`): as quatro funções das faixas tratam a 11 como 10 e devolvem `g_selectedCharacter` a 11; `dependencies` do `.bsh` traz os `.fig`; `class_text` traz os rótulos do `.bsh` |
| Pompom do emote de torcida com modelo de erro | `AE_ATTACH_ENTITY models/effects/mesh/cheer_pom.mdl` só existe no NA; a preparação não seguia modelos citados em scripts | `dependencies` dos `.txt` da classe traz os `.mdl` citados |
| `Heroes_status_name_resound` sobre a Miri | os textos dos estados importados (`resound`, `dragon_trace`…) não citam a classe nem terminam num id importado | `class_text` traz as chaves das colunas `*LocalizedText` das linhas importadas e o nome sem nível (`HEROES_STATUS_NAME_RESOUND` = texto do nível 1) |
| Malha azul da armadura com a pele atravessando (amostra 2) | **não resolvido**. Medido e descartado: esqueletos da Miri (`miri_base`, `town`/`battle`/`_ex`) com os mesmos ossos e a mesma pose de repouso da pele e da armadura (só os *jiggle* das peças faltam, igual à Delia); conversão v45→v44 não toca ossos (a `pytonshock` nem foi convertida); malha íntegra (*checksums* iguais ao NA); ossos *jiggle* no mesmo formato das armaduras do 1.69; *shader* `Heroes_Faceskin` comum no 1.69 | diagnóstico no cliente (MG2 do C4): os botões de amostra 4 e 5 (`queen_after_trial_miri3/4`) usam armaduras nativas do 1.69 (summercasual, biquíni), os 2 e 3 (`trial_miri1/2`) importadas do NA (dragon_rises, pytonshock) |
| Luz de objeto destruído continua na fase | investigação (subagente, só leitura): a luz é uma entidade `light` do mapa, apagada por saída `OnBreak`/`OnHealthChanged → TurnOff` executada pelo `server.dll` local; o servidor Rust não participa. Muitos fogos não têm luz ligada (02a: `campfire01` 18 de 43 com luz nomeada; 02d: fogueira sem saída, luz de estilo 0 assada no *lightmap*), o que é o comportamento do conteúdo original | sem mudança; teste no cliente (MG2): 02a fogueira em (-1166, 1486) deve apagar; 02d em (-7596, -622) continua acesa no original |

## 9. MG3 — resultado da MG2 e figurinos do 1.69 sem modelo (02/11/2026)

- Armadura × pele: só os botões de amostra 2 e 3 (armaduras importadas do NA, `dragon_rises` 790 e `pytonshock` 808, ambas com o *inner* 9000) mostram a pele atravessando; 4 e 5 (armaduras do 1.69) estão certos. Descartado por medição: conversão v45→v44 (as duas armaduras e o *inner* 9000 não foram convertidos; o *inner* 9028, convertido, está certo), ossos (esqueletos e pose de repouso iguais, só os *jiggle* das peças faltam, como na Delia), *jiggle* no mesmo formato, *flexes* (nenhum), malha e *checksums* iguais ao NA, materiais (só `$surface` difere de uma armadura do 1.69), alfa das texturas (a `pytonshock` é DXT1 sem nenhum texel recortado). A medição em repouso por vértice mais próximo (`.vvd`) mostra a pele por fora da `pytonshock` justamente na pelve, coxas e antebraços (as regiões das fotos) com qualquer *inner*, mas armaduras nativas que funcionam dão taxas parecidas: o método não discrimina. **Não resolvido**; próximo passo: análise por triângulo e com pose no Blender (servidor MCP do operador na porta 9876 respondeu `Client timed out`).
- Figurino com estrela de erro (`avatar_female_chinaplate`, nome em coreano): item do próprio 1.69, usável por todas as femininas (máscara 3494), cujos modelos `models/armor/_chn/chinaplate/*` o cliente 1.69 nunca trouxe; o NA tem. O nome em coreano é do conteúdo original (os textos em inglês do 1.69 e do NA o trazem em coreano). Correção: `stage_na_class.inherited_costumes` traz do NA os modelos que faltam das entradas do 1.69 dos itens não arma cuja máscara cobre todas as femininas do 1.69 (29 modelos, 0,6 MB, entre eles chinaplate e hanbok 2014); 34 outros (corpos `tall`/`giant`) não existem em nenhum cliente.
