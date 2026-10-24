# Roteiro de testes no cliente — B3 progressão de equipamento

Bloco B3 (atributos de item, Craft, Enhance, Enchant e complementos: slots, reparo completo, armazenamento pessoal e SkillEnhance). O item 5 (Combine Craft e habilidades de item) foi adiado pelo operador. Gate fechado em 24/10/2026 (resultado no `MIGRATION.md`). Nenhuma entrada foi testada no cliente ainda; as do B1 e do B2 também continuam pendentes (`docs/client-tests/B2-fases.md`).

## Preparação
1. Conteúdo privado regenerado no B3: `.cache/private-content/item-enhance.json` (formato 6; melhoria, encantamento, manufatura, qualidade e SkillEnhance). A cópia do cliente (`.cache/play-client`) já recebeu os valores originais de `EnhanceInfo`, `EnchantInfo` e `ManufactureRecipeInfo` (`py -3.13 scripts/client/apply_original_content.py --client-copy .cache/play-client` deve listar 0 pendências).
2. Runtime no Docker: `docker compose up -d --build --wait` (recompila a imagem, aplica as migrações até a 0038 e sobe `bootstrap`, `server` e `game`). Conferir `http://127.0.0.1:8081/en-eu-x-gm/EndPoint.txt` (SERVER1) antes de abrir o cliente.
3. Personagem: a mesma Fiona inicial do B0–B2. Com ela **deslogada** (o `lab-give-item.ps1` recusa personagem conectado), conceder:
   - ouro: `pwsh scripts/dev/lab-give-item.ps1 -ItemClass gold -Count 200000`;
   - pergaminhos de encantamento: `-ItemClass "enchant_scroll[PREFIX:bitter]" -Count 1` e `-ItemClass "enchant_scroll[SUFFIX:kobold]" -Count 1`;
   - melhoria: a pedra da arma inicial (para espada longa nível 1, `-ItemClass enhance_stone_lvl1 -Count 20`; a janela do ferreiro mostra a pedra exigida para outra arma);
   - manufatura (receita `iron_ore_lvl2`, ofício `metal_weapon`): `-ItemClass iron_ore_lvl1 -Count 10` e `-ItemClass metal_solvent_lvl1 -Count 5`;
   - reparo completo: uma segunda peça igual a uma peça da bolsa (mesma classe);
   - SkillEnhance: `-ItemClass skillenhance_use_stamina_decrease_lv1 -Count 3`, `-ItemClass life_erg_crystal -Count 10` e, para o caso com runa, `-ItemClass skill_enhance_rune_gift_lv1 -Count 1`.
4. Alternativa nativa (depuração): `docker compose stop game bootstrap` e `scripts/dev/start-lab-frontend.ps1` com o perfil e os assets de sempre; logs em `.cache/verification`.

## Ordem de gameplay (uma sessão)
1. Entrar na cidade e abrir o inventário: B3.0a (pergaminhos com atributo) e as abas atuais (B3.4c, antes do comando).
2. Ferreiro: melhorar a arma até falhar e restaurar pelo depósito: B3.2c, B3.2b (atributos e EquipmentInfo da peça melhorada), B3.2d; depois o reparo completo da peça que perdeu durabilidade máxima: B3.4b.
3. Encantamento: B3.3 com os dois pergaminhos.
4. Ofício: aprender e fabricar no NPC de ofício: B3.1a e B3.1b (conferir a qualidade e o nome de fabricante).
5. Histórias que abram slot de equipamento e aba de armazenamento, se a progressão chegar nelas (ou um NPC de teste): B3.4a e B3.4c.
6. Habilidades: abrir o aprimoramento numa habilidade da Fiona que aceite o grupo 2 (ex.: `skill_fiona_smash*`), usar Erg, confirmar com sucesso e com falha: B3.4d-1.
7. Uma missão curta usando a habilidade aprimorada e, na volta, restaurar a durabilidade: B3.4d-2.
8. Relogar no fim e conferir tudo o que as entradas listam como preservado (níveis de melhoria/encantamento, itens restaurados, ofício e receitas, itens fabricados, slot/aba abertos, aprimoramento e durabilidade).

## Como relatar
Para cada linha marcada: ID da entrada e número da linha (ex.: `B3.2c-3`), o que foi feito, o que apareceu na tela (texto do diálogo/mensagem; captura se possível) e o horário aproximado (hh:mm), para comparar com `docker compose logs game` (ou os logs nativos em `.cache/verification`). Linhas não testadas ficam desmarcadas com o motivo (ex.: "história não alcançada").

