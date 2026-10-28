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
- **Correção (sessão D6, 28/10/2026):** a grade S3 do 1.69 está incompleta. O mapa `s3_game_create_character` só tem atores para 9 classes (sem Hagie nem Delia), o modelo não muda e a criação trava no cliente. O envio de `S3_CharacterCreation` foi desfeito. O mapa do NA é idêntico; a criação de 28 classes do NA usa outra cena, ainda a localizar.

### 8.8 A criação em grade do NA e o plano para o 1.69 (28/10/2026)

Leitura pontual do `client.dll` do NA (32 bits, base `0x38000000`). O código de criação está no `.text`, legível; o `.vmp0` não foi tocado.
- **A grade do NA** é o mesmo `NewCharacterDialogS3` (`NewCharacterDialog_V2.gfx`), mas na versão nova do filme, que lista 28 classes. A prévia de cada classe toca `csf/createcharacter_<classe>.csf` em vez de atores no mapa. O 1.69 já tem os 11 `.csf`, inclusive o da Delia.
- **Contrato filme ↔ `client.dll` do NA** (nomes presentes nos dois):
  - C++ → filme: `SetNewIconCharacter`, `SetCharacterName`, `SetCharacterDescription`, `SetCharaterAttribute`, `SetCharaterIntroMovie`, `SetCharaterSkillMovie`, `SetWeaponButton`, `SetScreenSize`, `ShowCharacterInfo`, `ClearCharacterInfo`, `ClearCharacterSkillMovieInfo`;
  - filme → C++: `OnStart`, `OnReadyCreation`, `OnStartCreation`, `OnPlayRandomVoice`, `OnStopRandomVoice`, mais o comando `Select <classe>`.
  - O `client.dll` 1.69 conhece só `OnStart`, `SetCharacterName` e `ShowCharacterInfo`.
- **Despachante do NA** (`0x389e06f0`…): trata `Select`, converte o nome em índice (`0x3820d9c0`) e confere `< 0x1c` (28 classes). Também chama `SetNewIconCharacter` (referência `0x389e07af`) e `SetCharaterIntroMovie` (`0x389e0efd`, `0x389e1218`). `OnStartCreation` fica em `0x38bdf541`; a feature `CharacterCreation_UI_V3` é referenciada em `0x380921d9`, `0x381e4c1b`, `0x3888c200` e `0x3888cdee`.

**Plano para o 1.69** (cliente e servidor), na ordem:
1. **Tabelas de classe com N entradas** no `client.dll` e no `server.dll` da cópia de laboratório:
   - realocar `0x38cafd50` (client) e `0x40d0cdf8`/`0x40d140dc` (server) para uma área nova;
   - trocar os limites `0xa` e as referências.
   - É pré-requisito de tudo; sem efeito visível com 10 classes, e com a Delia na posição 10.
2. **Entidade do jogador da Delia** (`player_delia` e `DT_HeroesPlayerDelia`) nos dois DLLs, como cópia da entidade da Fiona (11 métodos próprios), mais a arma `BASTARDSWORD` sobre uma entidade de arma existente. Resolve a queda do `server.dll` ao entrar na cidade.
3. **Criação em grade:**
   - levar o `NewCharacterDialog_V2.gfx` do NA (e as imagens que ele usa) para a cópia de laboratório; **exige decisão do operador**;
   - implementar no `client.dll` 1.69, em código novo, o contrato acima, com a lista de classes vinda de uma tabela gerada pelo projeto (nome, descrição, `.csf`, armas). As rotinas legíveis do NA servem de referência.
   - Ligar `CharacterCreation_UI_V3`/`S3_CharacterCreation` pelo laboratório.
4. **Servidor:** criação da classe 10 (initializer da Delia; o 1.69 não tem `DefaultItemInfo` da classe 10, mas o banco do NA pode servir de referência), `CHARACTER_COUNT` e as travas 0–9 da criação.

A alternativa de estender a barra V4 (um botão a mais e ramos fixos para a Delia) é menor para uma classe, mas não escala: cada classe nova repete o trabalho.

Ferramenta de patch: seguir o modelo de `scripts/client/patch_shop_list_limit.py` (patch verificado por hash na cópia de laboratório, nunca no original).

### 8.9 Passo 1 feito: tabelas de classe com N entradas; a entidade da Delia no NA (28/10/2026)

- **Decisão do operador:** usar o NA como fonte (grade de criação e entidade da Delia migradas por engenharia reversa, sem clonar a Fiona). Fica autorizado copiar para a cópia de laboratório o filme de criação do NA e as imagens dele.
- **`scripts/client/patch_class_table.py`** (simulação por padrão, `--apply`; recusa o original e confere os bytes de cada ponto):
  - acrescenta a seção `.hcls` ao `client.dll` e ao `server.dll` da cópia, com as tabelas de pares {nome, corpo} e de nomes para 32 classes, preenchidas com as 28 da tabela do NA (`client.dll` NA `0x3958ad38`; as 10 primeiras são idênticas às do 1.69 e a Delia é corpo 2);
  - aponta os acessores para a seção nova e sobe os limites para 11 classes (as 10 mais a Delia): `client.dll` `0x3879ec54`, `0x3879ec68`, `0x3879ec6d`; `server.dll` `0x40755b44`, `0x40755b58`, `0x40755b5d`, `0x408b3984`, `0x408b3989`;
  - move o diretório de relocação para a seção nova, com um bloco a mais para os ponteiros dela (os DLLs não têm ASLR, mas a imagem continua relocável); zera o diretório de assinatura, que já não cobria a cópia.
  - Hashes: `client.dll` `314272ec…` → `a9591d3a…`, `server.dll` `d7727e18…` → `af43431e…`; cópias `*.pre-hcls.bak` ao lado.
- **A entidade da Delia no NA** (sem RTTI; achada pela fábrica `player_delia` → `C_HeroesPlayerDelia`, criação `0x3883d3d0`, construtor `0x3883cfa0`, vtable `0x393aa098`):
  - o jogador do NA tem 538 métodos virtuais (1.69: 452) e ocupa `0x7c20` bytes (Hagie: `0x7b78` no NA, `0x4bf8` no 1.69). A base cresceu com o conteúdo posterior (níveis, segundas armas, sistemas novos), então *slots* e *offsets* não coincidem: o código do NA não pode ser copiado, precisa ser reescrito sobre a base do 1.69;
  - a Delia troca 28 *slots* da vtable (Hagie 29): `0, 13, 15, 16, 17, 29, 31, 83, 190, 191, 286, 301, 330, 331, 349, 351, 380, 416, 419, 455, 470, 476, 477, 485, 487, 489, 516, 534`.
