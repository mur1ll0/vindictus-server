# Launcher do jogo (recurso futuro do painel)

Status: planejamento (26/10/2026). Nada implementado. Objetivo: o jogador baixa o jogo, recebe as atualizações publicadas pelo servidor baixando só o que mudou, e entra no jogo. O launcher substitui o `start-lab.bat` atual. O que não foi verificado aparece marcado como **hipótese**.

Encaixe no roteiro: o L0 (teste de arquivos soltos) pode rodar a qualquer momento, porque é só um checkpoint de cliente. Do L1 em diante, o launcher vem depois do B4 e **antes** de qualquer etapa que mude assets do cliente: ícones e modelos do [editor de itens](item-editor.md) e a classe C do [editor de fases](stage-editor.md). Ver "Recursos futuros do painel" em [playable-game-roadmap.md](../playable-game-roadmap.md).

## 1. Hoje

- O cliente de teste é a cópia `.cache/play-client` do 1.69 EN (`docs/client-compatibility.md`, "Clientes de referência").
- O `start-lab.bat` foi gerado por `scripts/client/install_lab_launcher.py` no B4.4c. Ele roda `py -3.13 scripts/client/apply_lab_content_additions.py --client-copy … --apply --edits .cache/content-state/content-edits.json`, que reaplica as edições publicadas no `sql/heroes.db3` da cópia a partir do backup original, e depois executa `start vindictus.exe -stage -dev -lang en-eu-x-gm -noupdate -threads 4`.
- Limites: exige Python e o repositório na máquina do jogador; não baixa nem repara arquivos; só altera o `heroes.db3`; não confere a versão do conteúdo do servidor.
- Login (B4.2d, `MIGRATION.md`): o cliente não envia a senha (diagnóstico B4.2-D); o frontend aceita no ID o login de uma conta ativa, sem senha no jogo.

## 2. O cliente funciona com arquivos soltos ou exige os `.hfs`?

### 2.1 Evidência

1. **Caminhos de busca:** `.cache/play-client/GameInfo.txt` declara `SearchPaths { Game |gameinfo_path|. ; Game |gameinfo_path|hl2 ; Game ..\source }`. A pasta raiz do cliente é um caminho de busca da engine Source.
2. **Como os pacotes são montados:** strings de `bin/FileSystem_Stdio.dll` (958.224 bytes, só inspeção de strings): `hfs\%s.hfs`, `hfs/%s.hfs`, `Lazy-mounted: %s for %s`, `Loading index file: %s`, `Can't load packfile: %s`, `Incompatible pack file detected!%s`, `target file don't have central dir record!%s`, `use_packfile_to_load_map`, `fs_monitorreadfrompack`. Em `bin/engine.dll` aparecem `Base_platform.hfs`, `Base_hl2.hfs` e `sql/heroes.db3`. Leitura: os `.hfs` são pacotes com diretório central (estilo zip), montados sob demanda ("lazy") a partir de um índice.
3. **Formato do `.hfs`:** em `hfs/286FE9924483F382029EF68BA6C260B3C2563BF9.hfs` (o banco SQLite dentro de HFS citado em `client-compatibility.md`), o cabeçalho é `48 46 01 02` e o fim é `48 46 05 06`. É a estrutura do zip com `PK` trocado por `HF`, método 0 (armazenado), nome de 15 bytes ilegível e conteúdo sem texto claro: nomes e dados ofuscados. As ferramentas do operador em `D:\PROJETOS\VindictusPrivate\Tools` cobrem os dois sentidos: `HFSExtract` (github.com/yretenai/HFSExtract) extrai, e `vZipFlip_HFS` (`unHFS_File.bat` / `ReHFS_File.bat`) converte HFS ↔ zip. Refazer um `.hfs` é, portanto, possível (**hipótese** até repetir com um pacote alterado e o cliente aceitar).
4. **Um pacote por pasta:** são 2.681 `.hfs` na cópia. `hfs_extracted/summary.txt` liga cada nome à pasta que ele contém (`0000AA13… -> models/armor/kingdom`). O nome **não** é o SHA-1 do caminho (testado com `models/armor/kingdom` em minúsculas e maiúsculas). A regra do nome segue desconhecida, e isso pesa para pastas **novas**.
5. **Arquivos soltos já são lidos hoje:**
   - `scaleform/` na raiz tem 453 arquivos soltos (`.gfx`, `.dds`) que não estão nos HFS de scaleform (`hfs_extracted/scaleform` tem 1.475 outros arquivos; `AchievementDialog.gfx` e `ArenaDialog.gfx` só existem soltos). As telas do jogo funcionam, logo são lidas soltas.
   - `sql/heroes.db3` está solto, e o `engine.dll` abre o caminho literal `sql/heroes.db3`. O EndPoint do laboratório só existe nessa cópia solta e foi buscado pelo cliente (`client-compatibility.md:147`), enquanto o banco de dentro do HFS (`hfs_extracted/sqlite/heroes.db3`) traz a URL oficial. Todas as edições de cliente do projeto (lojas, MaxStack, Enhance) já dependem disso.
   - `cfg/` (5 arquivos), `maps/graphs/*.ain`, `sound/`, `media/` também ficam soltos.
