# B5 — roteiro de cliente do multijogador

Roteiro das etapas MP1–MP7 do bloco B5 (`docs/multiplayer.md`). Primeiro teste com mais de um cliente real; o operador decidiu (03/11/2026) testar tudo numa sessão só, depois de implementado.

## Preparação

1. Subir o laboratório com o Docker já reconstruído (`docker compose up -d --wait`; a imagem do `game` precisa ser posterior aos commits do MP7).
2. Duas contas ativas com um personagem cada (A e B, nomes diferentes), por exemplo `lab169_a` e uma segunda conta criada pelo painel ou por `scripts/dev/create-account.ps1`. Para o teste de três jogadores (opcional), uma terceira conta (C).
3. Abrir dois clientes ao mesmo tempo (duas janelas do `start-lab.bat` ou do launcher, uma por conta). Se o cliente recusar uma segunda instância na mesma máquina, relatar a mensagem antes de seguir.
4. Os dois personagens na mesma cidade e no mesmo canal. Para MP4, escolher uma missão curta que os dois cumpram (sem item obrigatório para B); anotar EXP e ouro de cada um antes.

## Ordem da sessão

MP1 inteiro → MP7 chat → MP2/MP3 → MP4 (missão completa) → MP7 professor → MP5 (nova missão: desistência de B; depois host desconecta; depois reentrada) → MP6 (lista de navios com outra party). Relogar os dois no fim e conferir EXP, ouro e itens.

## Como relatar

Para cada entrada: o ID (por exemplo MP4-2), o que cada cliente fez e mostrou, e o horário (para cruzar com os logs em `.cache/verification` e `docker compose logs game`). Anexar o `Report Error` (`Documents\Vindictus EU GM\Report Error`) se algum cliente travar.

### MP1 — party na cidade: convite, aceite, recusa, saída e expulsão
Pré-condição: personagens A e B na cidade, nenhum em party.
- [ ] MP1-1: A convida B pelo nome (menu de party) → B recebe o convite com o nome de A; A vê a janela de party só com A.
- [ ] MP1-2: B aceita → os dois veem a party com A (líder, slot 0) e B (slot 1).
- [ ] MP1-3: B sai da party → B volta a ficar sem party; a party de A some (A ficou sozinho).
- [ ] MP1-4: A convida B de novo e B recusa → A recebe o aviso de recusa com o nome de B e a party de A some.
- [ ] MP1-5: A convida um nome inexistente → diálogo de personagem inexistente; A convida um personagem existente desconectado → aviso de que não está conectado.
- [ ] MP1-6: com A e B em party, A expulsa B → B sai da party (aviso de expulsão); a party de A some.
- [ ] MP1-7: com A e B em party, B tenta convidar alguém → mensagem de que só o líder convida.
- [ ] MP1-8: com A e B em party, B fecha o cliente → a party de A some.
- [ ] MP1-9: solo (sem party): aceitar e jogar uma missão como antes, inclusive desistir e voltar para a cidade.
Limites conhecidos: transferência de líder pelo cliente não migrada.

### MP2/MP3 — navio e dungeon com dois jogadores
Pré-condição: A e B em party na cidade, A líder.
- [ ] MP3-1: A aceita a missão no quadro/barco → A entra no navio; B vê o navio da party na interface de party.
- [ ] MP3-2: B embarca (JoinShip) → B carrega o navio; A vê B no navio; os dois na lista de membros.
- [ ] MP3-3: B marca pronto → A vê B pronto; antes disso o início pelo A é recusado.
- [ ] MP3-4: A inicia → os dois carregam a mesma dungeon e se veem (posição, movimento, ataques): é o teste do relay P2P.
- [ ] MP3-5: B fecha o cliente no navio → A continua no navio e B some da lista.
- [ ] MP3-6: A sai do navio antes de iniciar (com B dentro) → B volta para a cidade na party, agora como líder.

