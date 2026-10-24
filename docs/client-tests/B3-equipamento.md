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
