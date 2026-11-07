# Protocolo do Nexon Messenger no cliente 1.69 (MS1)

Notas da investigação dirigida MS1 (06/11/2026): tabelas PE, RTTI/vtables e funções específicas descompiladas sem auto-análise (Ghidra headless `-noanalysis`); nenhum binário executado. [C] confirmado, [I] inferido. Base do `nmcogame.dll` em Rust (MS2) e do módulo `messenger` (MS3).

# MS1 — Nexon Messenger glue (nmcogame.dll / engine.dll) — directed RE notes

Date: 2026-11-06. Read-only evidence; no game binary executed.
Binaries (sha256): `Client/nmcogame.dll` 124d016aa629bc0d…, `Client/nmconew.dll` 2da556e69abea6b9…,
`Client/bin/engine.dll` a3add9742f7fa03c… (image base 0x42000000; .text RVA 0x1000 = file 0x400; has a .vmp0
section, but all messenger code below is in plain .text).
Method: own PE parser (import/export tables, RTTI → COL → vtable, byte search for IAT/immediate refs),
Ghidra 12.1.4 headless **with `-noanalysis`** (no auto-analysis) + a probe script that disassembles /
decompiles only listed addresses. Scratch project and scripts live in the session scratchpad `ms1/` (not in repo).
Legend: **[C]** confirmed from disassembly/tables, **[I]** inferred.

## 1. Exports / imports

nmcogame.dll exports (PDB `e:\Work\PlatformDev\_branch\Taiwan\20150615_TWMH_MSG\MessengerNew\DLLWrapper\UnicodeRelease\nmcogame.pdb`) [C]:

| ord | name | RVA | convention / args (from engine call sites) |
|---|---|---|---|
| 1 | NMCO_CallNMFunction | 0x50c0 | cdecl, 5 args, returns BOOL (see §2) |
| 2 | NMCO_MemoryFree | 0x51b0 | cdecl `void (void* p)` |
| 3 | NMCO_SetLocaleAndRegion | 0x5ae0 | cdecl `BOOL (u32 locale, u32 region, u32* pErr, u32* pErrDetail)`; nmcogame writes *pErr=1000,*pErrDetail=0 on failure; engine writes 0xbb9 when the following SetLocale func fails |
| 4 | NMCO_SetPatchOption | 0x4fc0 | cdecl `BOOL (u32 opt)` → stores global, returns 1 |
| 5 | NMCO_SetVersionFileUrlA | 0x5220 | cdecl `BOOL (const char* url)` → returns 1 |
| 6 | NMCO_SetVersionFileUrlW | 0x5290 | not imported by engine |

- engine.dll imports from nmcogame.dll exactly ords 3,5,4,2,1 (IAT 0x426ee6e4..0x426ee6f4) [C].
- Direct call sites [C]: CallNMFunction only at 0x423ced51 (inside CNMCOClientObject::CallNMFunc 0x423cec60);
  MemoryFree 0x423ced8d (reply buffer), 0x423d81da (event buffer), 0x42374057 (thunk);
  SetLocaleAndRegion 0x423ceee5, 0x423cee39, 0x423740ab, 0x423740d4; SetPatchOption 0x423afbdc, 0x42374065;
  SetVersionFileUrlA 0x42374085. The `jmp [IAT]` thunks at 0x4264e7c0..d8 have no direct callers.
- nmcogame.dll has no static import of nmconew.dll: it LoadLibrary/GetProcAddress `NMCO_CallNMFunc2`
  (preferred), `NMCO_CallNMFunc` (legacy path, re-encodes), `NMCO_MemoryFree` and forwards [C]. Its WinInet/WS2
  imports belong to the NGM/version-file patcher [C imports, I purpose].
- nmconew.dll exports NMCO_CallNMFunc / NMCO_CallNMFunc2 / NMCO_MemoryFree and imports PostMessageA/W [C].
- client.dll imports nothing messenger-related (WININET, steam_api, icuuc56, KERNEL32, USER32, SHELL32, tier0,
  vstdlib, WS2_32) [C]. engine.dll exports only `?F@@YAXPAPAVIEngineAPI@@@Z` and `CreateInterface` [C].

## 2. NMCO_CallNMFunction

`BOOL __cdecl NMCO_CallNMFunction(u32 uFuncCode, u8* pCallBuf, u8 uCryptVer, u8** ppRetBuf, u32* puRetLen)`
[C layout; names I]
- Caller CNMCOClientObject::CallNMFunc(CNMFunc*) @0x423cec60, under a critical section (+8) [C]:
  stream(0x400) → pFunc->bCalling(+0x10)=1 → pFunc->Serialize (vtbl+8) → envelope finalize →
  `*ppRetBuf = clientObj->retBuf` (+4, preallocated), `*puRetLen = 0x100000` → call.
  If result == 1: decode *ppRetBuf, bCalling=0, DeSerialize (vtbl+0x14), result = pFunc->bSuccess (+0x28);
  if *ppRetBuf != preallocated buffer → `NMCO_MemoryFree(*ppRetBuf)`.
  Result != 1 → CallNMFunc returns FALSE (benign failure path) [C].