### B3.0a — Itens com atributos no inventário (pergaminhos de encantamento)
Pré-condição: Fiona **deslogada**; conceder pergaminhos com atributo e ouro: `pwsh scripts/dev/lab-give-item.ps1 -ItemClass "enchant_scroll[PREFIX:bitter]" -Count 1`, de novo com `-ItemClass "enchant_scroll[SUFFIX:kobold]"` e `-ItemClass gold -Count 10000`. Containers recompilados (`docker compose up -d --build --wait`).
- [ ] Entrar no jogo e abrir o inventário → os dois pergaminhos aparecem com o nome do encantamento (prefixo "Bitter", sufixo "Kobold") no tooltip, não como pergaminho genérico.
- [ ] Vender um pergaminho numa loja de NPC → ouro pelo preço de venda; a aba de recompra mostra o pergaminho com o mesmo encantamento; recomprar → volta ao inventário com o encantamento.
- [ ] Relogar → os pergaminhos continuam com o encantamento.
- [ ] (Se cair numa fase) um `enchant_scroll[...]` de drop de monstro é recolhido e aparece com o encantamento.
Limites conhecidos: usar o pergaminho para encantar ainda não existe (B3.3) e é recusado; equipamento com atributos (ENHANCE, QUALITY, PREFIX/SUFFIX) ainda é recusado na entrega e no carregamento até o cálculo de ItemStat (B3.2/B3.3); destruir por história/consumo uma classe que tenha itens com atributo é recusado (a ordem do DestroyComparer usa o valor do item); EXPIRE e MIN_UPTIME em pedidos de entrega são recusados.

### B3.2b — Equipamento melhorado (ENHANCE) no inventário e vestido
Pré-condição: Fiona **deslogada**; `pwsh scripts/dev/lab-give-item.ps1 -ItemClass longsword_swordwind -Count 1` e `pwsh scripts/dev/lab-give-item.ps1 -ItemClass "longsword_swordwind[ENHANCE:6]" -Count 1`. Containers recompilados com o perfil `item-enhance.json` (já referenciado pelo `docker/lab/minimum-creation-assets.json`).
- [ ] Entrar e abrir o inventário → a segunda espada aparece como "+6" no nome/tooltip, com ATK maior que a espada comum e durabilidade máxima maior.
- [ ] Equipar a espada +6 → a janela de personagem mostra o ATK somado ao da melhoria; a arma exibe o efeito visual de melhoria (Effect 6 do `EnhanceInfo`).
- [ ] Trocar pela espada comum e de volta → os atributos sobem e descem conforme a peça; reparar a +6 num ferreiro custa mais que a comum com o mesmo desgaste (o valor do item soma os pontos de `RepairEnhanceInfo`).
- [ ] Relogar → a espada continua +6, equipada, com os mesmos atributos.
Limites conhecidos: melhorar no ferreiro ainda não existe (B3.2c); QUALITY, PREFIX/SUFFIX e os outros atributos de equipamento seguem recusados; classes `_lvlN` equipáveis (que o original converte em ENHANCE) seguem recusadas.

### B3.2c — Melhorar no ferreiro (Enhance)
Pré-condição: Fiona **deslogada**; ouro e materiais: `pwsh scripts/dev/lab-give-item.ps1 -ItemClass gold -Count 200000`, `-ItemClass enhance_stone_lvl1 -Count 60`, `-ItemClass enhance_elixir -Count 5` e uma arma `-ItemClass longsword_swordwind -Count 2`. Chances originais (`EnhanceServerInfo`): +1 a +3 sempre; +4/+5 75%; +6 a +8 50%; +9 a +12 40%; +13 a +15 33%.
- [ ] Ferghus → Enhance: escolher a espada **na bolsa**, colocar a pedra pedida e confirmar → +1 com sucesso; ouro e pedras descontados pelo valor da janela; o nome passa a "+1".
- [ ] Repetir até falhar em +4/+5 → mensagem de falha "RankDown" e a espada volta um nível; a durabilidade máxima pode cair (chance pelo nível).
- [ ] Em +6 a +8 uma falha volta a +0 ("RankReset").
- [ ] Ao chegar a +9 com sucesso, a espada fica vinculada ao personagem (não negociável).
- [ ] Uma falha a partir de +9 destrói a espada ("Destroy"): ela some da bolsa, uma parte dos materiais volta e o ferreiro avisa a destruição.
- [ ] Tentar melhorar dentro de uma fase → mensagem "não é possível fabricar na dungeon".
- [ ] Relogar → nível de melhoria, ouro e materiais preservados; a espada destruída não volta à bolsa.
Limites conhecidos: melhorar uma peça **vestida** ainda é recusado (sem resposta); a lista de restauração (`QueryRestoreItemList`) e o restaurar com AP vêm na próxima etapa — a peça destruída fica guardada no depósito de restauração (até 5) mas ainda não aparece na janela; o evento "enhance a shot", runas sem extra e a sobra que iria por correio são recusados; a chance não mostra a mensagem de depuração de GM.

