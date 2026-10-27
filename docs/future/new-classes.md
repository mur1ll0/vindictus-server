# Classes novas (Delia, Miri) e armas novas: investigação do cliente NA

Investigação de 27/10/2026, a pedido do operador, antes do B4.8. A pergunta tem três partes:
- dá para trazer a Delia e depois a Miri para o cliente e o servidor do projeto?
- dá para trazer as armas novas do Hurk e da Lynn?
- dá para ler o `heroes.db3` do cliente oficial?

Tudo foi lido sem executar nada: cabeçalhos PE, varredura de bytes e strings, e consultas SQLite somente leitura. Nenhuma análise completa de binário foi feita (limites da máquina). Os scripts ficaram no scratchpad da sessão. Os números estão nesta página.

## 1. O que foi comparado

| | Cliente do projeto (1.69 EN) | Cliente NA oficial (`F:\Games\vindictus\appdata\en-US`) |
|---|---|---|
| `bin/client.dll` | 14,4 MB, compilado em 2016 (carimbo 1456902639) | 24,7 MB, compilado em 2026 (carimbo 1791770480), também com `bin_x64` |
| Proteção | seção `.vmp0` (VMProtect parcial); o resto do `.text` é legível | `.vmp0` no `client.dll`, no `engine.dll` e no `FileSystem_Stdio.dll` |
| Pacotes | 2.681 `.hfs` (`<SHA1>.hfs`): zip com cabeçalhos `HF 01 02` em claro e nomes e dados em XOR com a tabela de 4.096 bytes (`scripts/client/hfs.py`) | 9.220 `.hfs` (`<SHA1>_%05d.hfs`, 42,3 GB): arquivo inteiro cifrado, sem cabeçalho em claro, todos com tamanho múltiplo de 16 |
| Cifra no `FileSystem_Stdio.dll` | só zlib | Crypto++ com AES/Rijndael (S-box e `Rijndael_Info`), política de cifra aditiva (modo de fluxo), ChaCha, SHA-256 e o formato `%08X%08X%08X%08X%08X_%05d` ("PknSync Package") |
| Nomes de classe no `client.dll` | as 10 de sempre; **"delia" e "miri": 0 ocorrências** | **32 classes:** Lethita, Fiona, Evy, Kalok, Kay, Vella, Hurk, Lynn, Arisha, Hagie, Delia, Miri, Grimden, Miul, Bel, Lethor, Kael, Tessa, Danah, Letty, Latiya, Czern, Achel, Sou, Sanyaa, Neamhain, Calia e Lumielle (tabela em `0x1285838`), com `expression_table_<classe>` e `npc_<classe>` |
| Armas (entidades `CWeapon…`/`C_Weapon…`/`DT_Weapon…`) | 9: phantomdagger, longblade, battleglaive, greatsword, pillar, staff, bow, longsword e dualsword | mais de 20: as anteriores e dualspear, hammer, scythe, blaster, crossgun, dualblade, **bastardsword**, teide, blute, whip, draker, katamadhar, manarevolver, battleaxe, crest, swordspear, rapier, soulblade, heavycannon, lance, orbis… |

A tabela de pacotes do laboratório não decodifica os pacotes do NA: `pack_files` não acha o registro final em nenhum dos quatro testados. O HFSExtract que já está em `D:\PROJETOS\VindictusPrivate\Tools\HFSExtrcat.CLI-Win10-x64` foi lido pelos metadados .NET, sem executar. Ele só implementa o formato antigo: `KeyTable` de 4.096 bytes, `XorStringWithKey` e `XorBlockWithKey`.

## 2. O que já existe no conteúdo do 1.69

Consultas no `heroesContents.db3` do servidor (`ServidorVindictus/Server/zh-tw-x-gm/Bin`, igual ao de `TutorialRagezone/DB/server`) e no `heroes.db3` do laboratório:

- **Delia (classe 10):**
  - O conteúdo tem `CharacterClassInfo` da classe 10 com 200 níveis, 196 itens com "delia" no nome, 284 itens exclusivos dela (`ClassRestriction = 1024`), 19 skills (`SkillInfo.ClassRestriction = 1024`) e 62 armas `BASTARDSWORD`.
  - Falta `DefaultItemInfo` da classe 10, ou seja, não há equipamento inicial.
  - O cliente 1.69 tem a pasta `models/player/delia`.
  - O conteúdo foi entregue antes do código: no 1.69, nem o `client.dll` nem o `server.dll` do cliente têm a classe.
- **Miri:** nada, nem item, nem bit 2048, nem skill.
- **Armas por classe no 1.69:** Lann `DUALSPEAR`, Fiona `LONGSWORD`/`HAMMER`, Evie `STAFF`/`SCYTHE`, Karok `PILLAR`/`BLASTER`, Kai `BOW`/`CROSSGUN`, Vella `DUALBLADE`, Hurk só `GREATSWORD`, Lynn só `BATTLEGLAIVE`, Arisha `LONGBLADE`, Hagie `PHANTOMDAGGER` e Delia `BASTARDSWORD`.
  - A segunda arma do Hurk e da Lynn não existe neste conteúdo.
  - O nome delas no NA ainda não foi confirmado: está no banco do NA, que está cifrado.

## 3. O que permite mais de 10 classes