- pCallBuf carries no explicit length: it is inside the envelope (total = len + 12) [C].
  uCryptVer = the stream codec byte (1 = v1) [C].
- nmcogame forwards as `CallNMFunc2(code, pCallBuf, (u8)crypt, ppRet, puRetLen)` [C, 0x100050c0].

### 2.1 Envelope (stream with cryptor) [C: 0x423c9b80, 0x423cce10, 0x423c4cc0, cryptors 0x42362a80..0x42362e10]

```
u32 magic | u32 len | body[len] | u32 trailer
v0 CStreamCryptor_v0 (id 0): magic 0x001FCA34, trailer 0x008A119E, body = plaintext
v1 CStreamCryptor_v1 (id 1): magic 0x0A98C32A, trailer 0x820A9E12, body = obfuscated, len = plain + 10
```

- Both cryptors are registered; the decoder selects by magic (0x423b1e20) and checks the trailer.
  Engine always *sends* v1 (default codec). A replacement may **reply in v0** (plain; v0 decrypt is a no-op) [C].
- v1 encode (0x42362d00): `tmp[0]=rand, tmp[1]=1 (id), tmp[2]=rand, tmp[3..6]=u32 key (rand), tmp[7]=rand`;
  `tmp[8+i] = p[i] ^ (u8)key ^ tmp[8+i-1]` (prev = 0 for i = 0); `out = reverse(tmp[0 .. len+8]) ++ u16 checksum(out)`
  (checksum 0x42362c60: byte-pair add/xor fold, complemented). Decode (0x42362e10): verify checksum, reverse,
  check tmp[1] == 1, `p[i] = c[i] ^ (u8)tmp[3] ^ c[i-1]` [C]. Needed only to read engine requests.

### 2.2 Object encoding (CNMSerializable) [C: encoder 0x423639a0, decoder 0x42363a60]

```
u32 hdr = (typeWord << 16) | (b6 << 8) | version ; u32 bodyLen ; body
```

- Decoder checks only typeWord against the object's own word; the low byte (version) selects the layout:
  version 1 = flat (base-class fields inline); version 2 = base class serialized as a nested object with its
  own header. b6 is ignored on read (each class uses its own ctor constant for b6-dependent fields) [C].
- All 179 CNMFunc ctor sites pass version 1 [C].
- Type words [C]: CNMFunc 'fo' 0x666f, CNMEvent 'eo' 0x656f, CNMCategoryInfo 0x6369, CNMDefaultUserInfo 0x7569,
  CNMRequestNewFriendData 0x7266, CNMFriendList 0x466c, CNMVirtualUserInfo 0x7576, CNMVirtualFriendInfo 0x6676,
  CNMGuildOnlineInfo 0x676f, CNMSessionInfo 0x7369, CNMMyGameInfo 0x6d67, CNMGameCode 0x6763, CNMCharacter 0x6372.
  Derived data classes (CNMFriendInfo, CNMRealUserInfo, CNMCateFriendInfo, CNMChatMessage …) inherit a base
  word [I — verify, §6].
- Primitives (CNMSerializableDecoder vtbl 0x427389fc) [C]: u8, u16, u32, u64 little-endian raw;
  compact length = low 2 bits of first byte: 0 → u8>>2, 1 → u16>>2, 2 → u32>>2, 3 → tag byte + u32;
  wstring = compact length in UTF-16 units + UTF-16LE, no terminator (reader truncates to the field size).
  Engine converts its ANSI strings with the stream code page (it sets CP_UTF8 via SetCodePage) [C reader,
  writer 0x42363330 → 0x423632c0 I symmetric].

### 2.3 CNMFunc body [C: ctor 0x4236ba70, base writer 0x42382d90, reader 0x42382ea0]

CNMFunc ctor(code, version, callArg, x): +4 word 0x666f, +6 = 1, +7 = version, +0xc code, +0x10 bCalling,
+0x24 callArg, +0x28 bSuccess. Body (v1) = `u32 (bCalling ? callArg : bSuccess)` followed by the derived
fields; derived classes write their *arguments* only when bCalling and their *results* only when !bCalling.

=> Minimal valid reply for any code: `v0{ hdr 0x666f0101, len, u32 bSuccess, <result fields> }`.