### B3.2d — Restaurar item destruído (depósito de restauração)
Pré-condição: uma peça destruída numa falha de melhoria a partir de +9 (B3.2c) e AP suficiente (o preço em AP aparece na janela).
- [ ] Abrir a janela de restauração no NPC → a peça destruída aparece com o nível de melhoria e o preço em AP.
- [ ] Restaurar → o AP desce pelo preço, a peça volta à bolsa com o mesmo nível de melhoria, vinculada e marcada como já restaurada; a lista fica vazia.
- [ ] Destruir de novo a mesma peça → como já foi restaurada, ela some de vez e aparece o aviso de restauração.
- [ ] Sem AP suficiente → falha da compra, a peça continua na lista.
- [ ] Relogar → a lista e a peça restaurada se mantêm.
Limites conhecidos: com a bolsa cheia a restauração é recusada (o original mandaria por correio); peças com validade não são restauradas.

### B3.3 — Encantar no NPC (Enchant)
Pré-condição: uma peça de equipamento na bolsa (não vestida), um pergaminho de encantamento compatível com ela (lab: `scripts/dev/lab-give-item.ps1` com `enchant_scroll_...[PREFIX:<classe>]`) e ouro para o preço da janela.
- [ ] Abrir a janela de encantamento com a peça e o pergaminho → aparecem a chance atual e a meta do minijogo.
- [ ] Jogar os dados (dado comum do NPC) → o valor atual sobe e a chance acompanha; o dado é consumido.
- [ ] Encantar com sucesso → o pergaminho some, a peça mostra o prefixo/sufixo e os bônus; as condições não cumpridas aparecem apagadas.
- [ ] Falhar → o pergaminho some; a peça perde durabilidade máxima (ou é destruída e vai para o depósito de restauração nos encantamentos que destroem).
- [ ] Cancelar → a janela fecha sem consumir nada.
- [ ] Relogar → o encantamento e os bônus se mantêm.
Limites conhecidos: peça vestida é recusada; encantamentos com validade e o `enchant_elixir_br` são recusados; numa missão aparece "não é possível no calabouço".

### B3.1a — Aprender e subir o ofício (manufatura) com um NPC
Pré-condição: personagem sem ofício; um NPC de ofício (NIEVE, BARAX, ARTHYEN, JARLATH ou CARYL, conforme a cidade) com as opções de aprender ofício e de subir grau.
- [ ] Entrar no jogo → a janela de ofício abre sem erro (lista vazia).
- [ ] No NPC, escolher aprender o ofício → o ofício aparece no grau 0 com 0 de experiência; o diálogo continua.
- [ ] Escolher aprender de novo → o NPC responde com a falha do comando (já aprendido) e nada muda.
- [ ] Escolher subir de grau sem a experiência máxima do grau → resposta de falha, o grau não muda.
- [ ] Relogar → o ofício e as receitas aprendidas continuam na janela.
Limites conhecidos: `remove_manufacture` e a entrega de receitas por quest (`AutoGiveQuest`) ainda não existem; fabricar é a B3.1b.

