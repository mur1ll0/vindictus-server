# Roteiro de testes no cliente (Fiona preservada, cliente 1.69 EN)

Lista acumulada do que já foi implementado e verificado por testes automáticos, mas ainda aguarda aceite no cliente real. Marque cada item e anote o horário: os logs do runtime ficam em `.cache/verification/lab-frontend-<tag>.stderr.log` e registram só categorias e tempos, nunca valores.

Pré-condição: runtime iniciado com `scripts/dev/start-lab-frontend.ps1` (perfil privado, prova de nomes e `minimum-creation-assets.json`).

## Já aceitos
- [x] (21/10) Login sem o aviso do "Security Code".
- [x] (21/10) Mover/trocar itens na bolsa.
- [x] (21/10) Barra de atalhos (consumíveis): atribuir, trocar de slot, remover, relogin.
- [x] (21/10) Janela de skills mostra as skills da Fiona e o AP (lista e AP enviados na entrada).
- [x] (21/10) Cápsula de AP: usar consome a pilha e soma AP na janela de skills (as 5 cápsulas foram usadas).
- [x] (21/10) Skills passivas não vão para a barra (comportamento original).
- [x] Login, segunda senha, criação/lista, seleção, cidade e movimento.
- [x] Inventário abre com os itens iniciais.
- [x] Equipar/desequipar os 5 equipamentos; aparência e atributos mudam; relogin preserva.
- [x] Voltar à seleção de personagem (LogOut) e entrar novamente.
- [x] Equipar responde rápido (≈80–300 ms no log `elapsed_ms`).

## Pendentes

### Mover/trocar itens (P0-02d)
- [x] (21/10) Mover e trocar itens na bolsa.
- [ ] (Opcional) Soltar fora da bolsa/aba inválida: aparece "Move item failed" e nada muda.

### Empilhar (P0-02e)
- [ ] Requer duas pilhas da mesma classe na bolsa (só possível após loot/compra). Soltar uma sobre a outra: soma até o limite; a de origem some se zerar.

### Barra de atalhos / quickslots (P0-02f)
- [x] (21/10) Atalhos de consumíveis funcionando.
- [ ] Usar o consumível pela barra só será possível dentro de fase (combate ainda não implementado).

### Treinar/evoluir skills com AP (P0-03d)
- [ ] Gastar AP numa skill que você já tem (Shield Mastery/Guard): o AP cai; ao completar o custo do próximo rank a skill sobe (mensagem de skill concluída) e o AP excedente volta. Shield Mastery rank 2 custa 30, Guard rank 2 custa 30.
- [ ] Abrir a tela do personagem depois de treinar: o AP usado (TotalUsedAP) aparece atualizado (o cliente pede `QueryCharacterCommonInfo`, agora respondido).
- [ ] Relogar: rank, AP restante e AP usado preservados.
- Limite atual: o rank que passa a exigir desbloqueio por história (Shield Mastery rank 7) e meditação ainda são recusados.
- [ ] Aprender skill nova (as outras 2 da lista) exige ouro pelo preço da skill; a Fiona ainda não tem ouro. Fica para a próxima fatia (carteira de ouro + LearnNewSkill).

### Loja Cash / "Depot"
- [ ] Depósito da loja Cash depende do serviço de loja (P3). Não bloqueia os testes atuais.

### Lista de skills (P0-03b)
- [ ] Abrir a janela de skills: aparecem Shield Mastery e Guard no rank 1, com custo do próximo rank e treino disponível. (21/10: corrigido — a lista e o AP agora são enviados na entrada, como o `InGame.OnEnter` original; antes só respondíamos a consultas que o cliente não faz.)

### Atalhos de skill (P0-03c)
- [ ] Arrastar Shield Mastery ou Guard para a barra: o original recusa (são passivas, `UseType = 0`) com o diálogo de dados inválidos; nada é gravado. Skills ativas aprendidas depois poderão ir para a barra.

### AP e cápsula de AP (P0-03a)
- [ ] Abrir a janela de skills: o AP exibido corresponde ao salvo (o cliente pede `QueryAP`).
- [ ] Aba **Cash do inventário** (onde está a caixa "Level Up"; não é o "Depot"): há uma pilha de **5× cápsula de AP 50** (concedida para teste em 21/10/2026). O "Depot" é o depósito da loja Cash (serviço de loja, P3) e continua desabilitado.
- [ ] Usar uma cápsula: a pilha cai para 4 e o AP sobe 50; reabrir a janela de skills confirma.
- [ ] Relogar: pilha e AP preservados.
- [ ] Usar até acabar: a pilha some da aba Cash.

## Como relatar
Para cada falha: o que fez, o que apareceu na tela e o horário aproximado. Para lentidão: o horário, para comparar com `elapsed_ms`.