- **Próximos passos da entidade:**
  1. ler os 28 métodos e mapear cada *slot* e campo para o equivalente do 1.69, usando a Hagie como par: ela existe nas duas versões e permite casar *slots* e *offsets*;
  2. o mesmo no `server.dll` do NA (`CHeroesPlayerDelia`, `player_delia`);
  3. registrar `player_delia`/`DT_HeroesPlayerDelia` nos DLLs da cópia, com o código reescrito numa seção nova;
  4. a arma `BastardSword` e as 5 condições de estado de ação.

### 8.10 Passo 2, primeira parte: a entidade da Delia lida nos DLLs x64 do NA e reconstruída em Rust (27/10/2026)

- **Pedido do operador:** engenharia reversa do `client.dll` e do `server.dll` x64 do NA (`.cache/bin_x64`, cópia idêntica de `F:\Games\vindictus\appdata\en-US\bin_x64`), em Rust e o mais parecido possível com os métodos. Escopo escolhido pelo operador: a Delia e as classes novas. A leitura é pontual (fábrica, construtor, vtables e os métodos trocados); nada do binário inteiro foi analisado e o `.vmp0` não foi tocado.
- **Por que o x64:** tem a tabela de exceções (início e fim de cada uma das 116.048 e 92.618 funções), o que delimita os métodos sem análise de fluxo. O VMProtect moveu essa tabela para fora da seção `.pdata`; o diretório de dados 3 aponta a posição certa.
- **Evidência reprodutível:** `scripts/reverse/na_x64_delia_entity.py` confere os hashes (`client.dll` `0be9bfa5…`, `server.dll` `9f674dba…`), refaz construtor → vtable e grava `docs/evidence/client/na-x64-delia-entity.json` (local, fora do Git como o resto de `docs/evidence/client/`) com os *slots* trocados pela Delia e pela Hagie contra a base.

| | `client.dll` x64 | `server.dll` x64 |
|---|---|---|
| Classe | `C_HeroesPlayerDelia`, 0x8ae0 bytes | `CHeroesPlayerDelia`, 0xaf98 bytes |
| Construtor / vtable | `0xaf1220` / `0x1af6f10` | `0x6e3ae0` / `0x10f17b0` |
| Base (construtor / vtable) | `0xabeb50` / `0x1af1340` | `0x692630` / `0x10e89e8` |
| *Slots* da base | 538 (igual ao NA 32 bits) | 752 |
| *Slots* trocados pela Delia | 26 (os 28 do 32 bits menos 477 e 534, que aqui apontam para a base) | 37 |

- **O que a Delia acrescenta no cliente:**
  - **guarda:** valor protegido em `+0x8980` (célula no *heap*, semente de 64 bits e chave de 1 byte; cada escrita avança o LCG do `rand()` do MSVC e grava `valor ^ chave*0x01010101`). *Slots* 487/489 gravam e leem; 330 decide se o golpe é bloqueado; 416 zera o dano do golpe bloqueado, marca o bit 2 de `+0xbc` e chama a base; 470 só chama a base fora da guarda;
  - **`SET_GUARD_LATCH_ATTEN`** (eventos 0x7a/0x7b do script de estado): guarda os números do comando em `+0x8ac8`, uma vez;
  - **lançamento de *ragdoll*** (`+0x89b0`, evento 0x14a): `ENABLE_RAGDOLL_LAUNCH <força>`/`DISABLE_RAGDOLL_LAUNCH`; no acerto, marca `npc+0x4ab0` em NPCs elegíveis, exceto `c_npc_seiren`;
  - **habilidade especial segurada** (`+0x8aa0`): `START_SPECIAL_HOLD_SKILL`, `DASH_TO_TARGET`, `END_SPECIAL_HOLD_SKILL`;
  - **parâmetros de pose `run_yaw`/`sprint_yaw`** nas atividades `0x465`/`0x468`, com os ConVars `plr_poseparam_*` (180/55 corrida, 40/20 *sprint*) e `delia_run_direct_stabilize_count` (5);
  - **falha da habilidade especial** (*slot* 476): mensagens `#GameUI_Heroes_delia_skill_fail_msg[_location|_transform]` e `#GameUI_Heroes_SPGauge_Lack`, no máximo uma por segundo, e o som `Effect.Campfire.failed`;
  - figura `csf/bodyshape_delia_default.fig` (*slot* 301), vida no painel superior (*slot* 516) e constantes (0,88, 0,96, 92 e 89/92).
- **O que a Delia acrescenta no servidor local:** a mesma guarda (`+0xad28`, *slots* 697/699, forçada a 0 quando `0x7dd5e0` responde sim; 434/435/615/677/390 usam a decisão), a arma `weapon_bastardsword` (*slot* 657, estado em rede `+0xa6c4`), o ataque forte (*slots* 730/731: armado pelo *buff* `skill_delia_strong_attack_delay` ou por 1,5 s depois do acerto anterior; lança, derruba ou enfileira o alvo e o segura pelo *slot* 729) e as classes auxiliares `DeliaRagdollLaunch`, `DeliaSpecialHoldSkillController` e `CHeroesDeliaChargeAttackTraceFilter_OnlyProp`.
- **Reconstrução em Rust:** `tools/na-delia-reference` (crate isolado, fora do servidor e do Docker; `cargo test --manifest-path tools/na-delia-reference/Cargo.toml`, 27 testes). Cada função cita o *slot* e o RVA de origem; chamadas de engine não reconstruídas ficam atrás dos *traits* `ClientHost`/`ServerHost`, com o nome do RVA chamado.
- **Lacunas desta parte:**
  - não lidos: a atualização dos 5 alvos do *ragdoll* (`0xaf39f0`), o *think* da habilidade segurada (`0xaf3670`), o resto do *slot* 615 e os *slots* 731/725/73/517 do servidor além do resumo, as classes auxiliares do servidor e as 5 condições `As_Cond_*`/`AS_Cond_Delia_*`;
  - nomes de vários *slots* da base continuam numéricos (190/191, 349/485 no cliente; 240/241, 466/694 no servidor);
  - a correspondência de cada *slot* e *offset* x64 com a base do 1.69 (32 bits, 452 *slots*) ainda não foi feita; é o próximo passo, pela Hagie.

### 8.11 Passo 2, segunda parte: condições, auxiliares e mapa de *slots* x64 → 1.69 (28/10/2026)