### B3.1b — Fabricar um item (ofício)
Pré-condição: ofício aprendido (B3.1a), uma receita aprendida (ou listada sempre) com a experiência e o grau exigidos, e os materiais e o ouro da receita na bolsa.
- [ ] Abrir a janela de fabricação e fabricar a receita → os materiais somem uma vez, o item aparece na bolsa e aparece a mensagem de experiência ganha (ganho, atual, máximo do grau).
- [ ] Equipamento fabricado → mostra a qualidade sorteada (e o nome do personagem como fabricante); os atributos do equipamento refletem a qualidade.
- [ ] Fabricar sem material → diálogo de material insuficiente, nada muda.
- [ ] Fabricar uma receita acima do grau → diálogo de receita indisponível.
- [ ] Relogar → o item fabricado, a qualidade e a experiência do ofício se mantêm.
Limites conhecidos: Combine Craft (peças, item 5 do B3) não responde; bolsa cheia recusa a fabricação (o original mandaria a sobra por correio); nomes de personagem fora do ASCII imprimível recusam o equipamento fabricado.

### B3.4a — Expandir slot de equipamento pela história
Pré-condição: uma história ou NPC que execute `open_equipment_slot` para uma parte travada (parte com `IsExpandedSlot`, mostrada como bloqueada na janela de equipamento).
- [ ] Antes do comando → a parte aparece bloqueada e não aceita equipamento.
- [ ] Executar o comando → a parte desbloqueia na hora (janela de equipamento atualizada) e o diálogo continua.
- [ ] Executar de novo → nada muda e o diálogo continua.
- [ ] Equipar um item nessa parte → funciona.
- [ ] Relogar → a parte continua desbloqueada.

### B3.4b — Reparo completo (durabilidade máxima)
Pré-condição: duas peças da mesma classe na bolsa (não vestidas), uma com durabilidade máxima reduzida (por exemplo por falhas de melhoria/encantamento), e ouro para o preço mostrado.
- [ ] No NPC de reparo, escolher o reparo de durabilidade máxima com a peça danificada como alvo e a outra como material → o ouro desce pelo preço, a peça material some e a durabilidade máxima do alvo sobe (até o limite); aparece a mensagem de sucesso com a quantidade recuperada.
- [ ] Usar uma peça de outra classe como material → diálogo de material diferente, nada muda.
- [ ] Usar como alvo uma peça já com durabilidade máxima cheia → diálogo de reparo impossível.
- [ ] Sem ouro suficiente → diálogo de ouro insuficiente, nada muda.
- [ ] Relogar → a durabilidade máxima reparada se mantém.
Limites conhecidos: peças vestidas (alvo ou material) não respondem.

### B3.4c — Abrir aba de armazenamento pessoal (`give_storage`)
Pré-condição: uma história ou NPC que execute `give_storage` para uma aba extra (2–5, 7 ou 8).
- [ ] Antes do comando → a aba aparece fechada/indisponível; mover item para ela falha com o diálogo de movimento.
- [ ] Executar o comando → a aba abre na hora, sem validade, e o diálogo continua.
- [ ] Mover um item para a aba nova → o item fica lá.
- [ ] Receber itens com as abas padrão cheias → os itens vão para a aba nova.
- [ ] Relogar → a aba continua aberta com os itens.
Limites conhecidos: abas com validade (itens de aba extra), renomear/marcar abas (`UpdateStorageStatus`) e o depósito compartilhado não foram migrados; um personagem com uma aba nesses estados não carrega o inventário.

### B3.4d-1 — Aprimorar habilidade (SkillEnhance)
Pré-condição: uma habilidade aprendida que aceite o grupo da pedra (`SkillInfo.SkillEnhanceGroup`), uma pedra de aprimoramento (`skillenhance_*`) e cristais de Erg (`*_erg_crystal`) na bolsa; opcionalmente uma runa `skill_enhance_rune_*`.
- [ ] Abrir o aprimoramento da habilidade com a pedra → a janela abre com chance 0 e o máximo da pedra.
- [ ] Usar cristais de Erg → os cristais somem e a chance sobe (sem passar do máximo); usar mais quando já está no máximo → mensagem de chance máxima.
- [ ] Confirmar com sucesso → a pedra some, a habilidade mostra o aprimoramento (lista de habilidades e janela).
- [ ] Confirmar com falha sem runa → a pedra some e a habilidade fica com a penalidade.
- [ ] Confirmar com falha usando runa → a runa some, a habilidade perde o aprimoramento e chega uma pedra protegida (runa comum em pedra não protegida); com runa premium e pedra protegida a pedra é mantida.
- [ ] Fechar a janela sem confirmar → nada é consumido além dos cristais já usados.
- [ ] Relogar → o aprimoramento continua na lista de habilidades e entra na batalha (MicroPlay).
Limites conhecidos: durabilidade do aprimoramento (gasto em batalha e restauração) é a B3.4d-2; bolsa cheia para a pedra protegida recusa a confirmação (o original mandaria por correio).

