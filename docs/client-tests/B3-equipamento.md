# Roteiro de testes no cliente — B3 progressão de equipamento

Bloco B3 (atributos de item, Craft, Enhance, Enchant e complementos). Roteiro em construção: a preparação, a ordem de gameplay e as instruções de relato completas entram no gate do bloco.

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
