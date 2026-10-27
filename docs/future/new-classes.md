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

## 7. Caminho C, etapa C1: leitura do NA (27/10/2026)

> **Atualização (mesmo dia):** o extrator `NPCodex/hfs-extract` v0.3.0, indicado pelo operador, abre os pacotes do NA (seção 7.6). As seções 7.1 a 7.5 registram a primeira tentativa. A hipótese de AES em ECB da seção 7.2 estava errada: o formato v4 usa ChaCha20 só no primeiro KB de cada arquivo, e os blocos repetidos vêm da parte que fica em claro.

Pedido do operador: decifrar os pacotes do NA só para leitura, com ferramentas públicas revisadas. Nenhum arquivo do NA foi extraído, copiado ou executado. Os downloads, o programa de teste e os logs ficaram em `.cache`, que é ignorada pelo Git.

### 7.1 Ferramentas testadas

| Ferramenta | Origem | O que é | Resultado no NA |
|---|---|---|---|
| HFSExtract 0.2.1 ("HFS 2 Hotfix 1", 07/04/2022) | `github.com/yretenai/HFSExtract` (hoje `neptuwunium/HFSExtract`, arquivado), branch `HFS-v2`, clonado em `.cache/hfsextract-src` | C#, cerca de 600 linhas, só cálculo (sem rede, processos ou registro). A classe `HFSSerpent` é, na verdade, uma cifra de fluxo da família SNOW/Sosemanuk. A chave do cabeçalho, da tabela e de cada arquivo vem do nome `<SHA1>_%05d.hfs` e da constante `MBHEROES!@0u9`. O CLI original não valida o cabeçalho decifrado e grava caminhos do pacote sem sanitizar, por isso não foi executado. | Cabeçalho inválido nos 11 pacotes testados (`checksum ≠ count + version`, contagens negativas ou na casa dos bilhões) |
| HFSTest (último push em 15/05/2023) | `github.com/DStarxing/HFSTest`, baixado com autorização do operador em `.cache/hfstest-src` (7 arquivos de código) | Mesmo código do HFS v2, com uma tentativa de descobrir a constante do formato "novo" de 2023 por força bruta (`MBHEROES00000`) | Sem efeito: a chave do cabeçalho só usa os 16 primeiros bytes da chave, que vêm do nome do arquivo, então a constante não muda o cabeçalho; resultado idêntico |

O programa de teste próprio (`.cache/hfs-probe`, .NET 8, só leitura) compila apenas as classes de cifra do HFSExtract. Além da posição esperada, ele procurou um cabeçalho válido em cada uma das 4.096 primeiras posições de 6 pacotes: nenhum acerto.

### 7.2 O que o formato atual mostra (medição pontual, sem análise completa)

- **Pacotes:** tamanhos múltiplos de 1.024; entropia por bloco de 4 KB entre 7,81 e 7,95, o esperado para dados cifrados.
- **Blocos repetidos:** há blocos de 16 bytes repetidos, alinhados, em 4 de 5 pacotes amostrados (até 1.007 repetições num pacote). Em fluxo aleatório isso é impossível. **Hipótese:** cifra de bloco de 128 bits em modo ECB, ou com o mesmo IV por bloco; texto claro igual (preenchimento, zeros) gera cifrado igual. É coerente com o AES do Crypto++ e com o relato do ResHax (pacotes recifrados em 2025).
- **`FileSystem_Stdio.dll` do NA** (3,4 MB; seção `.vmp0` de 580 KB, compilado em `Z:\187B\global\HeroesCode\…\filesystem_stdio_heroes`):
  - **Strings de pacote:** `hfs\%s.hfs`, `Failed to LoadPknSync Package_Exception`, a mensagem de erro com SHA-256 por arquivo, e os tipos `Lambda::PackageHeader`, `PackageHeader2` e `PackageHeader3`.
  - **Crypto++:** filtros autenticados (`AuthenticatedDecryptionFilter`), modos de bloco, assinaturas DSA/ECDSA e uma chave pública EC em hexadecimal (`0x24b5f8`, 210 caracteres; serve para verificar assinatura, não para decifrar).
  - **Ofuscação:** 199 nomes de classe do Crypto++ estão ofuscados (`.?AV<aleatório>@@@CryptoPP@@`).
  - **Chave:** nenhuma chave simétrica ou constante de derivação em claro, ao contrário do formato v2 (`MBHEROES!@0u9`).
