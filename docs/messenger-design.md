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

## Serviço `messenger` no servidor (MS3b)

- Ouvinte TCP no processo do jogo (porta de `messenger_bind` nos assets, como `relay_bind`), conexões limitadas e quadros com limite (`protocol::nm::MAX_ENVELOPE`).
- Autenticação: `LoginAuth` (0x110f) traz o passport (wstr ≤ 0x1000). O frontend, ao aceitar um login, registra passport → (uid, NexonID) num mapa em memória do Runtime por 10 min; o messenger aceita o passport desse mapa. Sem passport conhecido: `bSuccess = 0`.
- `LoginMessenger` (0x110d): sucesso e evento 0x1001 MessengerReply(resultado 0) ao alvo 0; o engine então chama `GetMyInfo` e `GetFriendList`.
- Dados (migração própria): `game_messenger_category(uid, category_id, name)`, `game_messenger_friend(uid, friend_uid, category_id, state)` com estados pedido/aceito/bloqueado; amigos por conta (NexonSN), nome exibido = personagem selecionado ou o último.
- Funções: GetMyInfo 0x2001, GetFriendList 0x4001, RequestNewFriend 0x4003 (evento 0x1005 ao destinatário online), ConfirmNewFriend 0x4004 (aceite/recusa; 0x100e/0x1010 aos dois), BlockFriend 0x4005, categorias 0x4007–0x4009 e 0x4101–0x4105. Códigos de patcher, notas, arquivos, canais e log falham (FALSE).
- Presença: entrar/sair do jogo (registro no `party_hub`) dispara FriendInfoChanged 0x1010 aos amigos online pela conexão messenger deles; tudo por um ponto único (`MessengerHub`).
- A confirmar no cliente: type words dos objetos derivados (FriendInfo, RealUserInfo, CateFriendInfo), significado dos campos sem nome, sequência pós-login real (log da MS2a).

## Chat em grupo (MS4)

- Sessões de chat (0xa00b/0xa00c) e envio 0xc101: o servidor cria a sessão, convida (0xa001/0xa002 aos membros) e repassa CSChatMessageReceived 0xa101 com CNMChatMessage aos participantes.

## Distribuição (MS5)

- O launcher instala o `nmcogame.dll` do laboratório e o `nmcogame.ini` com o endereço público do servidor; `install-lab.ps1` cobre a cópia local.
