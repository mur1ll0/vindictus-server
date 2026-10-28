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

### 7.9 `heroes.db3` do NA em claro pelo carregador isolado (27/10/2026, autorizado pelo operador)

- **Autorização:** o operador autorizou no chat o carregador isolado do `FileSystem_Stdio.dll` de 32 bits, em processo próprio, sem rede e sem os servidores da Nexon, só para ler o `heroes.db3`.
- **Carregador:** um programa C++ próprio, compilado com o MSVC x86, com fonte e binário em `.cache/na-loader-src` (ignorado pelo Git). Ele:
  - carrega o DLL e as dependências (`tier0.dll`, `vstdlib.dll`) direto de `F:\Games\vindictus\appdata\en-US\bin`, sem copiar nem alterar nada;
  - roda em `.cache/na-loader-run`, onde uma junção `hfs` aponta para a pasta `hfs` do NA. A biblioteca HFS procura `hfs\*.hfs` relativo à pasta atual, então as leituras vão ao NA e qualquer escrita fica no `.cache`.
- **Sequência recuperada por leitura pontual (vtable `VFileSystem017` em `0x10243c64`):**
  1. `fs[95](true)` liga o modo HFS (`[fs+0xa5]`);
  2. `fs[3]` `Init` cria o gerenciador de HFS (`0x101007e0`, vtable `0x10244240`, global `0x102b44d8`) e liga `[fs+0xa6]`, o sinalizador que o `Open` consulta para decifrar;
  3. o método `[9]` do gerenciador o habilita (`[+0x44]`);
  4. `fs[11]` `AddSearchPath(appdata\en-US, "GAME", tail)`;
  5. em `VBaseFileSystem011`, `[2]` `Open`, `[0]` `Read` e `[3]` `Close`.

  Não foi preciso o `Connect`: o `HRInterface001`, que vem do `engine.dll`/`client.dll`, não foi carregado. Sem os passos 1 e 3, `Open` devolve nulo.
- **Isolamento observado:**
  - o processo do carregador não teve nenhum *endpoint* TCP/UDP, conferido durante cada execução;
  - nenhuma DLL de rede foi importada pelo `FileSystem_Stdio.dll`; o `tier0.dll` importa 5 funções do `WS2_32`, que não foram usadas de forma visível;
  - nenhum arquivo foi criado na pasta de execução.
  - Não verificado: possíveis escritas do `tier0.dll` no registro.
- **Resultado:** `.cache/na-db/heroes-plain.db3`, 103.789.568 bytes (101.357 páginas de 1.024), SHA-256 `8eb596f807df67b0…`, `pragma quick_check` = ok, 495 tabelas. A chave estatística da seção 7.8 fica obsoleta.

### 7.10 Comparação NA × 1.69 (`heroes.db3`)

Consultas só leitura; logs em `.cache/verification/c1-db-*.log`.

- **Estrutura:**
  - 495 tabelas no NA e 151 no 1.69: 144 em comum, 351 só no NA e 7 só no 1.69;
  - `ItemClassInfo.ClassRestriction`: no 1.69 é máscara de bits (`2047` = 11 classes); no NA é texto com nomes e expressões (`all`, `female`, `delia`, `female(!delia)`). O NA lista 28 personagens (`Emotion.RequiredClass`).
- **Delia:** já existe no 1.69 como classe de bit `1024` (classe 10).

  | Tabela (filtro por nome) | NA | 1.69 |
  |---|---|---|
  | `ItemClassInfo` | 419 | 196 (187 com `ClassRestriction` = 1024) |
  | `EquipItemInfo` | 298 | 170 |
  | `CustomizeItemInfo` | 798 | 455 |
  | `CustomizePriceInfo` | 2.743 | 1.253 |
  | `SkillRankInfo` | 304 | 304 |
  | `SkillArgumentInfo` | 512 | 512 |

  A arma é `BASTARDSWORD` (124 itens no NA, 62 no 1.69). As habilidades têm a mesma contagem nas duas versões; a diferença está em itens, roupas e customização posteriores.