- **Condições de estado de ação:** não são código, são *flags* com nome. Cada DLL registra `nome → id` na inicialização (NA x64: cliente `0x1a5820`, servidor `0x1a26a0`, 231 nomes). As 5 da Delia são, no NA, `As_Cond_Can_Armor_Purge` 140, `As_Cond_Delia_SPSkill3_Activated` 141, `AS_Cond_Delia_Special_Hold_Skill` 142, `AS_Cond_Delia_Guard` 143 e `AS_Cond_Delia_Guard_Hit` 144.
  - Os ids são posições na lista e mudam entre versões: `As_Cond_Can_PerfectEndurable` é 79 no NA e 76 no 1.69. O registro do 1.69 (cliente e servidor) termina em 120 e não tem nenhuma das 5.
  - **Quem liga as condições:**
    - servidor (`0x6e4d30`, dentro do *slot* 517): 140 quando há armadura equipada (equipamento 4 a 8) e nada bloqueia; 141 pelo estado da habilidade `+0xaea8`. Os dois valores vão para o cliente em rede (`+0xaea0` → `+0x8a90`, `+0xaeac` → `+0x8a98`);
    - cliente (`OnDataChanged`, *slot* 5 da vtable secundária `+0x10`, `0xaf1f40`): repete 140/141 a partir da rede e, a cada golpe bloqueado novo (contador `+0x8abc`), liga 143 e 144;
    - cliente (`0xaf3670`, *think* da habilidade segurada): 142 enquanto o alvo está ao alcance (distância² ≤ extensão XY² do alvo + 450²), com o gesto de aviso no jogador local.
  - No golpe bloqueado (*slot* 416), o par usado é `As_Cond_Can_PerfectEndurable` → `As_Cond_Do_PerfectEndurable`.
- **Lançamento de *ragdoll* (cliente, `+0x89b0`):** até 5 alvos. Meio segundo depois do lançamento, o jogador local que lançou testa o chão com um *hull trace* e envia ao servidor `request_release_launch_ragdoll <slot> <entidade> <x y z> <pitch yaw roll>`. Antes disso, a *ragdoll* local é levada à posição da rede, se o salto for finito e de até 1024 unidades. A Seiren (`c_npc_seiren`) nunca é lançada.
- **Mapa de *slots* x64 → 1.69:** `scripts/reverse/na_x64_to_169_vtables.py` lê só as vtables de jogador das duas versões: no 1.69, pela RTTI; no x64, pelo método que devolve o nome da classe. O script compara 7 classes comuns por DLL, usando:
  - o conjunto de classes que trocam cada *slot*;
  - as strings e as constantes das trocas e da base.
  
  O alinhamento respeita a ordem de declaração. Resultado: 443 de 452 *slots* do cliente 1.69 e 641 de 646 do servidor pareados. A Hagie, que existe nas duas versões, confirma 12 âncoras (177↔190, 264↔301, 368↔419, 451↔515…). A tabela vai para `docs/evidence/client/na-x64-to-169-vtables.json` (local) e para `tools/na-delia-reference/src/layout.rs` (`port::CLIENT`, `port::SERVER`).

| | Existem no 1.69 (alta / média / baixa) | Só no NA |
|---|---|---|
| Cliente (26 *slots* da Delia) | 20 (8 / 7 / 5) | 330, 331, 349, 470, 485, 516 |
| Servidor (37) | 28 (12 / 9 / 7) | 434, 627, 631, 677, 694, 729, 731, 736, 748 |

- **O que isso significa para o porte no 1.69:**
  - as trocas com par no 1.69 entram numa cópia da vtable da entidade, no *slot* mapeado;
  - os métodos só do NA (verificar guarda, constantes 92 e 89/92, supressão durante a guarda, vida no painel, agarrar no servidor) nunca são chamados pela base do 1.69. O comportamento precisa ir para dentro das trocas que existem, ou para pontos de chamada novos;
  - as 5 condições precisam de ids novos acima de 120 nos dois registros do 1.69, e os scripts da Delia usam os nomes, não os ids;
  - os pares de baixa confiança (cliente 31, 380, 476, 487/489; servidor 28, 390, 435, 504, 628, 699, 730) devem ser conferidos antes de qualquer patch.
- **Reconstrução em Rust:** `condition` (ids das duas versões), `SpecialHoldSkill::think`, `DeliaClient::on_data_changed`, os 5 alvos do *ragdoll* (`free_slot`, `update_slot`, `take_release_request`, `should_snap`, `release_command`) e `DeliaServer::update_conditions`; 37 testes.
- **Lacunas:**
  - capacidade do armazenamento de condições no 1.69 (se ids acima de 120 cabem);
  - `0xaf3380` (habilidade segurada em `OnDataChanged`) e `0x235200` (início do voo do alvo);
  - corpo completo dos *slots* 517 e 731 do servidor (agarrar, `UPDATE_CHARGE_SMASH_EFFECT`);
  - quem chama, na base do NA, os métodos que não existem no 1.69.

### 8.12 Passo 2, terceira parte: capacidade das condições, revisão dos pares e plano do patch da entidade (28/10/2026)

- **Capacidade das condições:**
  - NA: as condições ficam num *bitset* do estado de animação (`+0x1b8`), com limite 231 (`cmp edx, 0xe7`) nas funções de ligar e desligar (`0x19c5a0`, `0x18a2b0`). A consulta (`0x18d7e0`) não confere limite.
  - 1.69: o mesmo esquema, com limite 121 (`cmp ecx, 0x79`) em quatro funções por DLL: cliente `0x7de3fb`, `0x7de44b`, `0x7de55b`, `0x7de5bb`; servidor `0x7e63cb`, `0x7e641b`, `0x7e647b`, `0x7e64db`. Os dois laços que percorrem todas as condições (cliente `0x7eac79`, servidor `0x7eed09`) imprimem a lista e não guardam nada.
  - Um `CBitVec` de 121 bits ocupa 4 *dwords* (128 bits). Os ids 121 a 125 devem caber sem mover campos, trocando só os 4 limites; isso foi deduzido do limite, não do *layout* do objeto, e precisa de confirmação no cliente.
  - O registro `nome → id` usa dois mapas (cliente `0x38fab1d8` e `0x38fab1f4`). Os 5 nomes da Delia precisam de código novo que os insira, com ids 121 a 125 (`condition::PROPOSED_V169`).