- **Conclusão:** recuperar a chave exigiria engenharia reversa do código protegido por VMProtect (estática profunda) ou execução do DLL do NA para observar a chave (emulador ou depurador). As duas coisas estão fora da autorização atual.

### 7.3 Outras pistas públicas não usadas

- **"HFS Extractor v4", de Kudv (RaGEZONE):** diz abrir o formato atual dos clientes TW e US. O tópico pede cadastro (HTTP 403 sem login) e, pelo que a listagem mostra, distribui um executável. Executável sem fonte não pode ser revisado, por isso não foi baixado.
- **ZenHAX, tópico 9175:** trata da recifragem de 2021 (resolvida pelo HFS v2), não da de 2025.

### 7.4 Critério de fracasso

O C1 não decifrou os pacotes do NA com nenhuma ferramenta revisada. Pelo critério do pedido, o caminho C para aqui. O C2 e o C3 não foram iniciados. O plano do caminho B (seção 4.B) foi apresentado ao operador, sem ser iniciado.

### 7.5 O que continua possível sem ler o NA

A maior parte do C2 não depende do NA:
- o conteúdo da Delia no 1.69;
- a pasta `models/player/delia` do 1.69;
- as guardas do servidor Rust;
- a leitura pontual da tabela de classes e do caminho da Hagie no `client.dll` 1.69.

Só a comparação de assets e de linhas com o NA fica perdida. Retomar o C2 assim é decisão do operador.

### 7.6 HFS v4 aberto com o `NPCodex/hfs-extract`

- **Ferramenta:** `github.com/NPCodex/hfs-extract`, commit `8c065ca` (15/10/2026, MIT), indicada pelo operador como o "HFS Extractor v4" do Kudv. O código-fonte foi clonado em `.cache/hfs-extract-v4-src`; o zip binário da release não foi baixado.
- **Revisão:**
  - C# .NET 8, sem rede, processos, registro ou chamadas nativas.
  - O CLI recusa caminhos absolutos, `..`, nomes reservados e links simbólicos, e não sobrescreve sem `--overwrite`.
  - O leitor limita contagens e faixas ao tamanho do arquivo e confere o MD5 de cada arquivo extraído.
  - Compilado do código-fonte.
- **Formato (documentado em `docs/HFS-v4.md` do repositório):** versão 4; segredo de 13 caracteres gravado no próprio pacote; ChaCha20 com chaves derivadas do nome do pacote e do segredo, mascaradas por uma constante de 32 bytes; só os primeiros 1.024 bytes de cada arquivo cifrados, o resto zlib em claro.
- **Resultado no NA (`--list`, só leitura):** 9.220 pacotes, 355.640 arquivos, 101,8 GB descomprimidos, 0 falhas (`.cache/verification/c1-na-list.tsv`). O `FileSystem_Stdio.dll` do NA medido tem o SHA-256 `b81f29ad…cf7c`.
- **Extraído para `.cache/na-extract`** (1,4 GB, ignorado pelo Git, 0 falhas reais):
  - `sqlite/heroes.db3`;
  - os 3.156 arquivos com "delia" no caminho: 154 em `models/player/delia`, animações `_anim/female/delia`, efeitos, sons e `scaleform/video/skill`;
  - os `csf/createcharacter_*` das 32 classes, `models/weapon/bastardsword` e os vídeos de `scaleform/video/createcharacter`;
  - 16 avisos de "arquivo já existe" pela sobreposição dos filtros.
- **Em claro (formato Source conhecido):** `.mdl` (`IDST`), `.vtf`, `.vvd`/`.vtx`, `.wav`, `.csf` e as listas de animação `_anim/*.txt`. Os assets da Delia, do Bastardsword e da criação de personagem do NA estão legíveis.

### 7.7 Segunda camada: `heroes.db3` e scripts

- **O que chega cifrado:** o `heroes.db3` extraído passa no MD5 do pacote, mas não é SQLite (entropia 7,97; 103.789.569 bytes = 101.357 páginas de 1.024 + 1 byte). Os `scripts/**/*.txt` (por exemplo, `expression_table_delia.txt`) também chegam cifrados. O `resource/heroes_english.txt` vem em claro.
- **Onde fica a decisão:** no `FileSystem_Stdio.dll` do NA, a abertura de arquivo (função perto de `0x100ed27a`, leitura pontual com o capstone de `scripts/client/.deps`) compara:
  - a extensão com `.db3` e `.bsp`;
  - o início do caminho com `scripts` (7), `sqlite` (6) e `resource\localized_text` (23).
