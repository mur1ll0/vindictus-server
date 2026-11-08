# Messenger próprio: desenho (B5 MS2–MS5, decisão D-S1)

Base: `docs/messenger-protocol.md` (MS1) e `protocol::nm` (MS3a). Sem sessão de cliente até o launcher distribuir o cliente (operador, 07/11): implementar com os layouts conhecidos e marcar o que só o cliente confirma.

## Túnel `nmcogame.dll` ↔ servidor (protocolo nosso)

- O `nmcogame.dll` do laboratório (`tools/nmcogame`) abre uma conexão TCP com o serviço `messenger` do servidor no primeiro `NMCO_CallNMFunction` que precisa dele. Endereço: `nmcogame.ini` ao lado da DLL (`[messenger] address=host:porta`), padrão `127.0.0.1:47611`.
- Quadro: `u32 LE comprimento` + `u8 tipo` + carga.
  - 1 Call (DLL → servidor): `u32 seq`, `u32 code`, envelope da chamada (v1 do engine, repassado como veio).
  - 2 Reply (servidor → DLL): `u32 seq`, `u8 ok`, envelope de resposta (v0). `ok = 0`: `CallNMFunction` devolve FALSE.
  - 3 Event (servidor → DLL): `u32 alvo` (0 = janela principal 0x8001; senão a chave de sessão/sala registrada por 0xc001/0xb001), `u32 código do evento`, envelope do evento (v0 'eo').
- Local na DLL: os registros de callback (0x1002/0x1003, 0xc001, 0xb001) guardam (chave → hWnd, mensagem) e também seguem ao servidor; Initialize/SetLocale/SetCodePage respondem localmente se o túnel não conectar.
- `CallNMFunction` é síncrona: envia Call e espera o Reply com o mesmo seq (5 s; sem resposta = FALSE). Uma thread lê o socket: Reply vai ao chamador; Event é copiado para um buffer alocado pela DLL e postado com `PostMessage(hWnd, msg, código, buffer)`; o engine o devolve em `NMCO_MemoryFree`, que libera.
- Só o módulo `ffi` da DLL tem `unsafe` (ponteiros do engine, PostMessage, alocação dos eventos).
- Implementado (07/11, `tools/nmcogame/src/tunnel.rs`): conexão sob demanda (2 s), Reply em até 5 s, thread leitora, alvos de 0x1002 (alvo 0) e 0xc001/0xb001 (chave = campo antes do hWnd), evento 0x1002 MsgConnectionClosed ao cair; respostas maiores que o buffer do engine e eventos usam buffers da DLL liberados em `NMCO_MemoryFree`.

## Serviço `messenger` no servidor (MS3b)