### B3.4d-2 — Durabilidade do aprimoramento de habilidade
Pré-condição: uma habilidade com aprimoramento bem-sucedido (B3.4d-1) e uma pedra do mesmo tipo na bolsa.
- [ ] Usar a habilidade em um calabouço (fora do modo prática) → a durabilidade do aprimoramento cai (janela/tooltip da habilidade).
- [ ] Gastar até zerar → aparece a mensagem de durabilidade esgotada da habilidade.
- [ ] Restaurar com a pedra → a pedra some, a durabilidade volta pelo valor de reparo da pedra e aparece a mensagem com antes/depois.
- [ ] Restaurar com durabilidade cheia → mensagem de durabilidade já cheia, nada consumido.
- [ ] Relogar → a durabilidade se mantém.
Limites conhecidos: a poção de durabilidade máxima (`skill_enhance_max_durability_potion`) não foi migrada; em grupo (B5) o host não repassa o gasto aos outros jogadores.

### B3.5 — Abrir caixa de presente (RandomItem)
Pré-condição: uma caixa do sistema antigo na bolsa (ex.: `randomitem_levelup_gift_lvl10`, "Level Up Gift (Lv 10 or higher)") e o nível exigido pela caixa (10 nesse exemplo); espaço livre na bolsa.
- [ ] Abaixo do nível → clique direito em "usar" mostra o diálogo de nível insuficiente e a caixa fica.
- [ ] Com o nível → a caixa some (uma unidade), aparecem a mensagem "item usado", uma mensagem por item recebido e a janela de recompensas; para a Fiona, a caixa de nível 10 dá `randomitem_levelup_v2_15`, 3 `energy_potion_lvl1`, `longsword_saver` e `levelup_belt_lvl10`.
- [ ] Relogar → os itens recebidos continuam na bolsa e a caixa não volta.
Limites conhecidos: caixas com chave, Titicore, classes raras/pacote e a sobra por correio com a bolsa cheia não respondem; o anúncio global de item raro não é enviado.

### B3.6 — Abrir caixa de presente (RandomItemV2)
Pré-condição: `randomitem_levelup_v2_10` ("Level Up Gift (Lv 10 or higher)") na bolsa ou na aba de cash; personagem nível 10 ou mais; espaço livre na bolsa.
- [ ] Abaixo do nível → diálogo de nível insuficiente; a caixa fica.
- [ ] Com o nível → confirmar o uso: a caixa some, aparecem "item usado", uma mensagem "recebeu" por item e a janela de recompensas; para a Fiona: `levelup_belt_lvl10`, 3 `cashshop_trade_coin` (se a feature `ingame_cashshop_preshow` estiver ligada), 20 `life_potion_lvl2_gift`, `randomitem_levelup_v2_15` e `longsword_saver`.
- [ ] A `randomitem_levelup_v2_15` recebida, com nível 15, abre do mesmo jeito (as `phoenix_feather` chegam vinculadas).
- [ ] Relogar → os itens continuam e a caixa não volta.
Limites conhecidos: caixas V2 com chave, Titicore, tempo de recarga, exclusivas, raras ou em sequência, recompensas de AP ou com validade e a sobra por correio não respondem; o anúncio global não é enviado.

### B3.7 — Equipamento com validade, Enhance do item equipado e craft do Ferghus
Pré-condição: `levelup_belt_lvl10` (Hero Belt) na bolsa; `longsword_saver` equipada com `enhance_stone_lvl1` e ouro; os materiais da receita do Ferghus (5 `iron_ore_lvl1`, 1 `skin_lvl1`, 2 `chieftain_hammer`).
- [ ] Equipar o Hero Belt → ele vai para o slot do cinto e os atributos mudam; quando a validade acaba, o cinto continua no slot sem dar atributos.
- [ ] Enhance da Sword of Salvation equipada no Ferghus → resultado da melhoria, a espada fica equipada e os atributos do personagem atualizam.
- [ ] Craft da Sword of Salvation no Ferghus → os materiais saem, a espada aparece na bolsa; sem materiais → diálogo de material insuficiente.
- [ ] Relogar → cinto equipado, melhoria e espada fabricada continuam.
Limites conhecidos: Enhance que destruiria um item equipado é recusado sem resposta; a missão diária "TryCraftItem" não conta.