### MP4 — combate e loot por jogador
Pré-condição: A e B na mesma dungeon (MP3-4).
- [ ] MP4-1: matar monstros com A e com B → os dois ganham EXP (dividida pelo nível; em party a soma é maior que a solo).
- [ ] MP4-2: chutar os núcleos que caem para cada um → cada jogador recebe os próprios itens (aviso de item só no próprio cliente).
- [ ] MP4-3: quebrar barris/caixas → as ergs vão para um jogador sorteado; quem ganhou recebe o ouro/item.
- [ ] MP4-4: B morre e A o revive (pena de fênix ou reviver) → B volta à vida nos dois clientes.
- [ ] MP4-5: B usa poção da barra → o consumível de B é gasto, o de A não.
- [ ] MP4-6: completar a missão → os dois veem a tela de resultado com a própria recompensa (EXP, ouro, itens) e voltam à cidade.
- [ ] MP4-7: relogar os dois → EXP, ouro e itens de cada um preservados.
- [ ] MP4-8: quando B pega um núcleo ou cumpre uma meta pessoal, A também vê o aviso com o nome de B (e vice-versa).
Limites conhecidos: props quebrados por B não contam para as metas de props de B.

### MP5 — saída no meio da missão, migração de host e reentrada
Pré-condição: nova missão de A e B, já dentro da dungeon.
- [ ] MP5-1: B desiste (sair da party/missão) → B volta para a cidade com o resultado de desistência; A continua sozinho e B some da party.
- [ ] MP5-2: nova missão; A (host) fecha o cliente no meio da dungeon → B vê o aviso de reinício do host e passa a hospedar a dungeon (até 120 s), e continua jogando; B vira líder da party.
- [ ] MP5-3: se B não conseguir reiniciar em 120 s → B volta para a cidade sem recompensa (relatar se acontecer).
- [ ] MP5-4: nova missão; B (membro) fecha o cliente no meio da dungeon → A continua. B abre o cliente de novo: na lista de personagens o personagem aparece com o estado de reentrada; ao entrar, aceitar voltar à batalha → B reentra na party e embarca de novo na mesma dungeon (cronômetro da missão recebido), e termina a missão com recompensa.
- [ ] MP5-5: repetir MP5-4 recusando a reentrada → B fica na cidade sem party e o estado some da lista.
- [ ] MP5-6: missão de A sozinho (ou todos saem); A fecha o cliente no meio da dungeon e volta dentro do limite → A reentra, recebe o aviso de reinício e volta a hospedar a mesma dungeon como líder.
- [ ] MP5-7: A, B e C na dungeon; A desiste → A vê "esperando os outros"; B e C recebem a pergunta "sair com o host?" (20 s). B aceita e C recusa → A e B recebem a tela de desistência (ou de sucesso se a meta principal já estava cumprida) e voltam à cidade juntos numa party nova; C continua e vira host. Com dois jogadores, sem resposta em 20 s, B segue o host.
- [ ] MP5-8: durante a pergunta, B tenta sair da missão → aviso de que não pode sair enquanto o host sai.
- [ ] MP5-9: missão concluída; A volta para a cidade → A e B veem a contagem de 10 s e B volta junto.
Limites conhecidos: quem desconecta e não reentra fica sem a recompensa, como no original (o servidor original não manda recompensa por correio).

### MP6 — lista de navios abertos
Pré-condição: três personagens (A, B, C) ou dois, com A em navio de uma missão que C (ou B, fora da party) pode fazer, na mesma dificuldade e faixa de nível.
- [ ] MP6-1: A aceita a missão e fica no navio (InShip) sem iniciar → C abre o barco/quadro e pede a lista de batalhas do mesmo conjunto → o navio de A aparece com nome, membros e vagas.
- [ ] MP6-2: C entra no navio de A pela lista → C entra na party de A e embarca; A vê C.
- [ ] MP6-3: com o navio cheio, já iniciado ou de outra dificuldade → o navio não aparece.
- [ ] MP6-4: A abre party com navio na cidade (criar party com missão, sem partir) → C vê esse navio na lista do barco, entra pela lista e fica na party de A na cidade; quando A aceita a missão, C embarca.
Limites conhecidos: a ordem da lista não gira como no original.

### MP7 — chat de party e pedido de professor
Pré-condição: A e B em party.
- [ ] MP7-1: A escreve no chat de party → A e B veem a mensagem com o nome de A; B responde e os dois veem.
- [ ] MP7-2: dentro de uma dungeon, a party continua conversando pelo chat de party.
- [ ] MP7-3: (se houver o botão de pedir ajuda/professor) o host solo de nível 1 pede professor → resposta de que ninguém aceitou; com nível maior que 1 → aviso de nível; em party → aviso de que não pode em multijogador.
Limites conhecidos: assistência por professor nunca encontra candidato no servidor recuperado (TeacherFinder sem candidatos), então ninguém entra como assistente.
