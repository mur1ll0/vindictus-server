# Battle transport (M19): ProudNet relay

Status: implemented for a solo host (block B2.1); client verification pending. The codec is `src/protocol/proudnet.rs`, the relay `src/infrastructure/relay.rs`, and mission entry `src/infrastructure/minimum_mission.rs`.

## Why it is needed

Entering a mission goes through the frontend `AcceptQuestMessage` → `OpenNewParty` → `RegisterShipInfo` → `StartMicroPlay2`. `StartMicroPlay2` opens a P2P group in the `RelayServer` (a `Nettention.Proud` `NetServer`). `AddPlayer` registers the player in the relay with `RelayServer.AddClientWaiting(CID, UID)` and returns the relay endpoint, a serial number and a key to the client (`ConnectionRequestMessage`: Address, Port, PosixTime, Key, Category). When the client never connects to the relay, `AddPlayer` fails with `P2PJoinFail`. Only the cheat/DS paths skip the P2P group, so this also applies to a solo Fiona.

## Evidence sources

- Server: `ProudNet.dll` (x86 SHA-256 `f6cd106e71a84b719d727c1818844742c81a8ceefe1f018b5965e13021384b2f`) is a mixed C++/CLI assembly. The managed decompilation only shows wrappers (`NetServer`, `Rmi`), and the network core is native.
- Client: `Client/bin/engine.dll` statically links the complete ProudNet library, including the server-side implementation (`CNetServerImpl`, `C2SStub`, `S2CStub`). Native analysis uses Ghidra 12.1.4 (official NSA release, zip SHA-256 `ddac49f903da9d5bac833e5cc79395098b9c33cfd3279be5f31bd00387d2d4db`), installed outside the repository at `D:\PROJETOS\VindictusPrivate\Tools\ghidra`, with headless scripts under `Tools\ghidra-work`. The decompiled output stays local and is not committed.

## Recovered so far

### TCP framing

Every TCP message is `[u16 0x5713 little-endian][compact length][payload]`. The compact integer is one size byte (1, 2, 4 or 8) followed by the little-endian value of that width. A negative length or a length above the configured maximum closes the connection.

### Message types (first payload byte)

The numbering matches ProudNet's internal `MessageType` enum. The types handled are:

- Client: 1 (RMI), 2 (user message), 3, 4, 6, 8, 9, 10, 11, 13, 15, 17, 19, 23, 24, 25, 28, 30–39.
- Server: 1, 2, 5, 7, 12, 14, 16, 18, 20, 21, 22, 26, 27, 36–39.

### Connection handshake

| Step | Direction | Type | Content |
|------|-----------|------|---------|
| 1 | S→C | 4 | Connection hint: a bool flag, then the network settings. The settings are, in order: a byte enum, an int, an 8-byte value, a byte enum, an int, a bool, an int (the session key length in bits), four bools, one more field and a final bool. |
| 2 | C→S | 5 | Byte array: the client's RSA key-exchange public key (CryptoAPI blob; the client generates the pair with `CryptGenKey(AT_KEYEXCHANGE)`). |
| 3 | S→C | 6 | Byte array: the server-generated RC4 session key (`CALG_RC4`, length from the settings), encrypted with the client public key (CryptoAPI `SIMPLEBLOB`). |
| 4 | C→S | 7 | Byte array of user data (the relay key), the protocol-version GUID (16 bytes) and an int internal version. Both versions must equal the server's. |
| 5 | S→C | 10 | Connect success: int HostID, the server-instance GUID, a reply byte array and the client address as seen by the server. |

