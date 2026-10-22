# Roteiro de testes no cliente — B2 fases e combate

Bloco B2 (EXP/level up, saída para a fase, transporte de batalha, dungeons, monstros, combate, drops, progressão e robustez). Por decisão do operador, a sessão de cliente acontece só no fim do bloco e começa pelo roteiro do B1 (docs/client-tests/B1-cidade.md). A preparação, a ordem de gameplay e o formato de relato são completados no gate.

### B2.0 — EXP, level up e títulos por nível
Pré-condição: perfil `npc_scripts` reexportado com as tabelas de progressão (CharacterClassInfo, LevelUpExpInfo, TitleInfo/TitleGoalInfo). A Fiona ganha EXP no B1 só concluindo uma história com recompensa de EXP; no B2 o EXP vem principalmente das missões (B2.7).
- [ ] Concluir uma história com recompensa de EXP/AP (ex.: uma das histórias "Goodbye"/cut-scene de nível 1, se o NPC oferecer) → a barra de EXP sobe; o AP aumenta (até o máximo).
- [ ] Quando o EXP passa do necessário → efeito de level up na tela, novo nível na janela de personagem, atributos maiores (STR/DEX/INT/WILL/HP) e a lista de títulos ganha os títulos de nível (em progresso, não adquiridos).
- [ ] Após o level up → a lista de quests recebe as quests automáticas do novo nível e histórias de nível novas aparecem no diário (quando existirem).
- [ ] Relogar → nível, EXP, atributos e títulos preservados; a seleção de personagem mostra o novo nível.
Limites conhecidos: títulos adquiridos (com status de concluído e bônus de atributo), recompensa de título/skill de história, log de crescimento (AddGrowthLog), ranking (CharacterRank) e efeitos de guilda não migrados.
