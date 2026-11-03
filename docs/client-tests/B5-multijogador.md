# B5 — roteiro de cliente do multijogador

Roteiro das etapas MP do bloco B5 (`docs/multiplayer.md`). Primeiro teste com mais de um cliente real.

Preparação comum (provisória até o gate):
- Duas contas ativas com um personagem cada, por exemplo `lab169_a` e uma segunda conta criada pelo painel ou por `scripts/dev/create-account.ps1`. Os dois personagens precisam ter nomes diferentes.
- Dois clientes abertos ao mesmo tempo (duas janelas do `start-lab.bat` ou do launcher, uma por conta). Se o cliente recusar uma segunda instância na mesma máquina, relatar a mensagem antes de seguir.
- Os dois personagens na mesma cidade e no mesmo canal.

### MP1 — party na cidade: convite, aceite, recusa, saída e expulsão
Pré-condição: personagens A e B na cidade, nenhum em party.
- [ ] A convida B pelo nome (menu de party) → B recebe o convite com o nome de A; A vê a janela de party só com A.
- [ ] B aceita → os dois veem a party com A (líder, slot 0) e B (slot 1).
- [ ] B sai da party → B volta a ficar sem party; a party de A some (A ficou sozinho).
- [ ] A convida B de novo e B recusa → A recebe o aviso de recusa com o nome de B e a party de A some.
- [ ] A convida um nome inexistente → diálogo de personagem inexistente; A convida um personagem existente desconectado → aviso de que não está conectado.
- [ ] Com A e B em party, A expulsa B → B sai da party (aviso de expulsão); a party de A some.
- [ ] Com A e B em party, B tenta convidar alguém → mensagem de que só o líder convida.
- [ ] Com A e B em party, B fecha o cliente → a party de A some.
- [ ] Solo (sem party): aceitar e jogar uma missão como antes, inclusive desistir e voltar para a cidade.
Limites conhecidos: chat de party, transferência de líder pelo cliente e lista de navios abertos não migrados.

### MP2/MP3 — navio e dungeon com dois jogadores (checkpoint)
Pré-condição: A e B em party na cidade (MP1), A líder; uma missão que os dois podem fazer (requisitos atendidos pelos dois, sem item obrigatório para B).
- [ ] A aceita a missão no quadro/barco → A entra no navio; B vê o navio da party na interface de party.
- [ ] B embarca (JoinShip) → B carrega o navio; A vê B no navio; os dois na lista de membros.
- [ ] B marca pronto → A vê B pronto; antes disso o início pelo A é recusado.
- [ ] A inicia → os dois carregam a mesma dungeon e se veem (posição, movimento, ataques): é o teste do relay P2P.
- [ ] B fecha o cliente no navio → A continua no navio e B some da lista.
- [ ] A sai do navio antes de iniciar (com B dentro) → B volta para a cidade na party, agora como líder.
Limites conhecidos: dentro da dungeon só o host recebe EXP, ouro, drops e recompensa (MP4); saída de membro no meio da missão e reconexão são o MP5; desistir como membro encerra só a própria MicroPlay.
Relatar: entrada, o que cada cliente mostrou e o horário; anexar o `Report Error` se algum cliente travar.