Notes:
- Type 8 is the denial.
- With a connection-request handler installed, as in `RelayServer`, the server answers type 7 with type 10. Without that handler it answers with a type-9 reply instead.
- The server only needs the public-key operation (RSA PKCS#1 v1.5 encryption of the session key). It never needs a private key.

### Field encodings

All little-endian.

| Field | Encoding |
|-------|----------|
| bool, byte | 1 byte |
| u16 | 2 bytes |
| int, HostID, event ID, frame number | 4 bytes |
| int64 | 8 bytes |
| GUID | 16 raw bytes |
| byte array | compact length, then the bytes |
| AddrPort | u32 IPv4, then u16 port |

The compact length uses the smallest width that holds the value as a signed number (1, 2, 4 or 8 bytes).

The "test splitter" (a 0xFE byte appended after every field) is disabled: its global default in the client is the constant 0.

### RMIs

An RMI is message type 1, followed by a u16 RMI ID and the arguments in declaration order.

ProudNet server→client RMIs (`ProudS2C`, IDs `0xfbf5`–`0xfc07`):

| ID | Name | Arguments |
|----|------|-----------|
| `0xfbf5` | P2PGroup_MemberJoin | group HostID, member HostID, custom field (byte array), event ID, P2P session key (byte array), first frame number, connection magic GUID, bool, u16 |
| `0xfbf6` | Unencrypted variant | Same arguments without the session key |
| `0xfbfc` | ReliablePong | none (the server's answer to ReliablePing) |

The client accepts members that are itself or the server (HostID 1).

ProudNet client→server RMIs (`ProudC2S`, IDs `0xfa01`–`0xfa13`), in PIDL order:

| ID | Name | Arguments |
|----|------|-----------|
| `0xfa01` | ReliablePing | float frame rate |
| `0xfa04` | P2PGroup_MemberJoin_Ack | group HostID, member HostID, event ID, bool |
| `0xfa07` | ShutdownTcpHandshake | — |
| `0xfa08` | Log | — |
| `0xfa09` | Log | — |

Heroes game RMIs (PIDL `S2C`/`C2S`):

| ID | Direction | Name | Arguments |
|----|-----------|------|-----------|
| `0x7d1` | S2C | NMInfos | — |
| `0x7d2` | S2C | S2CSendData | int host ID, byte array |
| `0x7d3` | S2C | BroadcastChat | — |
| `0x3e9` | C2S | NMInfosResult | int64 CID |

### Relay semantics (managed `RelayServer`)

- **Registration:** `AddClientWaiting` issues a serial number and a random key.
- **Connection request (type 7):** the user data must be exactly 8 bytes, `int SN` then `int Key`, matching a waiting client. The reply is the CID (int64) followed by the NexonSN (int32).
- **NMInfosResult:** the client sends its CID, which moves the relay client to Connected.
- **Joining the group:** `P2PGroup.Join` (`RelayServer/P2PGroup.cs`) calls `JoinP2PGroup(member, group, customField)`, where the custom field is ParentID (int64), ParentCategory (BinaryWriter string), ParentID again, the int 1 and the joining member's ClientHostID (int32), CID (int64) and NexonSN (int32). It then sends `SendNMInfos` with the members already in the group (the joining client is added afterwards). The member's `P2PGroup_MemberJoin_Ack` completes the join; this is the event that admits the player into the MicroPlay. The client's `P2PMemberJoinEventArgs` handler (`engine.dll` 0x42351740) reads this field without bounds: with only ParentID and ParentCategory it read past the buffer and crashed intermittently at +0x351932/+0x35193b (reports of 23, 24 and 25/10/2026).
- **Group custom field (`MakeCustomField`):** ParentID, ParentCategory, ParentID, then the int 0.

### Relayed frames (B5 MP2)

The relay offers no UDP, so every P2P frame between party members goes through it. Recovered from the server handlers of `engine.dll` (`CNetThreads_S::NetWorkerThread_ProcessMessage_*`: 0x42642d80, 0x426430b0, 0x42643420; destination list reader 0x425bbe60):

| Client → relay | Fields | Relay → each destination |
|---|---|---|
| 20 ReliableRelay1 | compact count, {HostID, u32 reliable frame number} × count, frame (byte array, must end the message) | 23 ReliableRelay2: sender HostID, that destination's frame number, frame |
| 21 UnreliableRelay1 | byte priority, compact unique ID, compact count, HostID × count, frame | 24 UnreliableRelay2: sender HostID, frame |
| 22 LingerDataFrame1 | destination HostID, u32 frame number, frame | 25 LingerDataFrame2: sender HostID, frame number, frame |

Only destinations that share a P2P group with the sender receive the frame; others are dropped. The destination count is bounded at 64 (the client's array bound was not recovered).

Group membership changes:
- **Join:** the joining member gets `P2PGroup_MemberJoin` for every member (itself included) and NMInfos of the earlier members; every earlier member gets `P2PGroup_MemberJoin` for the joining one. All carry the joining member's custom field and the same event ID.
- **Leave:** `P2PGroup_MemberLeave` (`0xfbfa`, ProudS2C slot 6: member HostID, group HostID; arguments read as two HostIDs by the client stub). Each remaining member hears that the leaver left; the leaver hears that every member, itself included, left. A closed relay connection leaves all its groups; an empty group closes; `close_group` (DestroyP2PGroup) notifies every member.

### Client completions above 2 GB (laboratory patch)

The client's IOCP completion getter (`engine.dll` 0x425fd2e0) tests the OVERLAPPED pointer as a signed int (`cmp eax, ebx; jge` at 0x425fd3a2). A negative pointer is treated as a custom event instead of being matched against the socket's receive and send OVERLAPPEDs. The laboratory `Vindictus.exe` is large-address-aware, so when the low 2 GB are used up a new `CTcpLayer_C` can land above `0x80000000`. Its first receive completion, the connection hint, is then dropped:
- the client never sends its public key;
- the worker's handshake timeout (10.0 s) closes the TCP connection;
- the Heroes connection wait (60 s, 0x42341570) and `NetClient::Disconnect` (0x425df460, at least 100 s, waiting for the receive still counted as pending) keep the client "Not Responding" before it returns to town.

The relay can't avoid this. `scripts/client/patch_engine_iocp_sign.py` turns that `jge` into `jmp` in the laboratory copy. Non-matching values still become custom events with the same fields.

## Mission entry wiring

The laboratory starts the relay on `relay_bind` from the assets file (the original `relay tcpPort` is 27005). For AcceptQuest the frontend:

1. Sends PartyInfo for JoinParty (InTown, party size 4).
2. Runs the QueryQuestDigest checks and sends PartyInfo plus UpdateShip for RegisterShipInfo (InTownWithShipInfo).
3. Opens the P2P group (StartMicroPlay2, EnteringShip, no broadcast).
4. Registers the host with AddClientWaiting, then sends PayCoinCompleted and InsertBlessStoneComplete.
5. Sends ConnectionRequest (relay address and port, PosixTime = SN, Key, category `MicroPlayServiceCore.MicroPlayService`) and LaunchShipGranted with the host's GameJoinMemberInfo.

The relay's Connected event triggers `join` (P2PGroup.Join with the party as parent); the member's acknowledgement completes it.

The host's later messages:
- RegisterServer → PartyInfo/UpdateShip (InShip).
- StartGame → Starting broadcast, GameStartGranted (QuestSectorInfos), then the InGame broadcast.