Layouts read so far [C]:
- RegisterCallback 0x1002 — call: u32 0, u32 hWnd, u32 uMsg; reply: u32 bSuccess.
- LoginMessenger 0x110d — call: u32 0, u32 0xb319 (support flags); reply: u32 bSuccess.
- Initialize 0x1111 — call: u32, u32; reply: u32.
- LoginAuth 0x110f — call: u32, wstr(≤0x1000, passport), wstr(≤0x100, id), wstr(≤0x80, pwd), u32, u32,
  wstr(≤0x200), u32; reply: u32 bSuccess, u32 (+0x13c0), u32 (+0x13bc).
- GetMyInfo 0x2001 — reply: u32 bSuccess, CNMRealUserInfo object, u64, u64, u32, u32, wstr(≤0x1000).
- GetFriendList 0x4001 (0x423d7a30) — call: u32, u32 (+0x2c, +0x30); reply: u32 bSuccess, u32 nCat,
  nCat × CNMCateFriendInfo (full objects with header), u32 nFriend, nFriend × CNMFriendInfo (full objects).
- CNMCategoryInfo body: u32, u32, u32, wstr(≤0x20 name), [if b6 == 1: u8, u8].
- CNMCateFriendInfo = CategoryInfo body + u32 n + n × CNMFriendInfo + u8, u8.
- CNMFriendInfo = CNMRealFriendInfo base + u64, u32 ×4, wstr(0x40), wstr(0x40), u8, u32, u32 (as u16), u8, u8, u8,
  wstr(1000), wstr(0x20).
- CNMRealUserInfo = CNMDefaultUserInfo base + wstr(0x20), wstr(0x40), u32, u8, u32, u32, wstr(2), u32, wstr(2),
  u32 (u16), u8, u8, sub-object, u8 ×4, [if b6 == 2: u8, wstr(0x40)].
- CNMDefaultUserInfo body: wstr(0x40), u64, u32, wstr(0x40), u32, u32, u32, u16.
- RequestNewFriend 0x4003 — call: CNMRequestNewFriendData {u32, wstr 0x40, u32, u32, u32, u32, u32, wstr 0x40,
  wstr 0x40, u32, wstr 0x100, u8}. ConfirmNewFriend 0x4004 — call: u32, u8, u32.
- RequestChatSessionEx 0xa00b — call: u64, u32 ×5; reply: u32 bSuccess, u32, u32.
- CSSendChatMessage 0xc101 — call: CNMChatMessage {u32, u32, sub-object, wstr 0x40, wstr 0x100, wstr 0x100,
  wstr 0x400, u32}.

### 2.4 Data objects (MS3b, targeted decompilation; decoder 0x42363a60, handlers 0x42374100) [C unless I]

- A derived class keeps its base's type word; its ctor only raises b6. Optional blocks follow the class's own b6,
  not the wire's. Version 2 nests the base as an object; send version 1 (flat) everywhere. `char[N]` fields hold N-1.
- CNMDefaultUserInfo 0x7569/b6 1: wstr(0x40) login [I], u64 IDCode [I], u32 flags, wstr(0x40) nickname [I],
  u32 status (0, 13, 14 = offline), u32, u32, u16.
- CNMRealUserInfo 0x7569/b6 2 (GetMyInfo): default body, wstr(0x20), wstr(0x40), u32, u8, u32, u32, wstr(2), u32,
  wstr(2), u32, u8, u8, obj CNMAvatarFullInfo, u8 ×4, u8, wstr(0x40).
- CNMAvatarFullInfo 0x6169/b6 2: u8, u32, u8, u16, u16, 0x26 × (u32, u32) (zeros, ignored). CNMAvatarItemInfo
  0x6169/b6 1: u8, u32.
- CNMFriendInfo 0x7569/b6 3: default body, wstr (empty, discarded), obj AvatarItemInfo, NMFriendKey {u64 IDCode,
  u32 owner gameCode, u32 owner virtualId, u32 friend gameCode, u32 friend virtualId}, wstr(0x40) display name,
  wstr(0x40), u8 in-game flag (online = status online and flag 1), u32 (low 24 bits), u32, u8, u8, u8,
  wstr(1000) memo [I], wstr(0x20). **The friend list shows only names starting with "[" + GetGameServerName + "]"**
  (0x4235e900); GetGameServerName 0x8011 replies bSuccess + wstr(0x80) and its argument is the engine's game code.
- CNMCategoryInfo 0x6369/b6 1: u32 category, u32, u32, wstr(0x20) name, u8, u8. CNMCateFriendInfo 0x6369/b6 2:
  u32 category, u32, u32, wstr(0x20), u32 n, n × obj CNMFriendInfo, u8, u8.
- CNMRequestNewFriendData 0x7266/b6 1: u32 serial, wstr(0x40) peer (target on the call, requester on the event),
  u32, u32 gameCode, u32 virtualId, u32 gameCode, u32 virtualId, wstr(0x40), wstr(0x40), u32 category,
  wstr(0x100) message, u8. Event 0x1005 is dropped unless its game code is the engine's.