6. **Windictus 2023 (somente leitura):** `C:\Windictus 2023` ainda tem 3.596 `.hfs`, mais uma pasta `data/` com 25.484 arquivos soltos (`data/maps/01a.bsp`, `data/models/armor/2016_summer/…mdl`, `data/materials`, `data/scaleform`, `data/scripts`). O `GameInfo.txt` dele tem os mesmos caminhos de busca, sem `data`, e o `FileSystem_Stdio.dll` tem as mesmas strings de HFS. **Hipótese:** o `data/` é montado por binários modificados ou pelo próprio launcher deles. Esse launcher (`Windictus.exe`, .NET, 167.936 bytes) confere a integridade de cada arquivo: o `WindictusLauncherLog.txt` mostra `The following file is corrupted: nexon_api.dll / heroes_text_english_eu.txt / Windictus.exe`. O REA (`inspect_managed_artifact`) não leu os metadados: `Duplicate metadata stream #GUID` é uma proteção anti-descompilação deliberada, e o `VMProtectSDK32.dll` está na pasta.

### 2.2 Como o cliente abre um arquivo (engenharia reversa, 26/10/2026)

Com a autorização do operador, o `bin/FileSystem_Stdio.dll` da cópia (SHA-256 `8b3bf91b6ac1…`) foi analisado no REA (Ghidra 12.1.4). Funções lidas: `CBaseFileSystem::Open` (`0x1000fb10`), busca no índice (`0x10027f30`), montagem sob demanda (`0x10025f00`), leitor do índice (`0x10028990`, `0x100288b0`), hash (`0x10056380`) e abertura em diretório × pacote (`0x1000cee0`).