- **Miri:** ausente no 1.69 (0 itens, 0 habilidades). No NA: 1.022 itens, 304 ranks de habilidade, arma `DRAKER` (77 itens), sem nenhum `DRAKER` no 1.69.
- **Segundas armas** (`EquipSharingCharacterWeaponType`, só no NA):
  - Hurk: `GREATSWORD` → `TEIDE`;
  - Lynn: `BATTLEGLAIVE` → `BLUTE`.

  No NA há 94 `TEIDE` e 72 `BLUTE`; no 1.69, nenhuma ocorrência de `teide` ou `blute` em qualquer tabela. As primeiras armas existem nas duas (`GREATSWORD` 162/107, `BATTLEGLAIVE` 154/99).
- **Consequência para o C2:** a Delia é o caso mais barato, porque os dados de classe, habilidade e arma já estão no 1.69. Falta medir as camadas de cliente:
  - modelos e animações (`models/player/delia` do NA extraídos);
  - interface de criação;
  - código nativo do `client.dll` 1.69 para a classe 10.

  A Miri e as segundas armas exigem dados novos (tabelas, tipo de arma) além dos assets.

## 8. Caminho C, etapa C2: o que a Delia precisa no 1.69 e como tirar o limite de 10 classes (27/10/2026)

Leitura pontual, sem análise completa:
- `client.dll` 1.69 original, SHA-256 `b284ba719047556b…`, base `0x38000000`;
- `server.dll` do cliente, base `0x40000000`.

Ferramentas: capstone de `scripts/client/.deps`, busca de strings, referências `imm32` e RTTI. A cópia de laboratório do `client.dll` (`314272ec…`) já tem os patches do projeto e não foi usada como referência.

### 8.1 Camadas

| Camada | Estado no 1.69 | Evidência |
|---|---|---|
| Dados de servidor (`heroes.db3`) | **presentes**: classe 10, bit 1024, 196 itens, 304 ranks e 512 argumentos de skill, `BASTARDSWORD` | seção 7.10 |
| Assets do cliente | **presentes**:<br>• `models/player/delia` (146);<br>• `_anim/female/delia` (13 arquivos, o mesmo número da Hagie);<br>• `models/weapon/bastardsword`;<br>• `effects/player/delia` (61);<br>• `player_action_state_delia(_ex).txt`, `expression_table_delia*.txt`;<br>• `csf/bodyshape_delia_default.fig`;<br>• vídeos `NewCharacter_Delia.usm` e 20 vídeos de skill;<br>• ícones | `.cache/play-client/hfs_extracted` |
| Tabela de classes | **10 entradas fixas, nos dois DLLs** | ver abaixo |
| Entidade do jogador | **ausente**: não existe `C_HeroesPlayerDelia` nem `CHeroesPlayerDelia` | RTTI |
| Arma | **ausente**: nenhuma ocorrência de `bastard` no `client.dll` nem no `server.dll` | strings |
| Mecânica (estados de ação) | **quase toda em script**; faltam 5 condições nativas | ver abaixo |
| Criação e seleção | **código desenrolado por classe**; a Delia não tem ramo | ver abaixo |

### 8.2 Onde está o limite de 10 classes

- **`client.dll`:**
  - pares (nome, tipo de corpo) em `0x38cafd50`: Lethita 1, Fiona 2, Evy 2, Kalok 3, Kay 1, Vella 2, Hurk 4…;
  - acessor sem limite em `0x3879ec50` (11 chamadas);
  - acessor do tipo de corpo com `cmp eax, 0xa` em `0x3879ec60` (2 chamadas).
- **`server.dll` do cliente** (simulação local da batalha):
  - a mesma tabela de pares em `0x40d0cdf8`;
  - uma lista só de nomes em `0x40d140dc`.