- Calls (derived fields after callArg): GetMyInfo reply = obj RealUserInfo, u64, u64, u32, u32, wstr(0x1000);
  ConfirmNewFriend = u32 serial [I], u8 confirm code [I], u32; BlockFriend = key, u8; AddFriendToCategory =
  key, u32 category, u8; DeleteFriendFromCategory = key, u32 category, u8; MoveFriendCategory = key, u32 from,
  u32 to, u8; AddCategory = u32 gameCode [I], u32, wstr(0x20) name, u32, u32 (no id returned); DeleteCategory =
  u32, u8; ChangeCategoryName = u32, wstr(0x20), u8. 0x4005/0x4007/0x4009 have no caller in engine.dll.
- Events: 0x1010 FriendInfoChanged = obj CNMFriendInfo, u8 hasAvatar, [obj AvatarFullInfo]; dropped unless the
  friend game code is the engine's and the owner virtualId equals the engine's (vt+0x8c, value unknown).
  0x100e has no case in the game's handler: friend updates go as 0x1010.
- Chat: CNMChatMessage 0x636d/b6 1 = u32 gameCode, u32, obj AvatarItemInfo, wstr(0x40), wstr(0x100), wstr(0x100),
  wstr(0x400) text [I], u32. CNMSessionInfo 0x7369 = u32, u32 gameCode, u32, u8, u32, u32, u64, u64, u32 ×4.
  CNMSessionMemberInfo 0x736d = u64 IDCode, wstr(0x40), wstr(0x40), u32, u32, wstr(0x40), u32, obj AvatarFullInfo.
  0xa00b call = key, u32; reply = bSuccess, u32 r1, u32 r2 (session key = r2 or r1). 0xa00c call = u32 ×3, u32 n,
  n × key; reply = bSuccess, u32 session key [I]. 0xa001 = u32, obj SessionInfo, u32 n, n × obj member, u8, u8;
  0xa101 = u32, obj ChatMessage, u8.
- Unknown: the engine's game code and virtualId values (vt+0x84 / vt+0x8c of the interface at 0x4286ad5c), the
  confirm code values and most unnamed fields (zeros are accepted).

### 2.5 Chat sessions (MS4, targeted decompilation; switch 0x42374100, session list 0x42e67a78) [C unless I]

- The engine only opens 1:1 sessions with a friend: 0xa00b RequestChatSessionEx (key, u32 0) → reply bSuccess,
  u32 r1, u32 r2 (key = r2 or r1). 0xa00c, 0xc006 and 0xc008 have no callers.
- The invited user gets 0xa001 GSSessionCreated on the game window (u32 key, SessionInfo, u32 n, n × member, u8, u8);
  the engine checks SessionInfo +0x10 = its game code and hands it to client.dll, which adds the session [I]. The
  session ctor (0x42359e70) calls 0xc003 GetSessionInfo(key) then 0xc001 GSRegisterCallback (callArg key, hWnd,
  0x8001). There is no accept step.
- 0xa002..0xa008 and 0xa101 are routed by the leading u32 key (unknown keys are dropped). 0xa002 Established =
  0xa001 with one trailing u8 (members to the listener); 0xa006/0xa007 = SessionMemberInfo; 0xa004 Closed removes
  the session before the lookup (the client never hears it).
- CNMSessionInfo wire: u32 key, u32 gameCode, u32 virtualId, u8, u32, u32, u64, u64, u32 ×4 (+0x40 = 3 switches the
  sender name to a virtual user). CNMSessionMemberInfo: u64 IDCode, wstr(0x40), wstr(0x40), u32, u32, wstr(0x40)
  title name, u32, obj AvatarFullInfo.
- 0xc101 CSSendChatMessage: callArg key, obj CNMChatMessage (sender "%s@%s" = my name + 0x800f GetGameShortName,
  text ≤ 0x3ff; game code left 0). Reply bSuccess. 0xa101 = u32 key, ChatMessage (receivers drop it unless its
  game code is theirs; "_SYSTEM_"/"_NOTICE_" senders show as notices), u8 (unused).
- 0xc002 WantClose: key, u8 → reply bSuccess + u8. 0xc003 reply: SessionInfo, u32 n, members. 0xc005 reply: u32 n,
  members. 0x800f GetGameShortName: call u32 0, u32 gameCode → bSuccess, wstr(0x80).
- Unknown: whether client.dll echoes the sender's own message, the trailing u8s, SessionInfo +0x18..+0x3c.

## 3. Event path (messenger → game) [C]

- NMCOGame::RegisterCallbackMessage (vtbl 0x42737d4c slot 2 = 0x42360040), called by engine init 0x421cb340
  with the game window: unless `-nonmco` on the command line, cvar `NGM_disable`, or flag 0x42bada8b, it sends
  Func 0x1002 {hWnd, 0x8001}.