- Ouvinte TCP no processo do jogo (porta de `messenger_bind` nos assets, como `relay_bind`), conexões limitadas e quadros com limite (`protocol::nm::MAX_ENVELOPE`).
- Autenticação: `LoginAuth` (0x110f) traz o passport (wstr ≤ 0x1000). O frontend, ao aceitar um login, registra passport → (uid, NexonID) num mapa em memória do Runtime por 10 min; o messenger aceita o passport desse mapa. Sem passport conhecido: `bSuccess = 0`.
- `LoginMessenger` (0x110d): sucesso e evento 0x1001 MessengerReply(resultado 0) ao alvo 0; o engine então chama `GetMyInfo` e `GetFriendList`.
- Jogo aberto pelo launcher (08/11, relato de dois clientes): com `NGM_disable` o engine nunca chama `LoginAuth`/`LoginMessenger` (logs: só 0x1104/0x110e/0x1110, 0x2001 e 0x4003 recusados). A DLL manda, ao conectar, o quadro 4 `Ticket` com o `-launcher_ticket` da linha de comando; o servidor guarda o ticket e, quando o frontend aceita o login com ele (`Passports::remember` acorda a conexão), a sessão entra como LoginAuth + LoginMessenger e recebe o 0x1001. Nessa sessão o `LogoutMessenger` do engine não a tira do ar (fica online até o túnel fechar).
- Dados (migração própria): `game_messenger_category(uid, category_id, name)`, `game_messenger_friend(uid, friend_uid, category_id, state)` com estados pedido/aceito/bloqueado; amigos por conta (NexonSN), nome exibido = personagem selecionado ou o último.
- Funções: GetMyInfo 0x2001, GetFriendList 0x4001, RequestNewFriend 0x4003 (evento 0x1005 ao destinatário online), ConfirmNewFriend 0x4004 (aceite/recusa; 0x100e/0x1010 aos dois), BlockFriend 0x4005, categorias 0x4007–0x4009 e 0x4101–0x4105. Códigos de patcher, notas, arquivos, canais e log falham (FALSE).
- Presença: entrar/sair do jogo (registro no `party_hub`) dispara FriendInfoChanged 0x1010 aos amigos online pela conexão messenger deles; tudo por um ponto único (`MessengerHub`).
- Implementado (07/11): `infrastructure::messenger` (ouvinte em `127.0.0.1:47611`, `Service`, `MessengerHub`), `messenger_friends` (migração 0069: `game_messenger_categories`, `game_messenger_friends` com estados pedido/aguardando/amigo/bloqueado por direção) e `protocol::nm::objects` (layouts do §2.4 do protocolo).
- Nome na lista: `[Vindictus]` + personagem (último logado, senão o primeiro vivo; sem personagem, o login), porque o engine só mostra nomes com o prefixo de `GetGameServerName` (0x8011, respondida pelo servidor com "Vindictus"). O pedido de amizade aceita nome de personagem ou login, com ou sem o prefixo.
- Código de jogo: o argumento da 0x8011 fica no contexto da conexão e vai nas chaves e eventos de amigo (o engine descarta evento com código diferente); o virtualId vai 0.
- Lista (0x4001): categoria 0 "Amigos" e as criadas, cada uma com seus amigos, depois todos os amigos; pedidos aguardando resposta seguem como eventos 0x1005 (serial = NexonSN de quem pediu, que o ConfirmNewFriend devolve; código 1 aceita, outro recusa).
- Remover amigo: DeleteFriendFromCategory (0x4008) desfaz a amizade (um amigo tem uma categoria só); o outro lado recebe 0x1010 offline.
- A confirmar no cliente: valores reais do código de jogo e do virtualId (vt+0x84/vt+0x8c), código de aceite do ConfirmNewFriend, se a lista usa as categorias ou a lista solta (as duas vão preenchidas), sequência pós-login real.

## Chat em grupo (MS4)

- Implementado (07/11): o engine só abre conversa 1:1 com um amigo (0xa00b; `docs/messenger-protocol.md` §2.5). O servidor (`messenger::Chats`) cria a sessão (amigo aceito e online; até 32 por conta, 4096 no total), responde a chave e manda 0xa001 à janela do jogo do amigo; cada lado registra a janela da sessão (0xc001, que a DLL responde localmente e também repassa ao servidor) e, com os dois registrados, recebe 0xa002. 0xc003/0xc005 devolvem a sessão e os membros; 0xc101 vira 0xa101 aos outros membros com o código de jogo de cada um e o remetente definido pelo servidor (`personagem@Vindictus`, sem confiar no cliente); 0xc002 e a última conexão caindo tiram o membro (0xa007 aos outros).
- A confirmar no cliente: se a própria mensagem aparece para quem envia (o servidor não a devolve) e se o cliente adiciona a sessão ao receber 0xa001.

## Distribuição (MS5)

- Implementado (07/11): a cópia do cliente (`.cache/play-client`) leva o `nmcogame.dll` do túnel (`install-lab.ps1`), e o launcher grava `nmcogame.ini` (`[messenger]`, `address=<host do servidor do launcher>:47611`) na pasta do jogo ao clicar em Jogar (`tools/launcher/src/game.rs`). Para outro computador, a porta 47611 precisa ser publicada fora do loopback junto com as demais portas do jogo (decisão de distribuição do operador; o compose publica todas em 127.0.0.1).