- **Revisão dos pares fracos.** As assinaturas x64 eram cortadas no primeiro pedaço da tabela de exceções (o MSVC divide funções grandes); com varredura linear, o script ficou mais confiável. Revisão manual registrada em `REVIEW` no script:
  - **confirmados:**
    - cliente 31 ↔ 31 (mesmas strings `Player.Common.Critical_Hit`);
    - cliente 380 ↔ 334 (`TiredRun`, `TiredIdleBegin`, `TiredIdleEnd`);
    - cliente 487/489 ↔ 429/430 e servidor 697/699 ↔ 618/619: o par "estado da classe" (gravar/ler). As classes que o trocam no 1.69 (Fiona, Evy, Kalok, Arisha) estão contidas nas do NA, que o trocam em quase todas. A guarda da Delia usa esse par existente; não é um método novo;
  - **recusados** (passam a só NA): servidor 435 (*stub* de 2 instruções contra 47), 504 e 628 (corpos sem correspondência);
  - **continuam fracos:** servidor 390 ↔ 346 (compartilham só `scripts/action_table_NPC.txt`) e 730 ↔ 641 (só posição).
  - Resultado: cliente 20 de 26 com par (12 altos, 8 médios); servidor 25 de 37 (16 altos, 7 médios, 2 baixos).
- **Quem chama os métodos só do NA:** métodos da base que existem no 1.69 e são o ponto de entrada no porte (`layout::na_only_callers`):

| DLL | Método só do NA | Chamado pela base x64 | Mesmo *slot* no 1.69 |
|---|---|---|---|
| cliente | 470 (pula a base durante a guarda) | 469 | 413 |
| cliente | 485 (89 / *slot* 349) | 484 | 427 (par fraco) |
| servidor | 677 | 453 | 400 |
| servidor | 694 | 693 | 614 |
| servidor | 729 (segurar alvo), 731 | 624 e 730 | 563 e 641 |

  Os *slots* 330/331 (cliente) e 434 (servidor), que decidem se o golpe é bloqueado, aparecem em dezenas de chamadas indiretas sem dono identificado; a Delia os usa dentro das próprias trocas (416 no cliente, 615 no servidor), que existem no 1.69.
- **O que o 1.69 não tem:** nenhum dos comandos de script (`SET_GUARD_LATCH_ATTEN`, `ENABLE_RAGDOLL_LAUNCH`, `START_SPECIAL_HOLD_SKILL`, `DASH_TO_TARGET`), nem o comando `request_release_launch_ragdoll`, nem `weapon_bastardsword`, `skill_delia_strong_attack_delay`, `UPDATE_CHARGE_SMASH_EFFECT`, `holdskill_on_immune` ou as classes `DeliaRagdollLaunch`/`DeliaSpecialHoldSkillController`. Tudo é código novo nos dois DLLs. `GuardBegin` existe nas duas versões.

**Plano do patch da entidade da Delia** (cópia de laboratório; cada etapa com patcher verificado por hash e teste no cliente):

1. **E1 — entidade mínima:** registrar `player_delia` e `DT_HeroesPlayerDelia` nos dois DLLs, numa seção nova. A vtable é uma cópia da base do 1.69, com só o nome da classe (*slots* 13/16 no cliente, 9/11 no servidor), a figura do corpo (264) e as constantes 0,88/0,96 (177/178 no cliente, 221/222 no servidor). O objeto tem o tamanho da base do 1.69 e as tabelas de rede herdam as da base. **Teste:** entrar com a `DeliaLab` (classe 10). Esperado: a cidade carrega sem a queda em `server.dll`, com o modelo da Delia. Resolve a queda registrada no D3.
2. **E2 — arma:** estado `weapon_bastardsword` no servidor (*slot* 586 do 1.69) e a entidade de arma do cliente para `BASTARDSWORD`, sobre uma arma existente. **Teste:** a arma aparece na mão e o ataque básico funciona.
3. **E3 — condições e guarda:**
   - limites 0x79 → 0x7e e os 5 nomes registrados;
   - campos em rede (armadura removível, SP3, contador de golpes bloqueados);
   - `OnDataChanged` (vtable secundária, *slot* 5);
   - guarda pelo par 429/430 (cliente) e 618/619 (servidor);
   - golpe bloqueado no 367 (cliente) e no 555 (servidor);
   - `SET_GUARD_LATCH_ATTEN` no tratador de eventos (368).
   
   **Teste:** a guarda da Delia bloqueia dano e mostra a reação de golpe bloqueado.
4. **E4 — habilidade segurada e lançamento de *ragdoll*:**
   - cliente: alcance e gesto no *think* (334), eventos no 368, lançamento no 308/84;
   - servidor: o comando `request_release_launch_ragdoll` e as classes auxiliares, ainda não lidas.
   
   **Teste:** habilidade segurada num alvo e lançamento de um NPC comum (não a Seiren).
5. **E5 — ataque forte e restos:**
   - servidor: ataque forte (*buff* `skill_delia_strong_attack_delay`, 1,5 s) e o agarrar (563/641);
   - cliente: mensagens de falha (419), inclinação ao correr (334), métodos só do NA pelos chamadores da tabela acima (413, 427, 400, 614).

Antes do E1 falta ler o registro de fábrica e de tabela de rede do 1.69 (como `player_hagie` e `DT_HeroesPlayerHagie` são instalados), para reproduzir o mesmo formato.

### 8.13 Limite real de classes no NA e inventário dos limites de 10 no 1.69 e no servidor Rust (28/10/2026)

Pedido do operador (28/10/2026): ampliar para mais de 10 classes no cliente e no servidor, com o seletor em grade do NA e a Delia funcional. Leitura pontual (tabelas, acessores, `m_iClass` e os `switch` por classe); nenhuma análise do binário inteiro.