- Chat rooms register (roomKey, hWnd, 0x8002) via CNMCRRegisterCallbackFunc 0xb001 (@0x423624f0);
  general/chat sessions register (sessionKey, hWnd, 0x8001) via CNMGSRegisterCallbackFunc 0xc001 (@0x42359ec7).
- nmconew posts `PostMessage(hWnd, 0x8001|0x8002, wParam = event code, lParam = heap envelope buffer)`
  [C engine side; nmconew imports PostMessage, its sender not disassembled].
- engine WndProc (switch near 0x421d7a6f): 0x8001 → 0x42358500, 0x8002 → 0x42358510 → 0x423d8050:
  event factory 0x423c0700(wParam) creates the event; 0x423cded0 decodes lParam (same envelope + 'eo' header);
  dispatch to the registered handler (+4 main handler, +0x10 chat-room handler); then **NMCO_MemoryFree(lParam)**.
  So event buffers must be allocated by the replacement DLL and released by its NMCO_MemoryFree.
- Side effects in 0x423d8050: 0x1001 MessengerReply with result 0|1, or 0x1003 Refresh with 1 → GetMyInfo
  (0x423cffd0) + refresh (0x42360da0); 0x1002 MsgConnectionClosed; 0x1012 GuildOnlineInfoEx (guild list update);
  0x1017 AuthConnectionClosed.
- CNMEvent base body is empty (ctor 0x42368db0: +4 'eo', +7 version, +0xc code); most events start with
  u32 (+0x24). MessengerReply / Refresh / MsgConnectionClosed = u32. FriendInfoChanged 0x1010 = CNMFriendInfo
  object, u8 flag, [object]. RealFriendInfoChanged 0x100e = u32, object, u8, [object]. RequestNewFriend 0x1005 =
  CNMRequestNewFriendData object. CSChatMessageReceived 0xa101 = u32, CNMChatMessage, u8. GSSessionCreated 0xa001 =
  u32, CNMSessionInfo, u32 n, n × member (0xf0 bytes in memory each), u8, u8.

Event codes [C, factory 0x423c0700]:
0x1001 MessengerReply, 0x1002 MsgConnectionClosed, 0x1003 Refresh, 0x1004 Special, 0x1005 RequestNewFriend,
0x1006 ServerMessage, 0x1007 CustomMessage, 0x1008 NoteInstantMsg, 0x1009 RefreshMessage, 0x100a FindUserReply,
0x100b InviteVirtualUser, 0x100c UserInfo, 0x100d GuildOnlineInfo, 0x100e RealFriendInfoChanged,
0x100f VirtualFriendInfoChanged, 0x1010 FriendInfoChanged, 0x1011 Notify, 0x1012 GuildOnlineInfoEx,
0x1013 GuildChatMessage, 0x1014 CustomNotify, 0x1016 NoteInfo, 0x1017 AuthConnectionClosed;
0x9001..0x9009 ChatRoom Created / Established / Error / Member / Info / MessageReceived / CreatedEx / EstablishedEx / InfoEx;
0xa001..0xa00a GS SessionCreated / Established / Failed / Closed / InfoChanged / NewMemberJoined / MemberLeaved /
MemberInfoChanged / SessionInfo / InviteRejected; 0xa101 CSChatMessageReceived; 0xa102 CSMultiChatCreated;
0xa201 FileUploadEvent; 0xa401..0xa403 whiteboard; 0xc001..0xc007 Channel Error / Created / Established / Info /
MemberInfo / MemberInfoList / Message; 0xd001 LogReportSync.

## 4. Engine-side glue [C names from RTTI/strings]

- The whole Nexon NMCO client SDK is statically linked in engine.dll (314 NM RTTI classes): CNMCOClientObject
  (vtbl 0x4273d034; CallNMFunc 0x423cec60; RegisterCallbackMessage 0x423cf1d0; singleton via 0x42358690),
  CNMManager, NMMainManager, NMChatSessionListManager, NMMainEventHandlerWrapper, NMChatRoomEventHandlerWrapper,
  CNMSerializable / CNMSerializableDecoder / CNMSimpleStreamDecoder, CStreamCryptor_v0/v1.
- NMCOGame (implements INMCOGame; vtbls 0x42737d4c and 0x42737d34) is what client.dll uses. Its slot 0x4235f070
  matches the name "NEXONplugV2.4" (string also in client.dll); "NEXONplug_MAIN_V2.4" (engine + client) and
  "NEXONplug_SESSION_V2.4" (engine) are the other interface names [C strings; I = obtained via CreateInterface].
