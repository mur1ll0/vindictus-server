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

### 2.2 Conclusão

- **Comprovado:** o cliente 1.69 lê arquivos soltos da raiz para caminhos que **não** estão em nenhum HFS (scaleform, `sql/heroes.db3`, cfg). Arquivos novos, como ícones, modelos e scripts em caminhos novos, podem ser distribuídos soltos.
- **Não comprovado:** se um arquivo solto **sobrepõe** o mesmo caminho dentro de um HFS (por exemplo `scripts/player_costume.txt` ou `scaleform/item_icon/<x>.tga`). A ordem padrão da Source varia por versão, e o "lazy mount" por índice pode até ignorar o solto.
- **O motor exige os HFS?** Sim para o que só existe neles. A cópia sem a pasta `hfs/` não foi testada e não é necessária: o launcher distribui os HFS originais como arquivos opacos e acrescenta os soltos.

### 2.3 Testes de cliente (L0, checkpoint antes de implementar)

Fazer sempre na cópia de laboratório, nunca no original.

| Teste | Passo | Resultado que decide |
|---|---|---|
| L0-a sobreposição | copiar `scaleform/item_icon/<ícone do Elixir>.tga` (hoje dentro de HFS) solto na raiz, com outra imagem | o ícone muda na loja → soltos sobrepõem HFS |
| L0-b script | `scripts/useitem.txt` solto com uma entrada alterada | o comportamento muda → scripts podem ser editados soltos |
| L0-c modelo novo | uma pasta nova `models/player/fiona/…mdl` solta (cópia renomeada de um modelo existente) referenciada por uma entrada nova de `player_costume.txt` | o personagem veste → editor de itens sem refazer HFS |
| L0-d HFS refeito | extrair, alterar e refazer um `.hfs` com o `vZipFlip` | o cliente carrega → plano B, se L0-a falhar |

Se o L0-a falhar, o launcher passa a regravar o arquivo dentro do HFS refeito (plano B), como o pw-universal faz com o `.pck`.

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
  - arquivos soltos novos ou alterados (ícones, modelos, scripts e textos do editor de itens e do editor de fases);
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
- O formato e o nome do HFS não estão documentados no projeto, e o HFSExtract é ferramenta de terceiros. Usar só para leitura até o L0-d.
- Antivírus e SmartScreen com executável novo sem assinatura de código: decidir se o operador assina o launcher.
- Decisões: pacotes por grupo × por pasta; o que sai da versão 0; se o launcher faz login (depende de T1/T2 do B4); onde fica a chave privada.

## 6. Engenharia reversa com REA (pedido do operador, 26/10/2026)

- **Feito nesta etapa:** `inspect_managed_artifact` no `Windictus.exe`. A proteção dos metadados .NET impede ler o launcher do Windictus; o modelo dele fica só pelo log de integridade.
- **Útil e pontual (precisa de autorização):** no `FileSystem_Stdio.dll` (958 KB, pequeno, mas uma DLL do cliente, coberta pela regra de 24/10 contra análise completa), descompilar só as funções que referenciam `Lazy-mounted: %s for %s` e `Loading index file: %s`. Isso mostraria a ordem solto × HFS, de onde vem o índice e como o nome do pacote é calculado, e permitiria criar pacotes `.hfs` para pastas novas. O teste L0 de cliente responde à mesma pergunta sem REA e deve vir primeiro.

## 7. Etapas propostas

| Etapa | Entrega | Cliente |
|---|---|---|
| L0 | testes L0-a…d na cópia de laboratório | checkpoint de cliente |
| L1 | importação da versão 0, pacotes, manifesto assinado e HTTP de arquivos (com medição de tamanho e tempo) | não necessário |
| L2 | launcher: instalar, verificar, reparar e jogar | instalar numa pasta limpa e jogar |
| L3 | versão de cliente gerada a cada publicação de conteúdo que toca o cliente (`heroes.db3` gerado no servidor, EndPoint), no lugar do `start-lab.bat` | preço novo de loja visto depois de atualizar pelo launcher |
| L4 | autoatualização do launcher, notícias, manutenção, login com token (se T1/T2 permitir) | — |