- **Entidades:** cada classe tem sua subclasse de jogador nos dois DLLs, mas elas são finas. Contra os 452 métodos da base `C_HeroesPlayer`:

  | Classe | Métodos trocados |
  |---|---|
  | Lethita, Fiona | 11 |
  | Vella | 14 |
  | Hagie | 15 |
  | Hurk | 17 |
  | Lynn | 19 |
  | Arisha | 20 (+1 novo) |
  | Kalok | 24 |
  | Kay | 24 (+3 novos) |
  | Evy | 31 |

  O comportamento comum fica na base; o próprio de cada classe, nos scripts de estado de ação.
- **Criação de personagem:**
  - sequências desenroladas por classe, com strings fixas: `CsfChanged.create_<classe>`, `trial_<classe>N` (`0x384a3976`…), `SelectHagie`, `trigger_select_hagie`, `npc_hagie`;
  - fundo `map_background game_create_character[_vin]_<classe>` (`0x388e8fa6`), escolhido conforme um serviço que responde por nome se a classe está habilitada (`[0x38cd2f90]` método `+0x18`).
  - O NA trocou isso por cenas genéricas: só Arisha, Hagie e Lynn têm mapa próprio entre 28 classes.
- **Mecânica:**
  - o script da Delia usa 5 condições sem implementação nos DLLs do 1.69: `As_Cond_Can_Armor_Purge`, `As_Cond_Delia_SPSkill3_Activated`, `AS_Cond_Delia_Guard`, `AS_Cond_Delia_Guard_Hit`, `AS_Cond_Delia_Special_Hold_Skill`;
  - o script da Hagie usa só condições existentes.

### 8.3 Conclusão do C2 (critério de fracasso)

A Delia precisa de código nativo, mas não escrito do zero. Ninguém escreve uma classe e uma arma inteiras; o trabalho é estender tabelas e clonar entidades, com pequenos trechos novos:

1. **Tabelas de classe:** realocar as três tabelas para uma área nova com N entradas e trocar as poucas referências e os limites `0xa`. Falta enumerar todos os outros usos do índice de classe com limite 10, partindo das 13 chamadas dos acessores.
2. **Entidade:** registrar `player_delia` e `DT_HeroesPlayerDelia` nos dois DLLs como cópia da entidade de uma classe próxima (Fiona ou Lethita, 11 métodos próprios), com a mesma tabela de rede.
3. **Arma:** apontar `BASTARDSWORD` para uma entidade de arma existente (`C_WeaponGreatSword`/`LongSword`), com o modelo do 1.69.
4. **Mecânica:** implementar as 5 condições em código novo, ou reescrever os trechos do script que as usam com condições existentes. A segunda opção é só dados, mas perde a guarda, o *armor purge* e a SP3 originais.
5. **Criação e seleção:** um ramo novo para a classe 10 nas sequências da criação (câmera, gatilho, vídeo, figura) e o botão na interface Scaleform da criação, reempacotada com `scripts/client/hfs.py`.
6. **Servidor do projeto:** `CHARACTER_COUNT = 10` e as validações de classe passam a aceitar N; o bit 2048 em diante fica para a 12ª classe.

Os passos 1 a 3 e 5 são edição de binário na cópia de laboratório (tabelas, registro e saltos para código novo); o 4 pode ser só de dados. A Miri e as segundas armas (`DRAKER`, `TEIDE`, `BLUTE`) somam assets do NA (a decisão de cópia é do operador), dados novos e uma entidade de arma cada.

### 8.4 Lacunas do C2

- Usos do índice de classe sem os acessores (vetores de 10 posições, `cmp …, 0xa`), a enumerar a partir das 13 chamadas e das referências às duas tabelas do `server.dll`.
- A tela de seleção (Scaleform) e o serviço de classes habilitadas não foram abertos.
- Se partes da criação estão no `.vmp0` do `client.dll` (`0x62600` bytes): as funções vistas estão todas no `.text`.

### 8.5 Primeiro passo implementável, com teste no cliente