- NMCOGame methods identified: RegisterCallbackMessage (0x42360040); AttachAuth (0x423600b0 → 0x110f path);
  LoginAuth (0x42360100 / 0x42360150 → 0x110f); Init (0x423615b0: SetLocaleAndRegion, Initialize 0x1111,
  SetCodePage 0x2109 CP_UTF8, GameLog init when feature "NexonGameLog"); LoginMessenger (0x423601f0, only if
  feature "Messenger"; logs "LoginMessenger Enabled/Disabled, server=%d messenger=%d" from cvars
  platform_server_code / server_messenger_enable); JoinGuildChatRoom (0x42362320 → JoinToChatRoomEx 0xa00a with
  ServerIP/Port/CharacterSN, feature "Guild"); guild chat send (0x423606b0 → CR SendChatMessage 0xb009).
- Login error UI keys (OnMessengerReplyEvent): GameUI_Heroes_Messenger_LoginError_{LoginFain, DeniedIP,
  WaitForTenMinites, PenaltyBlock, ExpulsionBlock, WrongIdentificationBlock, AlreadyLogin, ServiceCheck,
  WrongPassword, NoID} [C strings; value mapping not read].

## 5. Function codes (all 179 CNMFunc subclasses; code = ctor push, version 1 for all) [C]

