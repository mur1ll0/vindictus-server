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
Limites conhecidos: com a party formada, o líder pode aceitar a missão, mas o membro ainda não entra no navio (JoinShip é a etapa MP3); chat de party, transferência de líder pelo cliente e lista de navios abertos não migrados.