Sem patch de binário: no banco de laboratório, uma personagem de teste passa a classe 10 (cópia de uma Fiona, com itens da Delia). O operador entra e observa a seleção, a cidade e uma batalha curta. O resultado (queda, pose T, modelo errado ou funciona) mostra qual camada falha primeiro e ordena os patches.

### 8.6 Preparação do teste sem patch (27/10/2026)

- **Servidor:** a lista de personagens (`minimum_character_list`) e a trava de login (`owned_first_fiona`) aceitam `class` de 0 a 10. Antes, uma personagem de classe 10 fazia a lista da conta inteira ser recusada (`Conflict`).
  - A criação continua de 0 a 9 (`character_creation_core`, `class_initial`, `fiona_initializer_profile`), porque a classe 10 não tem `DefaultItemInfo` e o cliente não a oferece.
  - As máscaras de `ClassRestriction` já eram genéricas (`1 << class`), então itens da Delia (bit 1024) servem para a classe 10 sem mudança.
- **O que fica como na referência:** a maestria de `BASTARDSWORD` e o nome da classe 10 no `Cond_class`. A referência C# 1.69 não tem nenhum dos dois; o enum `BaseCharacter` vai até 9, e `ToString()` de 10 dá `"10"`.
- **Ferramenta:** `scripts/dev/lab-set-class.ps1 -Name <nome> -Class <0..10>` troca a classe de uma personagem fora do jogo, pelo nome único, e imprime a classe anterior para desfazer.
- **Roteiro:** `docs/client-tests/C2-delia.md` (D0 a D5).

### 8.7 Criação e seleção com mais de 10 classes: o que o 1.69 já tem (28/10/2026)

Leitura pontual do `client.dll` 1.69 e dos filmes Scaleform (AS2) dos dois clientes. Os filmes do NA foram extraídos só para leitura em `.cache/na-extract/scaleform`.
- **Duas telas de criação no 1.69,** escolhidas pela FeatureMatrix em `0x38523690`:
  - se `S3_CharacterCreation` estiver ligada, abre `NewCharacterDialogS3`, que o `UIElements.xml` mapeia para `NewCharacterDialog_V2.gfx`;
  - senão, se `CharacterCreation_UI_V2` estiver ligada, abre `NewCharacterDialogV4`, a barra de 10 classes usada hoje.
  - O NA usa o mesmo mapeamento. O `NewCharacterDialogS3` é a grade de classes com modelo e prévia ao lado.
- **O `NewCharacterDialog_V2.gfx` do 1.69 já lista a Delia** (11 classes). O do NA lista 28 e usa *callbacks* que o `client.dll` 1.69 não tem: `OnFirstWeapon`, `OnSecondWeapon`, `SetCharaterIntroMovie`, `SetCharaterSkillMovie`, `SetNewIconCharacter`, `OnPlayRandomVoice` e outros. Por isso o filme do NA não serve direto no 1.69; o do 1.69 é o par do binário.
- **Habilitação:** `FeatureMatrix` do `heroes.db3` do cliente, região `EN-EU-X-GM`, com `CurrentVer` = `3.09.00`.
  - `S3_CharacterCreation` é NULL em todas as regiões.
  - A feature `Delia` pede `3.09.00`, então já está ligada.
  - A criação consulta as classes pelo nome no mesmo serviço (`Lynn`, `Arisha`, `Hagie`…).
- **Passo feito:** o laboratório envia `S3_CharacterCreation = 3.09.00` junto com as outras substituições de FeatureMatrix (`lab_frontend.rs`), sem patch de binário. A criação de uma classe 10 ainda é recusada pelo servidor (0–9), e entrar com ela ainda derruba o `server.dll` local (seção 8.6, segunda sessão).
- **Para passar de 11 classes** (Miri e as outras do NA), há duas saídas, e as duas pedem patch:
  - estender o filme S3 do 1.69, com edição de AS2 e ícones novos;
  - implementar no `client.dll` os *callbacks* do filme do NA.
  
  Antes de escolher, falta medir como o filme do 1.69 monta a lista: fixa no AS2 ou vinda do C++.