0x1002 CNMRegisterCallbackFunc, 0x1003 CNMResetCallbackFunc, 0x1004 CNMAttachToNMCOServerFunc, 0x1005 CNMDetachFromNMCOServerFunc, 0x1006 CNMBringForwardStandAloneMsgFunc, 0x1007 CNMStartStandAloneMsgFunc, 0x1103 CNMLoginVirtualFunc, 0x1104 CNMLogoutVirtualFunc, 0x1105 CNMGetMyVirtualUserListFunc, 0x1106 CNMChangeMyPositionFunc, 0x1107 CNMSendRefreshMessageFunc, 0x1108 CNMChangeMyLevelFunc, 0x1109 CNMRemoveMyVirtualUserFunc, 0x110a CNMLoginPassportFunc, 0x110c CNMLoginNexonPassportFunc, 0x110d CNMLoginMessengerFunc, 0x110e CNMLogoutMessengerFunc, 0x110f CNMLoginAuthFunc, 0x1110 CNMLogoutAuthFunc, 0x1111 CNMInitializeFunc, 0x1112 CNMLoginAuthSecurityCNFunc, 0x1113 CNMGetGameTokenFunc, 0x1114 CNMFinalizeFunc, 0x1115 CNMPCBangLoginFunc, 0x1118 CNMSetAuthClosedCallbackFunc, 0x2001 CNMGetMyInfoFunc, 0x2002 CNMGetUserDataListFunc, 0x2003 CNMChangeMyInfoFunc, 0x2004 CNMGetConfigFunc, 0x2005 CNMChangeConfigFunc, 0x2006 CNMGetLocalConfigFunc, 0x2007 CNMChangeLocalConfigFunc, 0x2008 CNMGetCountFunc, 0x2009 CNMSetCountFunc, 0x200a CNMChangeNicknameFunc, 0x200b CNMChangeMetaDataFunc, 0x200c CNMRequestNewsFunc, 0x200d CNMCheckNewsFunc, 0x2101 CNMGetDomainFunc, 0x2102 CNMGetVersionFunc, 0x2103 CNMSetStatInfoFunc, 0x2104 CNMSetLocaleFunc, 0x2105 CNMGetNexonPassportFunc, 0x2106 CNMGetSecurityQuestionFunc, 0x2107 CNMGetNxHWIDFunc, 0x2108 CNMGetSecurityCodeCNFunc, 0x2109 CNMSetCodePageFunc, 0x4001 CNMGetFriendListFunc, 0x4002 CNMGetFriendInfoFunc, 0x4003 CNMRequestNewFriendFunc, 0x4004 CNMConfirmNewFriendFunc, 0x4005 CNMBlockFriendFunc, 0x4006 CNMChangeFriendMemoFunc, 0x4007 CNMAddFriendToCategoryFunc, 0x4008 CNMDeleteFriendFromCategoryFunc, 0x4009 CNMMoveFriendCategoryFunc, 0x400a CNMChangeFriendMemoExFunc, 0x4101 CNMAddCategoryFunc, 0x4102 CNMDeleteCategoryFunc, 0x4103 CNMChangeCategoryNameFunc, 0x4104 CNMChangeCategoryPropertyFunc, 0x4105 CNMChangeCategoryAllowTypeFunc, 0x5201 CNMGetRejectedUserListFunc, 0x5202 CNMAppendRejectedUserFunc, 0x5203 CNMRemoveRejectedUserFunc, 0x6001 CNMGetNoteBoxFunc, 0x6002 CNMSendNoteMsgFunc, 0x6003 CNMProcessNoteFunc, 0x6004 CNMSendNoteInfoFunc, 0x6005 CNMGetNoteBox2Func, 0x6006 CNMSendAccuseNoteMsgFunc, 0x7002 CNMMonitorGuildOnlineInfoFunc, 0x7003 CNMGetMyGuildListExFunc, 0x7004 CNMMonitorGuildOnlineInfoExFunc, 0x8001 CNMExecutePatcherFunc, 0x8002 CNMExecuteLauncherFunc, 0x8003 CNMExecuteCommonFunc, 0x8004 CNMGetUrlFunc, 0x8005 CNMGetPathFunc, 0x8006 CNMSetSessionValueFunc, 0x8007 CNMGetSessionValueFunc, 0x8008 CNMGetGameListFunc, 0x8009 CNMGetUrlExFunc, 0x800a CNMDownloadFileFunc, 0x800b CNMUploadFileFunc, 0x800c CNMGetSupportGameListFunc, 0x800d CNMGetGameServerListFunc, 0x800e CNMGetGameFullNameFunc, 0x800f CNMGetGameShortNameFunc, 0x8010 CNMGetGameFriendTitleFunc, 0x8011 CNMGetGameServerNameFunc, 0x8012 CNMGetConnConfigFunc, 0x8013 CNMSetConnConfigFunc, 0x8015 CNMExecuteNGMPatcherFunc, 0x8016 CNMExecuteNGMLauncherFunc, 0x8017 CNMIsNGMInstalledFunc, 0x8018 CNMRestrictedWordsFunc, 0x8019 CNMMinimizeAllFunc, 0x801a CNMLogReportSyncFunc, 0x801c CNMSetServerInfoFunc, 0x801d CNMDownloadGuildMarkFileFunc, 0x801e CNMGetServerInfoFunc, 0x801f CNMWriteToWiselogFunc, 0x8020 CNMExecuteNGMInstallerFunc, 0x8021 CNMOpenGameSummaryFunc, 0x9001 CNMGetUserInfoFunc, 0x9002 CNMFindUserFunc, 0x9003 CNMGetFindUserResultFunc, 0x9004 CNMSendNoteExMsgFunc, 0x9005 CNMSendReportMsgFunc, 0x9006 CNMCustomNotifyFunc, 0xa001 CNMRequestChatSessionFunc, 0xa002 CNMRequestMultiChatSessionFunc, 0xa003 CNMRequestFileUploadSessionFunc, 0xa004 CNMRequestWBSessionFunc, 0xa005 CNMReplySessionRequestFunc, 0xa006 CNMCreateChatRoomFunc, 0xa007 CNMJoinToChatRoomFunc, 0xa008 CNMRequestSessionToOtherFunc, 0xa009 CNMCreateChatRoomExFunc, 0xa00a CNMJoinToChatRoomExFunc, 0xa00b CNMRequestChatSessionExFunc, 0xa00c CNMRequestMultiChatSessionExFunc, 0xa00d CNMRequestFileUploadSessionExFunc, 0xa00e CNMRequestWBSessionExFunc, 0xa010 CNMCreateChannelFunc, 0xa011 CNMJoinChannelFunc, 0xb001 CNMCRRegisterCallbackFunc, 0xb002 CNMCRCloseFunc, 0xb003 CNMCRGetRoomInfoFunc, 0xb004 CNMCRChangeRoomInfoFunc, 0xb005 CNMCRChangeMyInfoFunc, 0xb006 CNMCRGetMemberListFunc, 0xb007 CNMCRInviteUserFunc, 0xb008 CNMCRBanUserFunc, 0xb009 CNMCRSendChatMessageFunc, 0xb00a CNMCRGetMemberInfoFunc, 0xb00b CNMCRGetRoomInfoExFunc, 0xc001 CNMGSRegisterCallbackFunc, 0xc002 CNMGSWantCloseFunc, 0xc003 CNMGSGetSessionInfoFunc, 0xc004 CNMGSSetServingProcessIDFunc, 0xc005 CNMGSGetMemberListFunc, 0xc006 CNMGSInviteUserFunc, 0xc007 CNMGSGetInviteCandidateListFunc, 0xc008 CNMGSInviteUserExFunc, 0xc101 CNMCSSendChatMessageFunc, 0xc201 CNMFUDSGetFileEventFunc, 0xc202 CNMFUSSendFileFunc, 0xc203 CNMFUSControlFunc, 0xc301 CNMFDSControlFunc, 0xc302 CNMFDSGetDownloadDirFunc, 0xc303 CNMFDSSetDownloadDirFunc, 0xc401 CNMWSSendWBMessageFunc, 0xc402 CNMWSGetAssocSerialKeyFunc, 0xd001 CNMCHCloseFunc, 0xd002 CNMCHGetChannelInfoFunc, 0xd003 CNMCHChangeChannelInfoFunc, 0xd004 CNMCHGetMemberInfoListFunc, 0xd005 CNMCHChangeMyInfoFunc, 0xd006 CNMCHInviteUserFunc, 0xd007 CNMCHBanUserFunc, 0xd008 CNMCHSendChannelMessageFunc, 0xf000 CNMCharacterLoginFunc, 0xf001 CNMCharacterLogoutFunc, 0xf002 CNMCharacterRemoveFunc, 0xf003 CNMCharacterChangeNameFunc, 0xf004 CNMCharacterLogin64Func, 0xf005 CNMCharacterLogout64Func, 0xf006 CNMCharacterRemove64Func, 0xf007 CNMCharacterChangeName64Func, 0xf008 CNMCharacterSyncFunc, 0xf009 CNMCharacterSync64Func, 0xf010 CNMGameLogInitializeFunc, 0xf011 CNMGameLogFinalizeFunc, 0xf012 CNMGameLogWriteStageLogFunc, 0xf013 CNMGameLogWriteErrorLogFunc, 0xf014 CNMGameLogGetSessionIDFunc.