- **Hipótese:** nesses casos, conforme um sinalizador do pacote (`[obj+0x48]+0xa6`), o arquivo é envolvido num fluxo próprio (vtable `0x10241fc4`, com 10 métodos), que decifra na leitura. Os métodos medidos são pequenos e só delegam (indireções); a rotina de cifra em si não foi achada, e pode estar no `.vmp0`.
- **Pesquisa pública:** nada sobre a cifra do `heroes.db3`. O artigo do Falkrian (20/09/2026), base do extrator, cobre só o HFS.
- **Consequência para o C1:** os assets da Delia estão legíveis, mas a comparação de tabelas entre o NA e o 1.69 (Delia, Miri, segundas armas) continua bloqueada pela segunda camada.
- **Próximo passo possível:** continuar a leitura pontual da vtable `0x10241fc4` até a rotina de cifra. Se ela estiver virtualizada, o caminho seria o carregador isolado do DLL (seção 7.2), que pede decisão do operador.

### 7.8 Segunda camada medida: chave periódica, rotina no `.vmp0` (27/10/2026)

- **Formato:** cada arquivo tem 1 byte de prefixo; o resto é XOR com uma chave de 4.096 bytes que se repete (índice = posição no corpo mod 4.096).
- **Scripts:** todos usam a mesma chave, recuperada inteira pela estatística do texto. O `player_costume.txt` decifra limpo (3 MB, comentários coreanos em CP949), assim como os outros `scripts/*` da Delia.
- **`heroes.db3`:** usa outra chave, sem relação simples (deslocamento ou XOR) com a dos scripts. Recuperação estatística, em ordem:
  - byte mais comum;
  - máxima verossimilhança com histogramas do `heroes.db3` 1.69;
  - EM nas 4 classes de página;
  - restrições de estrutura do SQLite.
- **Resultado no `heroes.db3`:** cabeçalho, tipos de página e ponteiros de célula coerentes; 939.693 células legíveis e 48.900 ruins (cerca de 5%). O SQLite ainda responde "database disk image is malformed". Duas passadas de refinamento não mudaram nada:
  - nos inícios de célula ruins, 1.784 posições e 0 trocas aceitas;
  - nos ponteiros filhos das páginas internas, a melhor chave valida 4 de 83 filhos (ruído).
- **Código:**
  - os métodos da vtable `0x10241fc4` só delegam;
  - a máscara `& 0xfff` no `.text` aparece só no AES do Crypto++ (verificação de aliasing em `0x1018d0f5`) e no CRT (`0x101c89fa`);
  - há 4 ocorrências no `.vmp0` (`0x102cfa0c`, `0x102eef78`, `0x102f83e1`, `0x102ffc68`);
  - nenhuma janela de 8 bytes das duas chaves recuperadas aparece nos DLLs de 32 e 64 bits, então a chave é gerada em tempo de execução, não uma tabela fixa;
  - o `bin_x64/FileSystem_Stdio.dll` também tem `.vmp0` (1,05 MB).
- **Conclusão:** a geração da chave do `.db3` não está no código legível; o indício aponta para o `.vmp0`. Pela regra desta etapa, a leitura de código para aqui e aguarda a decisão do operador.
- **Opções:**
  1. **Leitura tolerante do banco parcialmente decifrado:** um leitor próprio de páginas e registros que pula as células ruins e busca as tabelas e linhas da Delia, Miri e segundas armas. Não pede nova autorização; lacuna de cerca de 5% das linhas, a marcar como incerta.
  2. **Carregador isolado do DLL de 32 bits:** em processo próprio e sem rede, chamar a abertura e leitura do `FileSystem_Stdio.dll` para obter o `heroes.db3` em claro. Executa código proprietário protegido pelo VMProtect, que pode ter anti-depuração; exige decisão explícita.
  3. **Sem o banco:** comparar só pelos scripts decifrados, pelo `heroes_english.txt` e pelos dados do 1.69.
