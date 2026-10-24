# Roteiro de testes no cliente — B3 progressão de equipamento

Bloco B3 (atributos de item, Craft, Enhance, Enchant e complementos). Roteiro em construção: a preparação, a ordem de gameplay e as instruções de relato completas entram no gate do bloco.

### B3.0a — Itens com atributos no inventário (pergaminhos de encantamento)
Pré-condição: Fiona **deslogada**; conceder pergaminhos com atributo e ouro: `pwsh scripts/dev/lab-give-item.ps1 -ItemClass "enchant_scroll[PREFIX:bitter]" -Count 1`, de novo com `-ItemClass "enchant_scroll[SUFFIX:kobold]"` e `-ItemClass gold -Count 10000`. Containers recompilados (`docker compose up -d --build --wait`).
- [ ] Entrar no jogo e abrir o inventário → os dois pergaminhos aparecem com o nome do encantamento (prefixo "Bitter", sufixo "Kobold") no tooltip, não como pergaminho genérico.
- [ ] Vender um pergaminho numa loja de NPC → ouro pelo preço de venda; a aba de recompra mostra o pergaminho com o mesmo encantamento; recomprar → volta ao inventário com o encantamento.
- [ ] Relogar → os pergaminhos continuam com o encantamento.
- [ ] (Se cair numa fase) um `enchant_scroll[...]` de drop de monstro é recolhido e aparece com o encantamento.
Limites conhecidos: usar o pergaminho para encantar ainda não existe (B3.3) e é recusado; equipamento com atributos (ENHANCE, QUALITY, PREFIX/SUFFIX) ainda é recusado na entrega e no carregamento até o cálculo de ItemStat (B3.2/B3.3); destruir por história/consumo uma classe que tenha itens com atributo é recusado (a ordem do DestroyComparer usa o valor do item); EXPIRE e MIN_UPTIME em pedidos de entrega são recusados.