Which codes the game actually reaches is only heuristically known [I]: a byte-level call graph (ctor → wrapper →
NMCOGame/NMMainManager caller) reaches the 0x10xx/0x11xx login and callback group, 0x2001, 0x2008, 0x2109,
0x40xx/0x41xx friends and categories, 0x52xx, 0x6002/0x6005, 0x7003/0x7004, several 0x80xx, 0x9001/0x9006,
0xa0xx, 0xb0xx, 0xc0xx-0xc3xx and 0xf0xx. Exact per-feature use needs step 6.2. A logging stub resolves it at runtime.

## 6. Recommended minimal Rust surface

DLL (32-bit, cdecl exports with the same names and ordinals 1..6):
- SetLocaleAndRegion / SetPatchOption / SetVersionFileUrlA / SetVersionFileUrlW → return TRUE (pErr = 0).
- MemoryFree → free only buffers this DLL allocated (large replies, event lParams); ignore others.
- CallNMFunction: decode the v1 (or v0) envelope + 'fo' header, dispatch on code, write the reply into *ppRetBuf
  when it fits *puRetLen (else allocate and replace the pointer) as a v0 envelope, set *puRetLen, return 1.
  Unknown code → return 0 (engine sees FALSE, no crash path observed).
- Event pump: keep (hWnd, msg) from 0x1002 / 0xb001 / 0xc001; PostMessage(hWnd, msg, eventCode, buf).

Must answer for the friends window and group chat:
0x1002/0x1003 callbacks, 0x1111 Initialize, 0x2104 SetLocale, 0x2109 SetCodePage, 0x110f/0x1110 LoginAuth/LogoutAuth
(AttachAuth path included), 0x110d/0x110e Login/LogoutMessenger plus async event 0x1001 (result 0), 0x2001 GetMyInfo,
0x4001 GetFriendList, 0x4003/0x4004 RequestNew/ConfirmNewFriend plus events 0x1005 and 0x1010/0x100e, 0x4005 BlockFriend,
0x4007-0x4009 category membership, 0x4101-0x4103 categories, 0xa00b/0xa00c RequestChat/MultiChatSessionEx,
0xc001 GSRegisterCallback, 0xc003/0xc005 session info/members, 0xc101 CSSendChatMessage plus events
0xa001/0xa002/0xa006/0xa007/0xa101.

Benign failure (return 0, or success with empty result): patcher/NGM/launcher 0x8001-0x8021 (re-check
GetGameServerName/ShortName if the UI needs a server label), notes 0x6xxx, file upload/whiteboard 0xc2xx-0xc4xx,
channels 0xa010/0xa011/0xd0xx, game log 0xf010-0xf014, PCBang/HWID/security 0x1112/0x1115/0x2105-0x2108,
virtual users 0x1103-0x1109, character sync 0xf000-0xf009 (probably success/no-op), guild chat room
0xa009/0xa00a/0xb0xx unless guild chat goes through the messenger.

Unknowns and next targeted steps (no whole-binary analysis needed):
1. Field semantics and exact type words of CNMRealFriendInfo / CNMFriendInfo / CNMCateFriendInfo / CNMRealUserInfo /
   CNMChatMessage / CNMSessionInfo: decompile their ctors (vtable xrefs from the RTTI map) and the NMMainManager code
   that copies them into client-visible structures.
2. MessengerReply result values and the post-login sequence: decompile 0x42360da0, OnMessengerReplyEvent
   (string ref at 0x42357c87) and the client.dll handler reached through "NEXONplug_MAIN_V2.4".
3. Wire writer for wstr (0x423632c0) to confirm the symmetric compact-length encoding.
4. Runtime check: a logging stub nmcogame.dll in the lab client copy (operator session) to record the real code
   sequence and envelopes; it replaces guesswork in §5.