**Limite no NA** (`.cache/bin_x64`, hashes conferidos pelos scripts de 8.10; logs `.cache/verification/na-class-*.log`, `na-pet-index.log`):
- tabela de pares {nome, corpo} com **28 entradas** nos dois DLLs (cliente `0x1f85aa0`, servidor `0x1675a00`, passo 16 bytes) e lista de nomes (cliente `0x1f81970`, servidor `0x166f980`);
- acessor do nome com `cmp ecx, 0x1b; ja` (cliente `0xb4b30`, servidor `0xb80c0`): classes 0 a 27; laços de criação com `cmp ebx, 0x1b` (cliente `0x23b93e`, `0x23c94b`, `0x23d972`);
- **o pet é o índice 28**, logo depois da última classe: `switch` das expressões (`0x202dcb`, `cmp esi, 0x1c`) e das palavras-chave de habilidade (`0x28fdd8`, `cmp eax, 0x1c`) mandam o 28 para `expression_table_pet`/`pet_skill_keyword_table`. No 1.69 o pet é o 10, que também é `CHARACTER_COUNT` do enum `BaseCharacter` do C#: o pet é sempre "o índice depois da última classe", e por isso a Delia caiu no caminho do pet;
- **rede:** o servidor do NA envia `m_iClass` com `log2(28)+1 = 5` bits (`0x68db38`): valores 0 a 31. **É daí que vem o limite de 32**: 28 classes, o pet e 3 valores livres. O 1.69 envia com `log2(10)+1 = 4` bits (`0x408eedb0` chamado em `0x405368c7`): 0 a 15;
- **conteúdo:** no `heroes.db3` do NA, `ClassRestriction` é texto (`fiona`, `delia,miri`, `all`, `female`…), com 28 nomes de classe. No 1.69 e no servidor Rust é a máscara `int` (`1 << classe`), que comporta 32 classes (bit 31 é o sinal, mas `wrapping_shl(c & 31)` já trata os 32).

**Decisão de representação para o laboratório:** seguir o NA: classes 0 a 27 na ordem da tabela do NA (já na seção `.hcls`, com 32 posições), pet no índice depois da última classe e `m_iClass` com 5 bits. O teto é 32 (`.hcls`, máscara `int` e 5 bits); o 1.69 só tem conteúdo para as 10 classes e a Delia, e o servidor aceita só as classes com conteúdo de criação.

**Inventário no 1.69 de laboratório** (`scripts/reverse/v169_class_uses.py`; JSON local `docs/evidence/client/v169-class-uses.json`; log `.cache/verification/v169-class-uses.log`). O script visita as 181 leituras de `m_iClass` de cada DLL (campo `+0x18ec` no cliente, `+0x1400` no servidor, e os *getters*) e as referências às strings da Hagie:

| Tipo | Cliente | Servidor |
|---|---|---|
| `switch`/limite 9 (`ja` → padrão, a classe 10 não tem caso) | cerca de 20 (lista no JSON): estado de ação `0x387ed842`, eventos `0x3852e6f1`/`0x3852e7c4`, nomes em minúsculas `0x387a3ae4`, figura `0x388f9f2f`, `0x386f3bf6`, `0x38705f62`, `0x3872c978`, `0x3846a737`, `0x3848d678`, `0x38572961`, `0x38760397`… | 7: estado de ação `0x407efdbd`, `0x40539af2`, `0x402e4bc0`, `0x405465de`, `0x40608e0e`, `0x40692851`, `0x40835818` |
| limite 10 com caso 10 = pet | expressões `0x3880401a`, palavras-chave `0x38861c22`, `0x387033d7`, `0x3888643a`/`0x3888e700` (vetor `0x38fbc6c0`, 11 posições), `0x3888ce3c`, `0x38630c96` | expressões `0x40804ac0`, palavras-chave `0x407ceb22`, `0x4081efca`/`0x408258d0` (vetor `0x40f68510`), `0x4082369c` |
| tabela indexada sem limite (10 entradas; a classe 10 lê a tabela seguinte) | — | `.csf` de criação `0x40cf9e64` (lida em `0x405381da`), prólogo `0x40cf9e3c` (`0x405381ff`), prefixo de som `Player.<Classe>.` `0x40cf968c` (3 leituras), *costume* padrão `0x40cfa0b0` (`0x4054eedc`) e de PvP `0x40cfa0d8` (`0x4054ed1d`; só 9 nomes, sem Arisha) |
| tabelas de classe (`.hcls`, já com 32 posições) | acessores `0x3879ec50`/`0x3879ec60` | `0x40755b40`/`0x40755b50`, `0x408b3980` |

- A queda D3 (`0x408b4700`, *getter* `[ecx+0x1c]` com objeto inválido) é coerente com a fábrica: o nome da classe 10 agora é `Delia`, e `player_delia` não existe no 1.69 (E1).
- O conteúdo do 1.69 já tem os arquivos da Delia que esses `switch` e tabelas pedem: `player_action_state_delia[_ex].txt`, `expression_table_delia[_ex|_common].txt`, `delia_skill_keyword_table.txt`, `csf/create_delia.csf`, `csf/prologue_delia.csf`, `csf/costume_default_delia.csf`, `csf/bodyshape_*_delia.fig`.
- **Caminho do pet preservado:** o patch não reaproveita o caso 10 de pet. Em cada `switch` com pet, o limite sobe para o novo índice do pet e o caso 10 passa à Delia; os vetores de 11 posições (`0x38fbc6c0`, `0x40f68510`) mudam de lugar para a `.hcls` com 33 posições. O servidor Rust não cria pets hoje; quando criar, usará o novo índice.

**Servidor Rust** (pontos com 10 classes):
- criação: `character_creation_core.rs:57` (`class > 9`), `class_initial.rs:91/98` (até 10 classes, `> 9`), `fiona_initializer_profile.rs:363` (`> 9`), `scripts/content/export_class_initial.py` e `export_fiona_initializer.py` (`range(10)`, `0..9`);
- persistência e verificação de posse: `character_base_stats.rs:60`, `character_skills.rs:230`, `initial_inventory.rs:79` (`class BETWEEN 0 AND 9`); `minimum_character_list.rs:68` e `fiona_character_state.rs:516` já aceitam 0 a 10;
- regras: `npc_talk.rs:470` (`c < 10`), nomes de classe com 10 posições em `npc_script.rs:48`, `npc_talk.rs:1814`, `admin_tooltip.rs:40/326`, `admin_characters.rs:292`; editor de itens `item_editor.rs:34` (`ClassRestriction` até 4095, 12 bits);
- já genéricos: a coluna `class` (`SMALLINT` 0–255) e as máscaras `ClassRestriction` (`wrapping_shl(class & 31)`).
- **Dados de criação da Delia no servidor 1.69:** `CharacterClassInfo` e `CharacterBaseLoadInfo` têm a classe 10; `DefaultItemInfo`, `DefaultSkillInfo` e o `switch` `INNERARMOR` do `ItemImpl.cs` não têm. Existem os itens e as habilidades da Delia: `bastardsword_newbie`, `delia_inner_free`, `skill_delia_bastard_mastery`, `skill_delia_guard`.

