# Battle transport (M19): ProudNet relay

Status: investigation in progress (block B2). Nothing of this transport is implemented yet.

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
- **Joining the group:** `P2PGroup.Join` calls `JoinP2PGroup(member, group, customField)`, where the custom field is ParentID (int64) then ParentCategory (BinaryWriter string), and then sends NMInfos. The member's `P2PGroup_MemberJoin_Ack` completes the join; this is the event that admits the player into the MicroPlay.
- **Group custom field:** ParentCategory, ParentID, then the int 0.

Message types 20, 21 and 22 relay reliable, unreliable and linger frames to the other group members, answered as types 23, 24 and 25. A one-member group has nobody to relay to.

## Next steps

1. Recover the RMI header, the argument encodings, the P2P-group RMIs (member join and its acknowledgment), the keepalive and server-time messages, and whether encrypted messages (types 36 and 37) are required.
2. Implement a minimal relay that admits one client per mission group, and verify it with tests that replay the recovered byte layouts.
3. Wire it into B2.1: AcceptQuest, MicroPlay, AddPlayer, then ConnectionRequestMessage.
