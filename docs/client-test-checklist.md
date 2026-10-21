# Roteiro de testes no cliente (Fiona preservada, cliente 1.69 EN)

Lista acumulada do que já foi implementado e verificado por testes automáticos, mas ainda aguarda aceite no cliente real. Marque cada item e anote o horário: os logs do runtime ficam em `.cache/verification/lab-frontend-<tag>.stderr.log` e registram só categorias e tempos, nunca valores.

Pré-condição: runtime iniciado com `scripts/dev/start-lab-frontend.ps1` (perfil privado, prova de nomes e `minimum-creation-assets.json`).

## Já aceitos
- [x] Login, segunda senha, criação/lista, seleção, cidade e movimento.
- [x] Inventário abre com os itens iniciais.
- [x] Equipar/desequipar os 5 equipamentos; aparência e atributos mudam; relogin preserva.
- [x] Voltar à seleção de personagem (LogOut) e entrar novamente.
- [x] Equipar responde rápido (≈80–300 ms no log `elapsed_ms`).

## Pendentes
### Login
- [ ] Na tela da segunda senha **não** aparece "The Crimson Blade Security Code is not currently available".

### Mover/trocar itens (P0-02d)
- [ ] Arrastar uma pilha (ex.: poções) para um slot vazio da bolsa.
- [ ] Arrastar um item sobre outro de classe diferente: os dois trocam de lugar.
- [ ] Mover um equipamento vestido: continua vestido.
- [ ] Relogar: posições preservadas.
- [ ] (Opcional) Soltar fora da bolsa/aba inválida: aparece "Move item failed" e nada muda.

### Empilhar (P0-02e)
- [ ] Requer duas pilhas da mesma classe na bolsa (só possível após loot/compra). Soltar uma sobre a outra: soma até o limite; a de origem some se zerar.

### Barra de atalhos / quickslots (P0-02f)
- [ ] Arrastar poção de vida, pena de fênix ou kit de reparo para a barra de atalhos (slots 100–119): o atalho aparece.
- [ ] Arrastar dardos ou bombas para o slot de arma secundária (subweapon).
- [ ] Mover o mesmo consumível para outro atalho: sai do anterior.
- [ ] Tirar um atalho da barra.
- [ ] Relogar: atalhos preservados.
- [ ] Usar o consumível pela barra só será possível dentro de fase (combate ainda não implementado).

### Consumível usável na cidade
- [ ] (Próxima fatia) Cápsula de AP no inventário Cash: usar e conferir AP.

## Como relatar
Para cada falha: o que fez, o que apareceu na tela e o horário aproximado. Para lentidão: o horário, para comparar com `elapsed_ms`.