**Seletor em grade:** o filme do NA (`NewCharacterDialog_V2.gfx`, `c88875d7…`, igual ao extraído em 8.7) não mudou nesta extração. Continua valendo o contrato de 8.8: 11 chamadas C++ → filme e 5 tratadores filme → C++ que o `client.dll` 1.69 não tem; o mapa `s3_game_create_character` não serve. **Bloqueio documentado:** trazer a grade exige código nativo novo no `client.dll` (o despachante do NA em `0x389e06f0` é a referência) e só faz sentido depois da entidade (E1), porque nenhuma classe acima de 10 cria uma personagem jogável sem ela. Até lá, a Delia de laboratório é criada pelo servidor (comando de laboratório), não pela barra V4, que é fixa em 10 classes.

## 9. Próximos passos registrados: Miri, segundas armas do Hurk e da Lynn, Dullahan (28/10/2026)

Pedido do operador (28/10/2026): registrar sem implementar. Contagens do `heroes.db3` do NA (`F:\Games\vindictus\hfsExtracted\sqlite`, copiado para `.cache/na-hfs`, `8eb596f8…`) e do `heroesContents.db3` do servidor 1.69. Logs: `.cache/verification/class-weapons-inventory.log` e `dullahan-inventory.log`.

### 9.1 Miri (classe 11 no NA)
- **NA:** 1.022 itens e 19 habilidades com `miri`; arma `DRAKER` (77 itens), maestria `skill_miri_draker_mastery` (primeira arma); entrada 11 da tabela de classes (corpo 2, feminino).
- **1.69:** nada. Nenhum item, habilidade, modelo, script de estado ou `.csf` com `miri` no servidor ou no cliente; `DRAKER` não existe em `EquipItemInfo`.
- **O que falta trazer:** modelos, animações, efeitos, sons e scripts (`player_action_state_miri*`, `expression_table_miri*`, `miri_skill_keyword_table`) do HFS do NA (decisão de cópia do operador); as linhas de `ItemClassInfo`/`EquipItemInfo`/`SkillInfo`/`CharacterClassInfo` da Miri convertidas para o esquema 1.69 (`ClassRestriction` em texto → máscara, bit 11); a entidade `player_miri` nos dois DLLs, no mesmo modelo do E1–E5 da Delia, e uma entidade de arma `DRAKER`; dados de criação (`DefaultItemInfo`, `DefaultSkillInfo`, *inner*) como os da Delia.
- **Pré-requisitos já prontos:** a `.hcls` tem a Miri na entrada 11; o limite de 32 classes e o pet deslocado (seção 8.13).

### 9.2 Segundas armas do Hurk e da Lynn
| Classe | Primeira arma (1.69 e NA) | Segunda arma (só NA) | Maestrias da segunda arma no NA |
|---|---|---|---|
| Hurk | `GREATSWORD` (162 itens no NA) | `TEIDE` (94 itens) | `skill_hurk_teide_mastery`, `skill_hurk_shooting_mastery` (`IsSecondWeapon`) |
| Lynn | `BATTLEGLAIVE` (154) | `BLUTE` (72) | `skill_lynn_blute_mastery`, `skill_lynn_vigor_mastery`, `skill_lynn_outside_power_mastery`, `skill_lynn_internal_power_mastery` |
- **1.69:** só a primeira arma de cada um (`skill_hurk_greatsword_mastery`, `skill_lynn_battleglaive_mastery`); `TEIDE` e `BLUTE` não existem, nem os arquivos de modelo e estado de ação.
- **O que falta trazer:** modelos e scripts de estado de ação das segundas armas; as entidades de arma nos dois DLLs; a troca de arma (o NA tem `OnFirstWeapon`/`OnSecondWeapon` na criação e `IsFirstWeapon`/`IsSecondWeapon` em `SkillInfo`, que o 1.69 já lê); itens e habilidades convertidos; regras do servidor para a escolha da arma (o servidor 1.69 não tem o conceito de segunda arma na criação).

### 9.3 Dullahan (chefe)
- **No nosso conteúdo e servidor: não existe.** Nenhuma linha com `dullahan` em `MonsterInfo`, `ItemClassInfo` ou `QuestInfo` do servidor 1.69; nenhum arquivo no cliente 1.69 (`.cache/play-client/hfs_extracted`); nenhuma referência em `src/` ou `docs/`. Os mapas do 1.69 vão até `3_19`.
- **No NA:**
  - **chefe:** `npc_dullahan` (8 linhas em `MonsterInfo`): nível 95 (`general`, com variações por feature `S4EP1`/`S4EP4`), `general_competition_raid` (`PvE_Competition`) e `ancient_ds` nível 125; modelo `models/monster/dullahan` e scripts `_dullahan_attack_motion.txt`/`_dullahan_common_motion.txt`;
  - **fases:** `quest_s3_ep7_dullahan` (nível 95, `S3CH1`), `quest_s3_ep7_dullahan_equal` e `quest_ancient_dullahan_125_ds` (nível 125); mapas `3_23.bsp`, `3_23_ds.bsp`, `3_23_ending.bsp` e as variantes `hw_s3*` que citam `npc_dullahan`;
  - **armas para todas as classes:** uma arma `*_dullahan` por tipo, 40 tipos, entre eles `bastardsword_dullahan` (Delia), `teide_dullahan`, `blute_dullahan` e `draker_dullahan` (as armas novas de 9.1 e 9.2), além de armaduras (`PLATE_ARMOR` masculina e 25 `CLOTH`), 50 avatares e materiais de combinação (`combine_*_lv95_dullahan`, `ReforgeMaterialInfo`). No total, 209 itens com `dullahan`;
  - **"quadro atualizado":** os tipos de arma do Dullahan cobrem as 28 classes do NA, inclusive as que o 1.69 não tem; para o 1.69 servem 18 dos 40 tipos, os que já existem em `EquipItemInfo` (`LONGSWORD`, `SHIELD`, `GREATSWORD`, `BATTLEGLAIVE`, `SCYTHE`, `PHANTOMDAGGER`, `BASTARDSWORD`, entre outros); os outros 22 dependem de classes ou armas que o 1.69 não tem.
- **O que falta trazer, na ordem:** (1) o mapa e os scripts do chefe, com o modelo, do HFS do NA (decisão de cópia do operador); (2) `MonsterInfo`, `QuestInfo` e as tabelas de *drop* e setores das três fases, convertidas para o esquema 1.69; (3) as armas e armaduras do Dullahan das classes que o 1.69 tem, com receitas de combinação e reforja; (4) as armas das classes que ainda não existem só depois delas (9.1, 9.2). As fases dependem do B2 (missões solo); a competição (`PvE_Competition`) e as variantes `ds` dependem do multijogador (B5).

