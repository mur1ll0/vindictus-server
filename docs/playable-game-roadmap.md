# Caminho incremental para o jogo funcionar

Prioridade do operador em 20/10/2026: primeiro o ciclo PvE jogável. PvP, guildas e eventos ficam para depois. Este roteiro organiza a integração no cliente e reutiliza os módulos de banco/domínio já recuperados. A migração integral M01–M24 permanece como objetivo.

## Estado confirmado

- Create/relogin da Fiona mínima e seleção/entrada na cidade/movimento foram aceitos no cliente 1.69 EN.
- A criação persistiu 11 itens, 23 registros de aparência e 2 skills. Diagnóstico somente leitura confirmou 5 equipamentos na bolsa, com 0 partes equipadas. Os itens foram sincronizados e aceitos no inventário; equipamento vestido depende de P0-02.
- O inventário abriu e exibiu os equipamentos iniciais no teste aceito. Próximo bloqueio: equipar/desequipar e carregar equipamento persistido. Não apagar/recriar a Fiona nem entregar defaults novamente para contornar esses sintomas.
- QueryInventory/InventoryInfo foram integrados e aceitos; mutações de inventário e SkillList ainda precisam de integração. Contratos/tabelas recuperados não equivalem a funcionalidades disponíveis no cliente.

## P0 — personagem utilizável na cidade

1. **P0-01 / M15.1c.3 — inventário e equipamento inicial (leitura aceita no cliente em 20/10/2026).** Recuperar QueryInventory → InventoryInfo, storages, slots, detalhes dos itens, equipamentos e quickslots. Conferir os itens/slots persistidos contra o perfil original e sincronizar aparência/stats a partir do equipamento real. Separar leitura inicial de mutações. Aceite: inventário abre sem download infinito, mesmos itens/quantidades, equipamentos corretos e relogin preservado, sem duplicar defaults.
2. **P0-02 — ações básicas de inventário.** Equipar/desequipar, mover slots, empilhar/separar, usar consumível e atualizar aparência/stats/carga. Validar requisitos, slots incompatíveis, propriedade e concorrência. Aceite: trocar equipamento, usar consumível e relogar mantendo os resultados; recusas não perdem nem duplicam itens.
3. **P0-03 — skills e barra de ações.** Entregar SkillList/AP/SP e quickslots persistidos; habilidades iniciais, efeitos passivos, custos, cooldowns, aprendizado/evolução conforme contratos recuperados. Aceite: skills iniciais visíveis, atalhos utilizáveis, recursos coerentes e relogin sem reset indevido.

## P1 — primeiro ciclo PvE completo

4. **P1-01 — NPCs essenciais e diálogos.** Presença/interação, conversa e navegação; estado mínimo das histórias exigidas pelo percurso inicial. Loja, compra/venda e reparo necessários ao ciclo. Não habilitar histórias de eventos para substituir o caminho normal. Aceite: conversar com o NPC inicial e abrir/fechar serviços sem travar a interface.
5. **P1-02 — quests iniciais.** Oferta, aceite, objetivos, acompanhamento, conclusão, recompensas e desbloqueios; persistência do estado de conversa/história. Aceite: aceitar a primeira quest, acompanhar um objetivo e manter o estado no relogin. Antecipar correio/conta apenas se uma dependência real exigir.
6. **P1-03 — acesso à primeira fase.** Quadro/barco, seleção de batalha, requisitos, criação/admissão na instância, carregamento, spawn, setores/portais e retorno à cidade. Recuperar a integração original com a autoridade da simulação/serviços de batalha. Aceite: partir solo, carregar a fase correta e retornar; timeout/desconexão não deixam personagem preso.
7. **P1-04 — combate e inimigos.** Spawn/AI, aggro, ações/hits, dano, bloqueio/esquiva, HP/stamina/SP, morte/revive e chefe; autoridade e sincronização dos resultados. Aceite: concluir combate real usando equipamento/skills, incluindo falha/morte, sem resultados fictícios.
8. **P1-05 — loot, vitória e progressão.** Drops/coleta, ouro/itens, XP/level/AP, objetivos de quest, conclusão/falha e recompensas; gravação consistente sem reaplicar recompensas no retry. Aceite: cidade → quest → fase → combate → recompensa → NPC → relogin conservando todos os resultados.
9. **P1-06 — robustez desse ciclo.** Logout/reconexão, queda durante missão/recompensa, recuperação de sessão, limites de pacote, propriedade e operações atômicas. Aceite: repetir ciclo/cancelamento/desconexão sem duplicação, perda de progresso ou loading infinito.