| Camada | Onde está o limite | Pode mudar? |
|---|---|---|
| Servidor Rust | guardas `class BETWEEN 0 AND 9` (4 arquivos), `< 10` em `npc_talk.rs` e perfis por classe (`class-initial.json`, initializers) | sim, é código e dado nossos (custo médio) |
| Servidor original (referência) | `BaseCharacter` com `CHARACTER_COUNT = 10` e os `switch` por classe (`extension-feasibility.md`, seção 7.1) | serve de molde; o Rust não depende dele |
| Conteúdo | `ClassRestriction` (o bit 1024 já é da Delia), `CharacterClassInfo`, `DefaultItemInfo`, `SkillInfo`, `VocationSkillInfo` e títulos | sim, são dados |
| Cliente 1.69: tabela de classes | `client.dll` `0x38cafd50`, limite `cmp eax, 0xa` em `0x3879ec68` | trocar a tabela e o limite não basta (linha abaixo) |
| Cliente 1.69: código da classe | combate, armas como entidades C++ (`CWeapon<Tipo>` com tabela de rede `DT_Weapon<Tipo>`), animação, expressões, criação de personagem e interface: tudo em `client.dll` e `server.dll` do cliente, sem nada da Delia | **não**: seria escrever em x86, sem fonte, o código de uma classe e de uma arma inteiras |

**Recompilar o cliente:** as ferramentas públicas da Source não resolvem.
- O Source SDK 2013 e as ferramentas de modelo e mapa (`studiomdl`, Crowbar, VTFEdit, Hammer/`vbsp`) tratam **assets**: modelos, texturas e mapas.
- O `client.dll` do Vindictus é código de jogo proprietário da Nexon sobre uma engine Source modificada.
- O código-fonte desse jogo não é público. Ninguém consegue recompilá-lo; dá para refazer assets, não o código das classes.
- Para modelos, o projeto já tem o caminho próprio: o kit e o add-on do Blender, provados no cliente.

## 4. Caminhos possíveis

### A. Ficar no cliente 1.69 (base atual)

- A Delia, a Miri e as armas novas **não** entram: falta o código delas no binário.
- O que entra são variantes de classes existentes (`extension-feasibility.md`, seção 7.5) e itens ou modelos novos para as 10 classes.
- Custo do que já está planejado: nenhum custo novo.

### B. Trocar a base para o cliente NA

O NA já tem tudo: as 32 classes, as armas e o conteúdo. O custo equivale a refazer boa parte do projeto:
1. **Pacotes:** decifrar o HFS novo (AES do Crypto++, com a chave dentro do `FileSystem_Stdio.dll` protegido) e regravar o formato, para itens próprios e para o launcher.
2. **Protocolo:** o servidor Rust implementa as mensagens do 1.69, recuperadas da fonte C# do servidor daquela época. Do servidor atual não há fonte.
   - **Hipótese:** dez anos de mudanças alteraram mensagens, login (Nexon/Steam) e o fluxo de criação, combate e missão.
   - Cada mensagem teria de ser recuperada do binário protegido do cliente.
   - Os blocos B0–B3 seriam refeitos para o protocolo novo.
3. **Conteúdo:** o `heroes.db3` do NA (no pacote cifrado) é só a metade do cliente. O banco de conteúdo do servidor atual não existe fora da Nexon.
   - Para cada sistema novo, o servidor teria de ser reconstruído a partir do banco do cliente.
4. **Cliente oficial em serviço:** o NA traz anti-cheat (BlackCipher, `NGClient`) e é atualizado pela Nexon. A cópia de laboratório teria de ser congelada numa versão.

Custo: um projeto novo, de vários blocos, com o maior risco técnico de todos os caminhos.

### C. Leitura do NA como referência, sem trocar a base

- Decifrar o HFS do NA só para **ler** o `heroes.db3`, os scripts e os modelos: nomes das armas do Hurk e da Lynn, skills e stats da Delia e da Miri, modelos.
- Serve para planejar, ou para o caminho B. Não faz a Delia funcionar no 1.69.
- **Ferramentas:**
  - O HFSExtract de `Tools` é antigo (sem AES).
  - O projeto público [HFSExtract](https://github.com/yretenai/HFSExtract/releases) lista suporte a "HFS v2" e "HFS 2 Hotfix 1" e credita o algoritmo de cifra.
  - Um post do RaGEZONE descreve um "HFS Extractor v4" testado nos clientes TW e US ([tags vindictus](https://forum.ragezone.com/tags/vindictus/)).
  - Um tópico do ResHax relata que as ferramentas atuais falham, porque os arquivos teriam sido recifrados ([ResHax](https://reshax.com/topic/18035-pc-vindictus-hfs-unpacking/)).
  - Nada foi baixado. O próximo passo seria baixar o código-fonte (não binários) do HFSExtract, revisá-lo e só então tentar ler um pacote do NA, somente leitura.

## 5. Recomendação

1. Fechar o B4 no 1.69 (B4.8 e gate). A troca de base é decisão do operador e não muda o B4.
2. Se o operador quiser medir o caminho B, começar pelo C: baixar e revisar o código do HFSExtract, decifrar só o `heroes.db3` do NA e comparar classes, armas e tabelas com o 1.69.
   - Com o banco em mãos, a próxima medida seria o protocolo: comparar os nomes das mensagens do cliente NA com as do 1.69 para estimar quanto do B0–B3 sobrevive.
3. Não copiar nada do NA para a cópia de laboratório sem decisão explícita (`docs/client-compatibility.md`).

## 6. Lacunas

- A chave e o modo exato da cifra do HFS do NA.
- O tamanho da diferença de protocolo entre o 1.69 e o NA (só hipótese).
- O que há na pasta `models/player/delia` do 1.69 (não aberta).
- Os nomes das segundas armas do Hurk e da Lynn, que dependem do banco do NA.