### 8.14 Classe 10 roteada para a Delia nos DLLs de laboratório (28/10/2026)

- **Leitura nova:** o servidor local monta o nome da entidade como `player_<cl_playerclass>` e, se a fábrica não existe, cria a entidade genérica `player` (`0x403712f7`). Logo, a classe 10 pode carregar sem `player_delia`, com a base do jogador; o E1 acrescenta depois o que é só da Delia. A queda do D3 vinha da classe 10 cair nos casos do pet e nas tabelas sem limite (8.13), não da fábrica.
- **`scripts/client/patch_class_routing.py`** (simulação por padrão, `--apply`; exige a `.hcls`; confere os bytes de cada ponto):
  - seção `.hrte` com tabelas de salto novas para 7 `switch`. O caso 10 é uma cópia do caso da Hagie com as strings trocadas pelas da Delia, e os desvios são recalculados;
    - cliente: estado de ação `0x387ed842`, expressões `0x3880401a`, palavras-chave `0x38861c22`, nome em minúsculas `0x387a3ae4`;
    - servidor: estado de ação `0x407efdbd`, expressões `0x40804aca`, palavras-chave `0x407ceb22`;
  - nos três `switch` com pet, o pet vai para o índice 28 e as classes 11 a 27 caem no caso padrão;
  - as 5 tabelas sem limite do servidor passam a ter 32 posições: Delia na 10 (`csf/create_delia.csf`, `csf/prologue_delia.csf`, `Player.Delia.`, `csf/costume_default_delia.csf`; *costume* de PvP nulo, como o da Hagie) e as posições 11 a 31 repetindo a 0. As 9 referências realocadas foram redirecionadas;
  - `m_iClass` com `log2(28)+1 = 5` bits (`push 0xa` → `push 0x1c` em `0x405368c5`);
  - o diretório de relocação passa para a `.hrte`, com blocos para os 88 (cliente) e 234 (servidor) ponteiros novos.
  - Hashes: `client.dll` `a9591d3a…` → `0e1b1800…`, `server.dll` `af43431e…` → `eda9ac59…`; cópias `*.pre-routing.bak` ao lado.
- **Não coberto ainda:** os outros `switch` com limite 9 do inventário (eventos de configuração, HUD, figura `csf/<classe>_figure.fig`, que não existe para a Delia), os vetores de 11 posições indexados pela classe (`0x38fbc6c0` no cliente, `0x40f68510` no servidor; o acesso limita a classe e lê a linha 0 fora de 0–9, ver o resultado do D8) e a origem do valor 10 dos pets, que não foi achada (nenhuma escrita imediata em `m_iClass`; o valor vem dos dados do personagem). O servidor Rust não cria pets; quando criar, deve mandar 28.
- **Teste:** entrada D8 de `docs/client-tests/C2-delia.md`. Esperado: a cidade carrega com a classe 10 sem a queda do D3. Depois do D8 vem o E1 (entidade `player_delia` sobre a base do 1.69).
- **Resultado do D8 (28/10/2026, cliente):** a seleção mostrou um modelo da Fiona sem rosto em pose T; a cidade `t01` carregou e o cliente caiu cerca de 2 s depois de entrar (`EXCEPTION_ACCESS_VIOLATION` em `client.dll` `0x387da6b0`, leitura de `[0 + 0x5c]`). O servidor não registrou erro (`.cache/verification/d8-game.log`). O patch foi revertido na cópia de laboratório (`client.dll` `a9591d3a…`, `server.dll` `af43431e…`; as cópias roteadas ficaram como `*.routing.bak`) e a `DeliaLab` está na classe 1.
- **Causa (minidump `Report Error`, copiado para `.cache/d8/`):**
  - `0x387da6b0` é um *getter* (`mov eax,[ecx+0x5c]`) chamado com `this` nulo por `CHeroesGameMovement` (vtable `0x38be649c`, entrada `+0x2c`, função `0x38789650`), que lê `[jogador+0x437c]` a cada quadro (`0x387898a1`).
  - `+0x437c` é o objeto de comandos/atalhos do jogador local (0x180 bytes, `RingCommandPage_QuickSlot`), criado só em `0x388895d0`. Essa função é chamada pela preparação do jogador local `0x387033b0` (vtable `+0xac` de `C_HeroesPlayer` e de todas as classes).
  - A preparação começa com `cmp eax,0xa; jge` sobre `m_iClass` (`0x387033d7`): com a classe 10 ela é pulada inteira, e o objeto nunca é criado.
- **Os outros limites 10 do inventário** (`0x38886430`, `0x3888ce32`, `0x3888e6f6` no cliente; `0x4081efc0`, `0x40823692`, `0x408258c6` no servidor) são os acessos aos vetores de 11 posições. Eles **limitam** o índice: classe fora de 0–9 lê a linha 0 (Lethita), sem queda. `0x38630c85` compara outro valor com 10, não a classe.
- **Correção preparada, não aplicada:** `patch_class_routing.py` ganhou `GATES`, que troca o limite de `0x387033d7` por 28 (`83 f8 0a` → `83 f8 1c`), com teste. A modelo sem rosto na seleção vem dos `switch` de figura com limite 9, que caem no caso padrão (Fiona) sem `delia_figure.fig`; é visual e fica para o E1.
- **Resultado do D8b (patch com `GATES`):** a cidade carrega. Falhas restantes:
  - **`SkillActionStateMatchTable` com 10 classes** (cliente `0x3885fb60`/construtor `0x38861640`, servidor `0x407ccb8b`): mapas por classe em `this + classe*0x1c + 0x20`, só 10. A classe 10 lê o campo seguinte e cai (emote no cliente, entrada no Mercenary Outpost no servidor local). O arquivo do 1.69 já traz 13 linhas `skill_delia_*`.
  - **Personagem invisível:** os `switch` de figura/modelo com limite 9 caem no caso padrão sem `delia_figure.fig` (o mesmo da seleção).
  - **"Cannot be equipped":** os dados permitem (`ClassRestriction` 1024 = bit 10 em `bastardsword_newbie` e `delia_inner_free`); a recusa está no código de equipar do cliente (limite de classe ou tipo de arma `BASTARDSWORD`, E2–E5), ainda não localizada.
  - **Sem habilidades:** o servidor dá só `skill_delia_bastard_mastery` (escolha de laboratório); a janela de habilidades por classe não foi analisada.