## P2 — ampliar o PvE e a economia básica

10. **Craft e coleta.** NPC/ofício, receitas/desbloqueios, materiais, custos, fabricação/filas e resultado persistido. Depende de inventário, NPCs e obtenção real de materiais. Aceite: fabricar uma receita normal consumindo recursos uma única vez e relogar com o resultado.
11. **Evolução de equipamento e armazenamento.** Durabilidade/reparo completo, melhoria/enchant/combinação conforme o conteúdo inicial exigir, armazenamento pessoal e expansão de slots. Reutilizar os contratos migrados, com erros/custos originais.
12. **Conteúdo e progressão seguintes.** Mais NPCs, quests, mapas/fases, dificuldades, chefes, receitas, equipamentos/skills; validar desbloqueios em sequência. Outras classes/criação e aparência entram em incrementos próprios após estabilizar o primeiro ciclo da Fiona.
13. **Cooperativo PvE.** Party, convite, instância/combate/loot sincronizados, host/servidor, reconexão e saída de membro. Antecipar apenas infraestrutura comprovadamente necessária ao percurso solo.
14. **Usabilidade e operação essenciais.** Opções/atalhos, erros claros, transições sem bloqueio, contas/sessões fora do perfil mínimo de laboratório, inicialização/restart e diagnóstico. Corrigir imediatamente falhas que bloqueiem incrementos anteriores.

## P3 — depois do ciclo principal

- PvP/arena/ranking competitivo, guildas e seus serviços.
- Eventos sazonais/UserCare e recompensas promocionais.
- Pets, títulos/achievements acessórios, sistemas sociais extensos, leilão/mercado, cash shop e cosméticos opcionais.
- Correio completo e demais serviços periféricos; antecipar somente dependência comprovada do PvE, registrando o motivo.

## Fechamento de cada incremento

- Usar fonte recuperada e fixtures/oracles para fio/comportamento; primeiro verificar o que já existe no domínio/banco.
- Entregar uma fatia pequena do cliente ao armazenamento, com recusas/persistência. Não enviar sucesso fictício ou listas vazias para ocultar serviço obrigatório ausente.
- Testes focados durante a alteração e um gate completo no fim do lote; logs em `.cache/verification`, jobs=1, incremental=0 e arquivos editados em LF.
- Preservar Fiona/marcador/schema e diretórios originais. Validar no cliente real antes de declarar compatibilidade.
- Commits locais por incremento verificado, sem push automático. Registrar novas lacunas aqui e no checkpoint, respeitando as dependências do PvE.

## Compilação e GitHub

O workflow `.github/workflows/verify.yaml`, que executava build/testes em todo push e pull request, foi removido a pedido do operador. A verificação local `scripts/dev/verify.ps1` permanece. A remoção afeta o GitHub quando o commit for enviado ao remoto; um commit local não dispara GitHub Actions. A causa das falhas históricas do workflow não foi diagnosticada neste lote.

Diagnóstico histórico e contrato do incremento de leitura: [minimum-inventory-client.md](minimum-inventory-client.md), com a correção de QueryInventory529/InventoryInfo468 e os 5 equipamentos existentes na bolsa.

P0-01: sincronização inicial e QueryInventory implementados para os 11 itens preservados; inventário e equipamentos na bolsa aceitos pelo operador em 20/10/2026. Reabertura/relogin não confirmados separadamente. Detalhes/limites em [minimum-inventory-client.md](minimum-inventory-client.md). P0-02 equipar/desequipar ainda não habilitado.

P0-02a: contratos EquipItem457/EquipmentInfo458 e kernel de slots preparados, com oracles Framework e testes diferenciais. A ligação ao cliente depende primeiro do loader de equipamento vestido, requisitos, carga e aparência persistida; ainda não habilitada. Sequência e limites: [minimum-equipment-client.md](minimum-equipment-client.md).