1. **O índice HFS vem primeiro.** Para leitura, `Open` normaliza o caminho (minúsculas, `\`) e, se o índice estiver carregado, procura nele (`0x10027f30`). Achando, monta o `.hfs` sob demanda (`hfs/%s.hfs`, mensagem `Lazy-mounted: %s for %s`) e abre o arquivo pela posição e pelo tamanho gravados no índice. Só se o caminho **não** estiver no índice, ou se o pacote falhar ao montar, `Open` percorre os caminhos de busca (pastas soltas).
2. **Prefixos bloqueados para soltos.** Se o caminho começa com `mater`, `model`, `scrip` ou `sqlit` (`materials/`, `models/`, `scripts/`, `sqlite/`), a busca de recurso só aceita caminhos de busca que são pacotes (campo `+0x10` do caminho de busca diferente de zero). Ou seja: **modelos, materiais e scripts soltos nunca são lidos**, nem como arquivo novo. Os demais (`scaleform/`, `resource/`, `maps/`, `sound/`, `sql/`, `cfg/`…) podem vir soltos quando não estiverem no índice.
3. **Pacote que falha cai para os soltos.** Se o `.hfs` listado no índice não abre (`Can't load packfile: %s`), o nó é marcado como falho e `Open` segue para os caminhos de busca. É exatamente o que quem montou o 1.69 EN fez: `hfs/RENOMEADo_9FB527B373DCBD159552ADD1710487A724237AF9.hfs` (presente também no original somente leitura) contém `AchievementDialog.gfx`, `ArenaDialog.gfx` e as outras telas do `scaleform/`; renomeado, ele não monta, e o cliente usa as 453 telas soltas da raiz.
4. **Formato do índice** (recuperado e conferido em 26/10/2026; ver 2.4): o arquivo `hfs/E608DF10F284E19AB75FD7D0D8AE9CCF87831041.hfs` tem 14 bytes claros (`48 46 01 02 0a 00` e zeros) e depois um fluxo zlib (`78 da`) cifrado com XOR pela chave de 4 bytes `78 BD 02 47`, aplicada por `posição & 3` (`EncryptedReader` → `InflateReader`, zlib 1.2.3). Descomprimido (2.236.020 bytes), traz um cabeçalho de 0x14 bytes (`HFI`, contagem de pacotes u16 em 0x04 = 2.680, contagem de arquivos u32 em 0x10 = 136.400), uma entrada de 0x14 bytes por pacote (o nome SHA-1 do `.hfs` em 5 u32 little-endian, ordenados) e uma tabela de 16 bytes por arquivo, ordenada pelo hash: hash u32 (`h = 0xAAAAAAAA; h = h*33 + tolower(c)` **só sobre o nome do arquivo**, sem a pasta), primeiro e último caractere em minúsculas, índice do pacote u16, posição e tamanho dos dados dentro do pacote. A pasta é conferida depois, contra o pacote (`0x10027cd0`).
5. **Onde fica o índice:** é o `E608DF10….hfs` (confirmado no cliente, seção 2.4). Ele não aparece no próprio índice, e o HFSExtract não o abre (`CANNOT LOAD`). A escolha do arquivo fica em `.vmp0`, mas a cifra depende só de `posição & 3` (a rotina protegida `0x10028b90` recebe esse valor), o que permitiu achar a chave por força bruta contra o cabeçalho zlib.

### 2.3 Conclusão

- **Comprovado (código):** soltos substituem o conteúdo de um HFS só quando aquele HFS deixa de montar (renomeado ou removido) **e** o caminho não começa por `materials/`, `models/`, `scripts/` ou `sqlite/`. A técnica "desativar o HFS e soltar a pasta inteira" vale para `scaleform/` (já em uso), `resource/` (textos), `maps/`, `sound/` e semelhantes. Ícones novos (`scaleform/item_icon/<nome novo>.tga`) não estão no índice e podem ir soltos sem desativar nada.
- **Comprovado (código):** `scripts/player_costume.txt`, `scripts/item_data.txt`, `models/…` e `materials/…`, sejam alterados ou novos, **só** entram por pacote. Como a posição e o tamanho de cada arquivo estão no índice, refazer um `.hfs` com conteúdo diferente exige **regravar o índice**. O índice já foi localizado, decifrado e recodificado (item 4); o L0-d mostrou que o cliente aceita um índice regravado (seção 2.4).
- **Alternativa por binário:** os `.hfs` `Base_hl2.hfs`/`Base_platform.hfs` são pacotes de caminho de busca (a busca bloqueada os aceita). **Hipótese:** um pacote HF a mais registrado como caminho de busca (o que o Windictus parece fazer com `data/`) passaria nos prefixos bloqueados; exigiria mudar o `GameInfo.txt` ou o binário, e precisa de teste.
- **O motor exige os HFS?** Sim: tudo que está no índice é lido dos pacotes. O launcher distribui os HFS originais como arquivos opacos e acrescenta os soltos; o que for `models/`/`scripts/`/`materials/` vai em pacote.

### 2.4 Testes de cliente (L0, checkpoint antes de implementar)

Fazer sempre na cópia de laboratório, nunca no original.

| Teste | Passo | Resultado que decide |
|---|---|---|
| L0-a índice | abrir o jogo com `-dev`, abrir o console e procurar `Loading index file:` | caminho do índice → permite regravá-lo (item 4) |
| L0-b texto solto | desativar (renomear) o HFS de `resource` e soltar a pasta `resource/` extraída com um nome de item alterado | o nome muda → textos editáveis soltos |
| L0-c ícone novo | `scaleform/item_icon/<nome novo>.tga` solto e `ItemClassInfo.Icon` apontando para ele | o ícone aparece → ícones novos sem pacote |
| L0-d índice regravado | com o caminho do L0-a, gerar um índice com um `.hfs` novo contendo um `player_costume.txt` alterado | o cliente aceita → modelos e scripts novos viáveis; se recusar, a proteção confere o índice |

**Resultados de 26/10/2026** (script `.cache/play-client/l0-test.ps1`, com `prepare`, `start`, `report` e `restore`; o cliente foi restaurado depois):

- **L0-b passou.** Com os pacotes `0FABE22A…` (`localized_text/english`) e `5A7E0CE7…` (`localized_text/english_eu`) renomeados, os textos soltos em `resource/localized_text/<idioma>/heroes_text_<idioma>.txt` (UTF-16 LE com BOM) foram lidos. O título "Select Character" virou `L0B-OK`, e o cliente abriu e logou normalmente. O botão "Start Game" não mudou: a tela usa outra chave ou um texto gravado no `.gfx` (**hipótese**). Textos são, portanto, editáveis sem pacote.
- **L0-a falhou.** Com `-dev -console -condebug`, nenhuma tecla abre o console e nenhum `console.log` é gravado. O caminho do índice segue desconhecido.
- **Candidato a índice (análise estática):** `hfs/E608DF10F284E19AB75FD7D0D8AE9CCF87831041.hfs` (1.748.398 bytes). O HFSExtract não o abre (`CANNOT LOAD`). Ele tem um único registro `HF 01 02`, sem o fim `HF 05 06`, e é isso que o leitor do índice (`0x10028990`) espera: lê 0x0e bytes, confere `0x02014648` e decifra o resto. O tamanho comporta cerca de 105 mil entradas de 16 bytes. **Hipótese** até decifrar.
- **Cifra:** o leitor usa `EncryptedReader` (vtable `0x100986a4`, leitura em `0x10020420`), que faz XOR com `0x10028b90(posição & 3)`, rotina em `.vmp0`. Como só há quatro valores, a chave é `78 BD 02 47`; o fluxo decifrado é zlib (`InflateReader`, `inflateInit2_(…, 47, "1.2.3")`). A primeira tentativa comparou com a cifra dos pacotes de dados, que é outra e muda por arquivo.
- **Leitura da memória (autorizada pelo operador):** com o cliente aberto como administrador, a tabela decifrada foi achada na memória pelo hash de três nomes conhecidos (136.400 entradas). Isso revelou a compressão e a regra do hash só sobre o nome.
- **Índice confirmado (L0-a', 26/10/2026):** com `E608DF10F284E19AB75FD7D0D8AE9CCF87831041.hfs` renomeado (`l0-test.ps1 index-off`), o cliente parou na abertura com `Can't find background image 'materials/console/s3_game_start_widescreen.vtf'` e fechou. O log em `Documents` termina em `DrawStartupGraphic::PlayStartupVideos`. Sem o índice, nada de dentro dos `.hfs` é achado, e `materials/` não aceita soltos. Logo, **esse arquivo é o índice**; o pacote foi restaurado (`index-on`). Falta para o L0-d a cifra dele: a chave por arquivo vem do código em `.vmp0`. Resolvido depois: a cifra e o formato estão no item 4 da seção 2.2.

- **Codec e L0-d preparado:** `.cache/play-client/l0-hfs-index.py` decodifica e recodifica o índice. Recodificar sem mudanças gera um arquivo **idêntico byte a byte** ao original (zlib nível 9), e o comando `drop` remove entradas pelo nome. O `l0-test.ps1 index-edit` regrava o índice sem as entradas de `heroes_text_english.txt` e `heroes_text_english_eu.txt`, sem renomear nenhum pacote, e solta os textos com `L0B-OK`; `index-original` desfaz. Se o cliente aceitar e mostrar `L0B-OK`, o código protegido não confere a integridade do índice. Falta ainda a cifra dos **dados** dos pacotes, que é outra e muda por arquivo, para criar `.hfs` novos (o `vZipFlip_HFS` do operador já faz a conversão).

- **L0-d passou (26/10/2026).** Com o índice regravado pelo `index-edit` e nenhum pacote renomeado, o cliente abriu e mostrou `L0B-OK` no lugar de "Select Character". O código protegido **não** confere a integridade do índice; o cliente segue o índice que recebe. O operador restaurou com `index-original`, e o índice da cópia voltou a ter o SHA-256 do original (`c63a8a4531bdf2ab…`). Consequências:
  - tirar entradas do índice libera qualquer caminho fora dos prefixos bloqueados para arquivo solto, sem desativar pacotes inteiros;
  - acrescentar pacotes e apontar entradas para eles fica viável no índice. Falta a cifra dos **dados** dos pacotes para gerar `.hfs` novos com `models/`, `materials/` e `scripts/` (próximo passo: decifrar um pacote conhecido ou validar o `vZipFlip_HFS`).

- **Cifra dos pacotes (26/10/2026):** o `.hfs` é um zip armazenado com as assinaturas `HF 01 02` (cabeçalho local e central) e `HF 05 06` (fim). Os campos dos cabeçalhos ficam claros, e o CRC-32 é dos dados claros. Nomes e dados levam XOR com uma tabela fixa de 4.096 bytes indexada pela **posição absoluta no arquivo**. Ela foi tirada de dois pacotes de texto conhecidos (as duas sequências coincidem, só deslocadas pela diferença de tamanho dos nomes). Começa por `78 BD 02 47`: o índice usa só esses 4 bytes (`posição & 3`). O atributo externo do arquivo é 0x20 ou 0x80 conforme o pacote. Os scripts ficam como `x.txt.comp` (`comp`, u32 tamanho original, zlib), e o índice os lista sem o `.comp`.
- **Codec de pacote:** `.cache/play-client/l0-hfs-pack.py` (`list`, `extract`, `build`, `roundtrip`; a tabela fica em `l0-hfs-keystream.bin`, derivada do conteúdo do jogo e não versionada). A leitura confere o CRC de cada arquivo, e **42 pacotes regravados ficaram idênticos byte a byte** (amostra de 40 mais os de scripts e textos). O `l0-hfs-index.py sync` aponta o índice para um pacote refeito: com quatro pacotes sem mudança, ele reproduz o índice original byte a byte.
- **Regra do nome do pacote (parcial):** ao abrir um arquivo, o cliente confere se o pacote listado no índice é o da pasta do arquivo (`0x10027cd0`). O texto da pasta é montado por `0x10027490` (separadores viram `@`, caixa alternada: `MoDeLs@aRmOr@kInGdOm`) e passa por um hash de 20 bytes em `.vmp0` (`0x10025460`), impresso com `%08X` ×5. Não é SHA-1, SHA-0, RIPEMD-160 nem SHA-256 desse texto (testado). Consequência: **pastas existentes** podem receber arquivos novos ou alterados refazendo o pacote com o mesmo nome (os nomes estão no `summary.txt`). **Pastas novas** dependem de descobrir esse hash.
- **L0-e preparado:** o `l0-test.ps1 pack-edit` refaz os pacotes `0FABE22A…` e `5A7E0CE7…` com o texto marcado `L0E-OK` e um arquivo novo (`l0_probe.txt`) e sincroniza o índice, sem nenhum arquivo solto; `pack-original` desfaz. Rodado e conferido fora do cliente em 26/10/2026.
- **L0-e passou (26/10/2026).** Com os dois pacotes de texto refeitos pelo codec (um deles com um arquivo novo) e o índice sincronizado, sem nenhum arquivo solto, o cliente abriu e mostrou `L0E-OK` em "Select Character". Ficou comprovado o caminho completo: **gerar `.hfs` com a cifra recuperada e apontar o índice para eles**. O primeiro `pack-original` do operador não restaurou (provavelmente o cliente ainda fechava); rodado de novo, o índice e os dois pacotes voltaram a ter o SHA-256 do 1.69 EN. O que falta para conteúdo novo: (1) o hash do nome de pasta, para pastas novas; (2) testar um `models/…` ou `scripts/…` alterado num pacote refeito (prefixos que só aceitam pacote; por exemplo, `player_costume.txt.comp` no pacote `763FD8F1…`).

Com o L0-d aprovado, o launcher distribui `.hfs` novos e o índice regravado, como o pw-universal regrava entradas do `.pck`. Se falhar, modelos e scripts novos dependem de mudar o binário ou o `GameInfo.txt` (seção 2.3), uma decisão à parte.

## 3. Referência: pw-universal-server (projeto do operador)

Lido em `F:\Python_C_Projects\PWSource1.5.3\pw-universal-server` (somente leitura): `crates/pw-atualizador/src/manifesto.rs`, `cliente.rs`, `assinatura.rs`, `tools/pw-launcher/src/main.rs` e `docs/admin/RECURSOS_VERSIONAMENTO_E_ATUALIZADOR.md` §4, §8.6–§8.8.

- **Servidor (HTTP estático):** `<versao>/atualizacao/versao.json` (+ `.sig`) aponta a versão atual e a mínima; `<n>/manifesto.json` (+ `.sig`) traz o estado completo da versão `n` (pacotes `{nome, sha256, tamanho}` e arquivos `{caminho, sha256, tamanho, pacote}`); `<n>/pacotes/<pasta>.tar.zst` é a instalação, um pacote por pasta com zstd nível 12; `objetos/<sha256>` guarda os arquivos mudados nas versões seguintes. A versão N reaproveita os pacotes da 0.
- **Assinatura:** Ed25519 sobre os bytes exatos do `.json`. A chave pública vai embutida no launcher, e a chave privada nasce no serviço, num volume só dele.
- **Launcher:** Rust, Win32 nativo. Guarda `.pw-atualizacao/estado.json`, `manifesto.json`, `<pacote>.parcial` (retomado com `Range`) e `objetos/<sha>`. Instalar, verificar/reparar (confere todos os arquivos com o manifesto completo), atualizar e jogar, tudo retomável.
- **Governança:** rascunho → itens por caminho → publicar N+1 assinado → reverter, pela página "Recursos do cliente" do painel.
- Números medidos lá (1.2.6: 61.234 arquivos, 4,7 GB, 26 pacotes de 2,80 GB) ajudam a dimensionar, mas o Vindictus precisa medir o próprio.

## 4. Modelo proposto para o Vindictus

### 4.1 Servidor

- Novo módulo (ex.: `client_distribution`) no monólito: domínio (manifesto, versão, regras de caminho), infraestrutura (armazenamento por SHA-256 num volume próprio, assinatura) e HTTP só de leitura para os arquivos, separado do HTTP operacional do painel.
- **Versão 0 (importação):** a instalação 1.69 EN original (`Vindictus1.69EN/Vindictus EU/en-EU`, somente leitura) é importada **sem** os arquivos que não servem: `hfs_extracted/` (só referência), logs (`npp_api_*.log`, `fmod.log`), `*.bak` e backups `heroes.db3.*`, `BlackCipher`, `NMService.exe`, `_eu_AdBalloonExt.exe` e outros instaladores ou serviços Nexon sem uso no laboratório (lista final no L1, decisão do operador).
- **Pacotes:** `hfs/` é a maior parte e muda raramente. Proposta: pacotes `.tar.zst` por grupo (por exemplo `hfs-0` … `hfs-N` por faixa de nome, `bin`, `scaleform`, `raiz`), porque 2.681 pacotes de uma pasta só seriam pequenos demais. Os HFS já são armazenados ofuscados e comprimem pouco (**hipótese**; medir no L1).
- **Objetos por versão de conteúdo:** cada versão publicada do painel (B4.4c) que toca o cliente gera uma versão do cliente com:
  - `sql/heroes.db3` completo, gerado no servidor a partir do original + `lab_content_additions.json` + sobreposição publicada (a mesma lógica de `apply_lab_content_additions.py`, migrada para o servidor). Tem 18.198.528 bytes; com zstd e baixado só quando muda, é aceitável. Delta binário (`zstd --patch-from`) é uma otimização posterior;
  - arquivos soltos novos ou alterados fora dos prefixos bloqueados (ícones novos, textos de `resource/` com o HFS dele desativado, mapas) e, depois do L0-d, `.hfs` novos com o índice regravado para `models/`, `materials/` e `scripts/` (seção 2.3);
  - o `EndPoint` certo para o servidor em `FeatureMatrix.EndPoints`, gravado no `heroes.db3` publicado, em vez do preparador manual (`prepare_lab_client.py`).
- **Ligação com a versão de conteúdo do servidor:** o `versao.json` leva a versão de conteúdo e a `minima`. A publicação com manutenção (B4.4c) só termina quando o pacote do cliente da mesma versão existir. **Hipótese:** o protocolo não leva a versão de conteúdo; quem impede jogar com conteúdo velho é o launcher (versão mínima). Uma checagem no servidor exigiria um campo novo, fora do protocolo recuperado.

### 4.2 Launcher (Windows)

- Rust (o mesmo idioma do servidor, binário único), reaproveitando o desenho do `pw-atualizador`: estado local numa pasta oculta, por exemplo `.vindictus-atualizacao/`; download retomável; conferência por SHA-256; reparar; assinatura Ed25519 conferida antes de usar qualquer manifesto.
- **Telas:** primeira instalação (escolher pasta, baixar e extrair em fluxo), atualização detectada ao abrir, "Verificar arquivos", notícias e estado do servidor (manutenção agendada do B4.4c), "Jogar".
- **Jogar:** executa `vindictus.exe -stage -dev -lang en-eu-x-gm -noupdate -threads 4`, os argumentos atuais do `start.bat`. Revisar `-dev` e `-stage`, que o `client-compatibility.md` liga a hipóteses de modo desenvolvedor.
- **Login no launcher (sugestão):** hoje o jogo entra sem senha (B4.2d). O launcher pode pedir login e senha da conta (o mesmo do painel), obter do servidor um token de uso único e entregá-lo ao cliente. Pelo plano do B4 (`docs/b4-painel-plano.md` §1.3) o canal seria o `-passport`, já que o cliente envia um campo passport (5 bytes no diagnóstico B4.2-D). **Hipótese:** precisa do teste T1/T2 do plano do B4 para saber se o cliente repassa um passport vindo da linha de comando.
- **Segurança:** nenhum caminho fora da pasta do jogo (recusar `..`, absolutos e links); limites de tamanho por manifesto e objeto; nunca executar arquivo baixado sem hash conferido; a autoatualização do launcher também é assinada; sem credenciais em log.

### 4.3 Substituir o `start-lab.bat`

| Hoje (`start-lab.bat`) | Com o launcher |
|---|---|
| Python e repositório na máquina | só o launcher |
| `apply_lab_content_additions.py` reconstrói o ShopInfo da cópia a partir do backup | o servidor gera o `heroes.db3` da versão; o launcher baixa só quando o hash muda |
| lê `.cache/content-state/content-edits.json` local | lê `versao.json` assinado pela HTTP |
| sem conferência de versão | recusa jogar abaixo da `minima` e mostra a manutenção |
| só o `heroes.db3` | qualquer arquivo (soltos, HFS refeitos) |

Durante a transição, o `start-lab.bat` continua como alternativa de laboratório, como o `apply-content-edits.ps1` continuou depois do B4.4c.

## 5. Riscos e decisões do operador

- Distribuir o cliente original é distribuir assets proprietários: o servidor de arquivos é local ou privado, e nada disso entra no Git (AGENTS.md).
- O índice é lido por código protegido com VMProtect; regravá-lo pode ser recusado por uma checagem que não se vê estaticamente (L0-d decide). O HFSExtract é ferramenta de terceiros: usar só para leitura até o L0-d.
- Antivírus e SmartScreen com executável novo sem assinatura de código: decidir se o operador assina o launcher.
- Decisões: pacotes por grupo × por pasta; o que sai da versão 0; se o launcher faz login (depende de T1/T2 do B4); onde fica a chave privada.

## 6. Engenharia reversa com REA (pedido do operador, 26/10/2026)

- `inspect_managed_artifact` no `Windictus.exe`: a proteção dos metadados .NET impede ler o launcher do Windictus; o modelo dele fica só pelo log de integridade.
- `FileSystem_Stdio.dll` (autorizado pelo operador em 26/10/2026): resultado na seção 2.2. Limite: a parte em `.vmp0` (VMProtect), que escolhe o arquivo do índice e monta a lista de pacotes, não é legível estaticamente. O L0-a (console) falhou; o candidato a índice e a cifra estão na seção 2.4.

## 7. Etapas propostas

| Etapa | Entrega | Cliente |
|---|---|---|
| L0 | testes L0-a…d na cópia de laboratório (seção 2.4) | checkpoint de cliente |
| L1 | importação da versão 0, pacotes, manifesto assinado e HTTP de arquivos (com medição de tamanho e tempo) | não necessário |
| L2 | launcher: instalar, verificar, reparar e jogar | instalar numa pasta limpa e jogar |
| L3 | versão de cliente gerada a cada publicação de conteúdo que toca o cliente (`heroes.db3` gerado no servidor, EndPoint), no lugar do `start-lab.bat` | preço novo de loja visto depois de atualizar pelo launcher |
| L4 | autoatualização do launcher, notícias, manutenção, login com token (se T1/T2 permitir) | — |