- **11º mapa da `SkillActionStateMatchTable` (decisão do operador, 28/10/2026):** o objeto tem `+0x4` (mapa de habilidades), `+0x20`..`+0x11c` (10 mapas por classe, `CUtlMap` de `0x1c` bytes) e `+0x138` (mapa por nome de ação), `0x154` no total; por isso o mapa novo fica fora do objeto. `MATCH` em `patch_class_routing.py`, nos dois DLLs:
  - mapa estático da Delia na `.hrte` (agora gravável, `0xE0000020`), inicializado como o construtor deixa os outros (função de comparação `0x38022280` no cliente, `0x400ceb80` no servidor; raiz e primeiro livre `0xffff`);
  - `GetSkillNamesByActionStateName` (cliente `0x3885fb93`, servidor `0x407ccba3`): classe ≥ 10 usa o mapa da Delia; abaixo de 10, o cálculo original;
  - `MatchingActionStateSkill`: o laço de classes vai até 10 (`cmp ecx,0xa` → `0xb` em `0x38860c1d`/`0x407cdb2d`) e a classe 10 grava no mapa da Delia (`0x38860993`/`0x407cd8a3`). O laço usa a máscara `ClassRestriction` de cada habilidade (`SkillInfo+0x90`); as 19 `skill_delia_*` do 1.69 têm 1024 (bit 10), e as habilidades comuns (2047) também entram.
  - Limite conhecido: o `Reload` (comando de console) limpa só os 10 mapas originais; recarregar repete as entradas da Delia, sem queda.
  - 1ª versão caiu ao iniciar (`server.dll` `0x407cda68`): além de `ebp`, o laço guarda `&mapa->m_Elements` em `[esp+0x10]` e o relê depois de inserir (`0x38860b4a`/`0x407cda5a`). O *stub* da classe 10 passou a gravar também esse ponteiro; o laço termina depois da classe 10, então o `add [esp+0x10],0x1c` seguinte não é usado.
- **D8c passou nas quedas** (emote e Mercenary Outpost). Restam a personagem invisível, o emote travado, a recusa de equipar e a janela de habilidades vazia.
- **Modelos de animação (servidor):** `0x4040ff47` escolhe o modelo de animação por `[classe*4 + tabela]` em três tabelas de 10 posições sem limite: cidade `0x40cf1f80`, batalha `0x40cf1fa8`, *ExtremeMode* `0x40cf1fd0`. Cada uma chama `PrecacheModel` (`0x40114940`). Com a classe 10, a cidade lia a batalha da Lethita (esqueleto masculino), a batalha lia a *ExtremeMode* da Lethita e a *ExtremeMode* lia um valor que não é ponteiro (`0x3d8`). Os três arquivos da Delia existem no conteúdo (`models/player/_anim/female/delia/pc_delia_{town,battle}_motion.mdl`, `pc_delia_battle_motion_ex.mdl`). As três tabelas entraram em `TABLES` (32 posições, Delia na 10). A lista de pré-carga `0x4054de00` já tem a batalha da Delia (21ª entrada), mas o laço da cidade só percorre as 10 primeiras; se a cidade recusar a pré-carga tardia, essa lista precisa crescer.
- **Lidos e descartados como causa:** o `switch` de figura `0x388f9f2f` (só Kay, Hagie e Arisha têm `*_figure.fig`; as outras classes recebem nulo, como a Delia); `cl_playermodel` monta `models/player/<cl_playerclass>/<cl_playerclass>.mdl` (`0x385948d1`) e `models/player/delia/delia.mdl` existe.
- **Entidade (E1), formato de registro lido:** o cliente instala `player_hagie` com `EntityFactoryDictionary()->InstallFactory("player_hagie", "C_HeroesPlayerHagie", 0x4bf8, 0x38710130)` (`0x38ab5960`, inicializador estático). A classe do cliente vem da classe de rede que o servidor manda; com a entidade genérica `player` no servidor, o cliente usa `C_HeroesPlayer`.
- **D8d:** as tabelas de animação não mudaram nada visível (personagem invisível, `F7` trava).
- **"Cannot be equipped" (`engine.dll`):** o `engine.dll` lê `ItemClassInfo.ClassRestriction` (`0x422be9d0`) e desdobra a máscara em um `bool` por classe a partir de `item+0x198` (*setter* `0x42245110`, único chamador `0x422be9f8`). O vetor tem 11 posições (`+0x198`..`+0x1a2`; os construtores de cópia `0x422a96a0` e `0x422b5e90` copiam as 11, e `+0x1a3` é enchimento), mas o *setter* para no bit 9. Toda arma e roupa nega a classe 10. Como o corpo vem da *inner armor*, isso pode explicar também a invisibilidade.
  - `scripts/client/patch_engine_class_flags.py` reescreve no lugar o fim do *setter* (bits 8 e 9, `0x42245170`..`0x4224518f`, 32 bytes) para gravar também o bit 10 em `+0x1a2`, e leva a saída antecipada (`je` em `0x42245116`, máscara 0) para o `ret 4` novo. Sem seção nova e sem tocar *dword* relocado (o `.text` não tem espaço livre: maior sequência de `int3` = 16 bytes).
  - O servidor Rust já confere máscaras por `1 << classe`.
- **D8e:** a Hagie continua equipando (sem regressão); a Delia continua recusada. Os leitores do vetor também param em 9:
  - `client.dll` `0x382d4c30` (método do item, classe no argumento) e `0x388f9c80` (item e classe no argumento): `switch` de 0 a 9 que devolve `flags[classe]`; o resto devolve falso;
  - `server.dll` `0x402682e0` (método do jogador, classe em `+0x1b98`, item no argumento): o mesmo `switch`;
  - `FLAG_CHECKS` em `patch_class_routing.py`: o início de cada um salta para um *stub* que devolve `flags[classe]` para 0 a 10 e falso acima. Os bytes antigos (com o endereço relocado da tabela de salto) ficam como código morto;
  - fora do caminho de equipar: `client.dll` `0x384ac450` refaz a máscara de 10 bits a partir do vetor e `0x38410910`/`0x385b06e8` montam a lista "usável por" da dica do item; ficam como estão.
- **Janela de habilidades vazia:** o `SkillInfo` também tem `ClassRestriction`, carregado pelo `engine.dll` (`0x422bae3c`, leitura por índice de coluna), sem passar pelo *setter* dos itens. Ainda não localizado.
