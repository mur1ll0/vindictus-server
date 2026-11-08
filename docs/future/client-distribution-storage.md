# Armazenamento e entrega do cliente 1.69 (servidor de atualização e launcher)

Medições de 02/11/2026 para a seção 4.3 de `docs/future/game-launcher.md`. **Decisão do operador (02/11/2026): zstd adaptativo com o XOR desfeito, um objeto por arquivo**, por ser o menor tempo de download somado à descompressão até 300 Mbit/s (4.6) e o mais barato de publicar (4.7); aplicada em `game-launcher.md` 4.3. Os arquivos do jogo não precisam ser escondidos nem cifrados: o servidor guarda e entrega só o que garante **integridade** (SHA-256 de cada arquivo final) e **autenticidade** (manifesto assinado com Ed25519). Todos os números abaixo foram medidos. Onde há extrapolação de uma amostra para o cliente inteiro, a tabela diz.

## 1. Resposta curta

| Pergunta | Recomendação | Número que decide |
|---|---|---|
| `.hfs` como estão (sem compressão)? | não: 37 % mais download e espaço, e mais lento até 300 Mbit/s mesmo somando a descompressão (4.6) | 100 Mbit/s, download + descompressão: 20,2 min × 15,1 min |
| Formato | **zstd adaptativo (nível 3 onde já não comprime, 19 no resto), um objeto por arquivo**, com checksum de quadro; sem `--long` | v3: 15.044 MB → **10.937 MB** (zstd 19 com XOR desfeito); `.tar.gz` sobre os arquivos como estão: 13.891 MB |
| Granularidade | **objeto por arquivo, endereçado pelo SHA-256 do arquivo final**; sem pacotes `.tar` de instalação | pacote sólido por tipo ganha no máximo 1,2 ponto (mapas) e no total ≈ 0,4 % |
| XOR dos `.hfs` | **desfazer o XOR do arquivo inteiro** (`transform: hfs-xor`) em todos os `.hfs`, inclusive `Base_hl2.hfs` | mapas 57,6 % → 27,8 %; shaders 42,9 % → 0,8 %; resource 46,8 % → 7,1 %; pacote refeito: delta de 14,1 MB → 187 KB |
| Atualizações | delta `--patch-from` sobre a forma clara quando houver o SHA antigo e o delta < 50 % do objeto; objeto inteiro para arquivos novos; nunca reconsolidar pacotes | v1→v2: ≈ 7 MB com delta × 41 MB inteiro; v2→v3: ≈ 1.816 MB (quase tudo pacote novo, que não tem delta) |
| Fase 2 (opcional) | `hfs-comp`: inflar os `.comp` (zlib) dentro do `.hfs`; o launcher deflaciona de novo bit a bit | v3: 10.937 → ≈ 10.026 MB (−8,3 %); Miri: 1.813 → 1.547 MB; delta de um `.mdl.comp` trocado: 3,6 MB → 72 KB |

Publicar uma versão do tamanho da Miri: 6 s de compressão e 1.824 MB enviados ao servidor (4.7). Tamanhos esperados com a recomendação (fase 1): **servidor ≈ 11,0 GB** para v1+v2+v3; **instalar a v3: 10.937 MB** de download; **atualizar v1→v2: ≈ 7 MB; v2→v3: ≈ 1.816 MB** (seção 6).

## 2. Metodologia

- **Máquina:** AMD Ryzen 7 5800X (16 threads lógicos), Windows 10 de 64 bits, Python 3.13.14, `zlib` 1.3.1, `lzma` (liblzma do Python), `zstandard` 0.25.0 (libzstd 1.5.7) instalado com `pip --target` numa pasta temporária fora do repositório. Não havia `zstd` nem `7z` na linha de comando; `xz`, `gzip` e `tar` existem no Git Bash, mas as medições usaram os módulos Python para que tudo rodasse no mesmo processo e com a mesma contagem de tempo.
- **Carga:** um job pesado por vez, um núcleo por medição (compressão de thread única). Os tempos são de compressão e descompressão em memória (sem disco), com ida e volta conferida byte a byte em todas as 484 medições.
- **Fontes, só leitura:** cópia de laboratório `.cache/play-client` (= v3) e original `Vindictus1.69EN/Vindictus EU/en-EU` (≈ v1). Nada foi gravado nelas. Os únicos arquivos intermediários (medição de memória) ficaram na pasta temporária da sessão e foram apagados.
- **Conjunto instalável:** a árvore sem `hfs_extracted/`, `lab-backup/` e a lista de exclusão de 4.3.6 (`*.bak`, `*.pre-*`, `l0-*`, logs, `passport/`, `BlackCipher/`, `downloads/`, `DownloadLists/`, `__pycache__/`, executáveis da Nexon). v1: 3.428 arquivos, 13.205 MB. v3: 3.613 arquivos, 15.044 MB.
- **Amostra:** 44 arquivos, 970 MB. Para cada tipo de pasta dos `.hfs` (pelo `hfs_extracted/summary.txt`; os 137 pacotes novos pela lista `lab-backup/added-packs.json`), foram escolhidos arquivos nos quantis do tamanho acumulado, para representar os bytes e não só a contagem: mapas 6 (196 MB), materiais 7 (32 MB), modelos 7 (36 MB), scaleform 4 (248 MB), pacotes novos de classe 4 (190 MB), som 3 (13 MB), shaders 1 (88 MB), resource 2 (67 MB), scripts 2, sqlite 1; soltos: 2 `.wav`, `client.dll`, `engine.dll`, `sql/heroes.db3`, `Vindictus.exe`, 1 vídeo de `media/`. `Base_hl2.hfs` (144 MB) e `Base_platform.hfs` da raiz foram medidos à parte.
- **Variantes:** `como está` (bytes do disco); `hfs-xor` (XOR com a tabela de 4.096 bytes desfeito no arquivo inteiro, `xor_at(tabela, 0, arquivo)`); `hfs-xor seletivo` (só nomes e dados, cabeçalhos intactos); `.comp inflado` (cada entrada `comp` do pacote descomprimida).
- **Extrapolação:** a razão comprimido/original de cada tipo, medida na amostra, multiplicada pelo tamanho total do tipo no inventário (`inventory-lab.json`); tipos pequenos sem amostra (fontes, efeitos etc., ≈ 60 MB) usam a média dos `.hfs`; `bip/` usa a média dos soltos.
- **Versões:** SHA-256 de todos os `.hfs` das duas árvores. Deltas medidos entre o arquivo do original e o do laboratório e, para o `client.dll`, entre os backups `pre-hcls` → `pre-miri` → atual.
- **Logs** (fora do Git) em `.cache/verification/client-storage/`: `inventory-lab.json`, `diff-orig-lab.json`, `hfs-sha256.json`, `sample.json`, `measure.jsonl`, `solid.jsonl`, `comp.jsonl`/`comp.log`, `comp-inflate.log`, `delta.jsonl`, `delta2.jsonl`, `mem.log`, `base-hfs.log`, `aggregate-1.txt`, `extrap.txt`. Os scripts de medição ficaram na pasta temporária da sessão; a lógica está descrita aqui.

## 3. O que muda entre as versões (SHA-256)

- `.hfs`: 2.666 iguais; **15 alterados** (91,8 MB, inclusive o índice `E608DF10….hfs` e um pacote de mesmo tamanho, `7B71CFAE…`, que só o hash revelou); **137 novos** (1.829 MB, 132 deles de classe, listados em `class-packs.json`). Nenhum `.hfs` tem conteúdo duplicado em nenhuma das árvores.
- Fora do `hfs/`: 58 arquivos novos ou alterados, 64,7 MB: os três DLLs (48 MB), `sql/heroes.db3` (18 MB) e ≈ 1 MB de `cfg/`, `scaleform/`, `bin/class-routing.json` e `bin/config_material.txt`. `maps/graphs/*.ain`, `demoheader.tmp` e `SAVE/hidedate.hdf` são gerados pelo jogo e devem entrar na exclusão ou em `preservar`.
- **Divisão v1→v2→v3:** a cópia de laboratório só tem o estado final. A divisão abaixo é **suposta**: v2 (Delia) = os 14 pacotes refeitos, o índice, `heroes.db3` e os DLLs; v3 (Miri) = os 137 pacotes novos, o índice, `heroes.db3` e os DLLs de novo. Os backups do `client.dll` confirmam que cada passo muda o DLL só alguns KB.

## 4. Medições

### 4.1 Formato e nível: cliente inteiro (extrapolado da amostra)

| Cenário | v1 (13.205 MB) | v3 (15.044 MB) | Compressão (MB/s, 1 thread) | Descompressão (MB/s) | Memória para descomprimir |
|---|---|---|---|---|---|
| `.tar.gz` como está (gzip 6) | 12.056 (91,3 %) | 13.891 (92,3 %) | 33,9 | 597 | 0,3 MB |
| zstd 19 como está | 10.987 (83,2 %) | 12.820 (85,2 %) | 4,5 | 1.274 | 9 MB |
| gzip 6 + `hfs-xor` | 9.633 (72,9 %) | 11.453 (76,1 %) | 28,7 | 365 | 0,3 MB |
| xz 6 + `hfs-xor` | 8.982 (68,0 %) | 10.797 (71,8 %) | 3,5 | **51** | 8,8 MB |
| zstd 3 + `hfs-xor` | 9.581 (72,6 %) | 11.408 (75,8 %) | 334 | 1.242 | não medida |
| zstd 12 + `hfs-xor` | 9.415 (71,3 %) | 11.240 (74,7 %) | 41,7 | 1.058 | 4,9 MB |
| **zstd 19 + `hfs-xor`** | **9.121** (69,1 %)¹ | **10.937** (72,7 %)¹ | 3,9 | 611 | **9,1 MB** |
| zstd 19 `--long=27` + `hfs-xor` | 9.171 (69,4 %) | 10.986 (73,0 %) | 3,3 | 654 | **100 MB**² |
| zstd 19 + `hfs-xor` + `.comp` inflado (fase 2) | ≈ 8.477¹ | ≈ 10.026¹ | 3,9 + inflar | 611 + deflate³ | 9,1 MB |

¹ Com a correção de `Base_hl2.hfs` (144 MB, medido à parte: 47,8 % como está, 23,0 % com o XOR desfeito), que a extrapolação genérica tratava como os executáveis da raiz (60 %): −54 MB. As outras linhas não têm essa correção.
² Pico de memória do processo medido (Windows, `PeakWorkingSetSize`) ao descomprimir em fluxo um pacote de 95 MB: a janela fica do tamanho do arquivo. Num `.tar` de 256 MB seriam 128 MB. As demais memórias da coluna também são picos medidos no mesmo pacote (zstd 3 não foi medido).
³ Deflate zlib nível 6 medido a 28,7 MB/s por thread; ver 4.3.

Leitura:
- **`.tar.gz` sobre os arquivos como estão quase não comprime** (−7,7 %), porque 93 % dos bytes são `.hfs` com XOR. O problema não é o gzip: desfeito o XOR, ele chega a 76 %.
- **O nível pesa pouco:** do zstd 3 ao 19 são 417 MB (3,7 %). Materiais, modelos e os pacotes de classe são `.comp` (zlib) e não comprimem de novo; o ganho do nível está em mapas, scaleform e som.
- **xz 6 dá 194 MB (1,8 %) a menos que o zstd 19, mas descomprime 12× mais devagar** (51 × 611 MB/s nesta máquina): 15 GB levam ≈ 5 min de CPU aqui, e mais num Windows 7 de 32 bits. Não compensa.
- **`--long=27` quase não ganha por arquivo** (5 MB no cliente inteiro) e multiplica a memória por 11. Só faria sentido em fluxo sólido (4.4), que também não ganha.
- **zstd 19 custa ≈ 52 min de CPU de thread única para comprimir os 15 GB** uma vez, na importação (extrapolado por tipo; ≈ 26 min com 2 threads, um objeto por thread). O zstd 12 faria isso em 4,6 min, com ≈ 250 MB (2,3 %) a mais para todo jogador baixar. O nível adaptativo (4.7) fica no meio: 37 min e 18 MB a mais que o 19.

### 4.2 XOR desfeito, por tipo de pasta (zstd 19, amostra)

| Tipo (tamanho na v3) | Como está | `hfs-xor` | Bytes em `.comp` |
|---|---|---|---|
| maps (3.470 MB) | 57,6 % | **27,8 %** | 2 % |
| materials (3.187 MB) | 100,0 % | 99,9 % | 100 % |
| models (2.281 MB) | 99,9 % | 94,4 % | 100 % |
| scaleform (2.025 MB) | 99,9 % | 84,2 % | 2 % |
| pacotes novos de classe (1.829 MB) | 100,0 % | 99,1 % | 100 % |
| sound (684 MB) | 86,6 % | **66,0 %** | 0 % |
| shaders (297 MB) | 42,9 % | **0,8 %** | 0 % |
| resource (140 MB) | 46,8 % | **7,1 %** | 0 % |
| `Base_hl2.hfs` (144 MB, raiz) | 47,8 % | **23,0 %** | — |
| scripts, sqlite (≈ 4 MB) | 99–100 % | 98–99 % | 100 % |

- Vale para mapas, shaders, resource, som, scaleform e `Base_hl2.hfs`; é neutro nos tipos `.comp`. Como o custo é o mesmo XOR em qualquer arquivo e nunca piora, **aplicar a todos os `.hfs`** (exceto o índice `E608DF10…`, que usa outra cifra e é zlib; vai como está) é mais simples que uma regra por pasta.
- **XOR do arquivo inteiro × seletivo:** 142,04 × 142,00 MB nos mesmos 28 pacotes (279 MB). Os cabeçalhos são insignificantes. Fica o do arquivo inteiro, que não precisa analisar o pacote: o launcher só aplica `xor_at(tabela, 0, dados)` em fluxo.
- Soltos (como estão): DLLs 30,9 %, `heroes.db3` 14,6 %, `.wav` 54,8 %, vídeos 96,7 %, `Vindictus.exe` 60,2 %.

### 4.3 `.comp` inflado (fase 2)

- Os 1.392 `.comp` da amostra (materiais 213, modelos 821, pacotes de classe 102, scripts 241, scaleform 11, shaders 2, mapas 1, sqlite 1) **recomprimem byte a byte** com o zlib 1.3.1 do Python: 1.371 no nível 6 (conteúdo original da Nexon, que usava zlib 1.2.3) e 21 no nível 9 (scripts refeitos pelo projeto com `comp_wrap`).
- zstd 19 sobre a forma inflada × sobre o zlib guardado: modelos 37,97 → 30,35 MB (−20 %), pacotes de classe 197,1 → 168,2 MB (−15 %), materiais 33,8 → 31,9 MB (−6 %), sqlite 3,28 → 2,44 MB. No cliente inteiro: 10.937 → ≈ 10.026 MB.
- Custo no launcher: deflacionar de novo ≈ 14,5 GB de dado inflado (materiais ×1,48, modelos ×1,75, pacotes de classe ×3,2) a 28,7 MB/s por thread, ≈ 8–9 min de CPU numa thread aqui. Exige um deflate **idêntico** ao zlib (por exemplo `libz-sys` estático com zlib, e não zlib-ng nem `miniz_oxide`), o nível de cada entrada no manifesto e, se o SHA-256 final não bater, recurso ao objeto `hfs-xor`. O mesmo mecanismo permitiria delta sobre a forma clara do índice (316 KB × 1,7 MB inteiro; a recodificação do índice com o zlib do Python é byte a byte, conferida nas duas versões).

### 4.4 Granularidade: sólido × por arquivo (zstd, `hfs-xor`)

| Tipo | Por arquivo, zstd 19 | Sólido zstd 19 | Sólido zstd 19 `--long=27` |
|---|---|---|---|
| maps | 27,8 % | 27,8 % | 26,5 % |
| resource | 7,1 % | 7,1 % | 6,3 % |
| scaleform | 84,2 % | 84,1 % | 84,0 % |
| materials, models, classe, sound, shaders | igual até 0,1 ponto | | |

O ganho de juntar pacotes de uma mesma pasta num fluxo sólido fica em ≈ 43 MB no cliente inteiro (0,4 %), e só com janela de 128 MB. Os `.hfs` já são "um pacote por pasta" e não repetem conteúdo entre si (nenhum SHA-256 duplicado).

Distribuição dos tamanhos da v3: 631 arquivos < 64 KB (10 MB no total), 1.501 < 1 MB, 3.460 < 16 MB, 3.590 < 64 MB; os maiores têm 206 MB (shader, que comprime para < 2 MB), 144 MB (`Base_hl2.hfs`) e 115/107/107 MB (pacotes). Com 3.613 objetos, um GET por objeto com keep-alive custa pouco; o `Range` retoma qualquer objeto grande.

### 4.5 Deltas entre versões (zstd 19, dicionário = arquivo antigo, janela ≥ antigo + novo, LDM; equivalente ao `--patch-from`)

| Arquivo | Novo | Objeto inteiro (zstd 19) | Delta | Observação |
|---|---|---|---|---|
| `sql/heroes.db3` v1→v3 | 17,96 MB | 2.637 KB | **445 KB** | |
| índice `E608DF10…` como está | 1,75 MB | 1.797 KB | 1.754 KB | zlib cifrado: delta inútil |
| índice, forma clara | 2,24 MB | 1.711 KB | 316 KB | exige recodificar o zlib bit a bit (fase 2) |
| `client.dll` pre-hcls → pre-miri (v1→v2) | 15,86 MB | 4.235 KB | **6,4 KB** | |
| `client.dll` pre-miri → atual (v2→v3) | 16,77 MB | 4.237 KB | **6,3 KB** | |
| `server.dll` original → atual | 18,15 MB | 5.139 KB | 9,3 KB | |
| `engine.dll` original → atual | 10,72 MB | 4.453 KB | 1,7 KB | |
| `5A7E0CE7….hfs` (1 arquivo trocado) como está | 35,01 MB | 15.566 KB | 14.466 KB | o XOR por posição estraga o delta |
| `5A7E0CE7….hfs`, `hfs-xor` | 35,01 MB | 2.396 KB | **187 KB** | |
| `0FABE22A….hfs`, `hfs-xor` | 34,49 MB | 2.355 KB | 210 KB | como está: 14.290 KB |
| `336A97B8….hfs` (1 de 131 trocado), `hfs-xor` | 5,07 MB | 4.557 KB | 24 KB | |
| `530D9B4D…`, `E67F5DB8…`, `97158EB4…` (1 trocado) | 0,5–5 MB | 0,4–4,5 MB | 23–52 KB | |
| `564E36A2…`, `82D37C6D…`, `BE7B0DBD…`, `DCB08EBB…` (1–3 novos) | 0,03–0,22 MB | | 3–46 KB | |
| `763FD8F1…` (1 novo, 20 trocados) | 0,76 MB | 758 KB | 343 KB | |
| `35C6890E…` (`pc_delia_town_motion.mdl.comp` trocado) | 3,73 MB | 3.809 KB | 3.683 KB | o `.comp` muda inteiro; inflado: **72 KB** |
| `E0F18F15…` (1 `.comp` trocado) | 0,37 MB | 379 KB | 373 KB | idem |
| **14 pacotes refeitos, somados (`hfs-xor`)** | 90,0 MB | 23,2 MB | **4,9 MB** | |

Tempo: o maior delta (pacote de 35 MB) levou 22 s para gerar e 75 ms para aplicar. Memória para aplicar: o arquivo antigo mais a janela, ≈ antigo + novo (70 MB no pior caso medido).

### 4.6 Download + descompressão: tempo total de instalação e de atualização

O HFS não é um formato de compressão: é um zip "armazenado" (método 0) com XOR. A única compressão que o jogo já usa são os `.comp` (zlib nível 6) dentro de materiais, modelos, scripts e pacotes de classe, cerca de metade dos bytes; por isso esses tipos não comprimem de novo (4.2). Em todos os cenários o jogador termina com o mesmo cliente de 15.044 MB no disco, byte a byte.

**Como foi medido** (`install_time.py`, saída em `install-time.txt`/`.json`): para cada tipo de pasta, o tempo de descompressão por byte medido na amostra (4.1, uma thread) vezes o tamanho do tipo no cliente; mais o XOR (5.663 MB/s, medido com XOR nativo de 64 bits, como o Rust fará) e o SHA-256 (2.032 MB/s, medido), que todo formato precisa para conferir. `Base_hl2.hfs` usa a razão dos mapas. A gravação no disco é igual em todos os formatos (os mesmos 15 GB): 11 s num NVMe (1.355 MB/s medidos com `fsync`), ≈ 2 min num HD (hipótese), e não entra nas colunas.

- **Soma** = baixar tudo e só depois descomprimir (pior caso).
- **Em fluxo** = o launcher descomprime cada objeto enquanto os seguintes baixam (o que 5.4 propõe); o tempo é o maior dos dois.
- **PC lento** = descompressão 4× mais lenta que nesta máquina (hipótese para um PC antigo de 32 bits; medir no gate).

**Instalar a v3 (download + descompressão, minutos):**

| Formato | Download | Descomprimir (rápido / lento) | 50 Mbit/s soma / fluxo | 100 Mbit/s soma / fluxo | 300 Mbit/s soma / fluxo | 1 Gbit/s soma / fluxo | 100 Mbit/s soma, PC lento | 1 Gbit/s soma, PC lento |
|---|---|---|---|---|---|---|---|---|
| `.hfs` como estão (sem compressão) | 15.044 MB | 7 s / 28 s | 40,2 / 40,1 | 20,2 / 20,1 | 6,8 / 6,7 | 2,1 / 2,0 | 20,5 | 2,5 |
| `.tar.gz` como está | 13.915 MB | 29 s / 116 s | 37,6 / 37,1 | 19,0 / 18,6 | 6,7 / 6,2 | 2,3 / 1,9 | 20,5 | 3,8 |
| gzip 6 + `hfs-xor` | 11.410 MB | 47 s / 188 s | 31,2 / 30,4 | 16,0 / 15,2 | 5,9 / 5,1 | 2,3 / 1,5 | 18,3 | 4,7 |
| zstd 3 + `hfs-xor` | 11.366 MB | 20 s / 80 s | 30,7 / 30,3 | 15,5 / 15,2 | 5,4 / 5,1 | 1,9 / 1,5 | 16,5 | 2,8 |
| zstd 12 + `hfs-xor` | 11.195 MB | 22 s / 88 s | 30,2 / 29,9 | 15,3 / 14,9 | 5,3 / 5,0 | 1,9 / 1,5 | 16,4 | 3,0 |
| zstd 19 + `hfs-xor` | 10.944 MB | 29 s / 116 s | 29,7 / 29,2 | 15,1 / 14,6 | 5,4 / 4,9 | 1,9 / 1,5 | 16,5 | 3,4 |
| **zstd adaptativo (3 ou 19) + `hfs-xor`** | **10.962 MB** | 28 s / 112 s | **29,7 / 29,2** | **15,1 / 14,6** | **5,3 / 4,9** | **1,9 / 1,5** | **16,5** | 3,3 |
| xz 6 + `hfs-xor` | 10.746 MB | 269 s / 1.076 s | 33,1 / 28,7 | 18,8 / 14,3 | 9,3 / 4,8 | 5,9 / 4,5 | 32,3 | 19,4 |

**Atualizar v2→v3 (os 137 pacotes novos da Miri, 1.829 MB):** todos os formatos ficam entre 1.813 e 1.830 MB de download, 2,5 min a 100 Mbit/s e 0,3 min a 1 Gbit/s (soma), porque esse conteúdo é quase todo `.comp`. A exceção é o xz: 44 s só de descompressão (3,1 min de soma a 100 Mbit/s). Numa atualização, o que reduz o download é baixar só o que mudou (e delta para o que foi alterado, 4.5); o formato pesa pouco.

Leitura:
- **`.hfs` como estão e `.tar.gz` como está são os mais lentos até 300 Mbit/s,** mesmo somando a descompressão: 20,2 e 19,0 min contra 15,1 min a 100 Mbit/s. Descomprimir (7–29 s) custa pouco perto dos 4–5 min de download a mais.
- **Com XOR desfeito, gzip e zstd baixam quase o mesmo, mas o gzip descomprime 1,6× mais devagar** que o zstd 19 e 2,3× mais devagar que o zstd 3. Não há motivo para `.tar.gz`.
- **zstd 3, 12, 19 e adaptativo empatam** em tempo total (diferença ≤ 0,4 min a 100 Mbit/s, mesmo num PC lento). O que muda entre eles é o tamanho no servidor e o custo de publicar (4.7).
- **O xz 6 é o "mais compacto que demora":** só 198 MB menor que o zstd 19 e 9× mais lento para descomprimir. Somado, perde até dos `.hfs` sem compressão a partir de 300 Mbit/s e, num PC lento, também a 100 Mbit/s (32,3 × 20,5 min).
- **Onde não comprimir ganharia:** só com 1 Gbit/s num PC lento (2,5 × 2,8–3,4 min, soma). Em fluxo e com duas threads de descompressão, esse empate desaparece.

### 4.7 Publicar uma versão nova: custo de criar, enviar ao servidor e distribuir

Medido para a maior mudança real, v2→v3 (137 pacotes novos, 1.829 MB, mais alguns arquivos alterados), e para a importação da base (15 GB, uma vez). Tempo de compressão extrapolado por tipo, uma thread (com duas threads, metade).

| Método | Publicar v2→v3: compressão | Enviar ao servidor | Jogador baixa (v2→v3) | Importar a base | Instalação nova depois de N versões |
|---|---|---|---|---|---|
| `.hfs` como estão, objeto por arquivo | 0 | 1.829 MB | 1.829 MB | 0 | 15.044 MB |
| Pacotes `.tar.gz`/`.tar.zst` por pasta (pw) | refazer o pacote de cada pasta tocada, ou objetos avulsos + "Consolidar instalação" | pacote inteiro da pasta ou objetos | idem | 7,3 min (gzip) | pacotes antigos + objetos avulsos, até consolidar |
| zstd 19 por arquivo | 6,4 min | 1.813 MB | 1.813 MB | 52 min | 10.944 MB |
| **zstd adaptativo por arquivo** | **6 s** | **1.824 MB** | **1.824 MB** | **37 min** | **10.962 MB** |
| xz 6 por arquivo | 9,8 min | 1.813 MB | 1.813 MB | 67 min | 10.746 MB |

**Nível adaptativo:** a ferramenta de publicação comprime cada arquivo com zstd 3 (334 MB/s) e só recomprime com zstd 19 se o resultado ficou abaixo de 97 % do original. Os `.comp` (materiais, pacotes de classe) ficam no nível 3, sem perda; mapas, scaleform, som, shaders, resource, modelos e os soltos vão a 19. Pela amostra: 9.794 MB a 19 e 5.250 MB a 3 na v3; a Miri inteira fica no nível 3. Custa 18 MB a mais na instalação (0,2 %) e corta a publicação da Miri de 6,4 min para 6 s e a importação de 52 para 37 min. O launcher não precisa saber o nível: qualquer nível descomprime com o mesmo código.

**Fluxo de publicação recomendado** (comando `client-dist publish --tree <pasta preparada> --title … --notes …`, ou o painel):
1. Calcula o SHA-256 de cada arquivo da pasta (≈ 15 GB a 2 GB/s: ≈ 8 s de CPU mais a leitura do disco) e monta o manifesto da versão nova.
2. Pergunta ao servidor quais SHA-256 ele ainda não tem (`POST /internal/objects/missing`). Na v2→v3, são só os pacotes novos e os poucos alterados; os outros 2.666 `.hfs` já estão lá.
3. Comprime só esses (adaptativo, 2 threads) e envia os objetos comprimidos, retomando se cair. O servidor descomprime, confere o SHA-256 e guarda em `objects/`.
4. O servidor gera os deltas dos arquivos alterados contra a versão anterior (segundos; o pior medido levou 22 s), assina o manifesto e troca o `versao.json`.
5. Os launchers veem a versão nova, comparam os manifestos e baixam só os objetos e deltas que faltam (4.5).

Com os `.hfs` como estão, o fluxo seria o mesmo, sem o passo de comprimir. O que se perde é o tamanho da instalação (+37 %) e os deltas: com o XOR por posição, o delta de um pacote refeito é 14,1 MB contra 187 KB. Os pacotes `.tar` por pasta são os mais trabalhosos de manter: cada versão precisa refazer pacotes ou acumular objetos avulsos e, de tempos em tempos, consolidar a instalação.

## 5. Recomendação

### 5.1 Formato, nível e transformação

- Cada arquivo do conjunto instalável vira **um objeto zstd**, nível 3 se o zstd 3 já deixar mais de 97 % do tamanho (os `.comp`) e nível 19 no resto (4.7), com checksum de quadro ligado e janela padrão do nível (≤ 8 MB; 9 MB de pico medido para descomprimir), **sem `--long`**.
- `transform` por arquivo no manifesto: `none` ou `hfs-xor` (todos os `.hfs`, exceto o índice). A tabela de 4.096 bytes vai como objeto do próprio manifesto (`hfs-keystream`), fora do Git, como 4.3.1 já previa. `hfs-comp` (4.3) fica como transformação opcional da fase 2, ligada só depois de um teste do deflate do launcher contra os SHA-256 de todo o cliente.
- O SHA-256 que identifica o objeto e que o launcher confere é o do **arquivo final no disco do jogador** (com o XOR). Assim "verificar arquivos" não depende da transformação, e o mesmo conteúdo nunca é guardado duas vezes.

### 5.2 Granularidade

**Objeto por arquivo, para instalar e para atualizar.** Os pacotes `.tar.zst` por pasta ou por grupo de 256 MB (proposta atual de 4.3.3) não se pagam aqui, ao contrário do pw-universal (61 mil arquivos pequenos):

- não comprimem melhor (4.4: ≈ 0,4 %, e só com 128 MB de janela);
- duplicariam o armazenamento (pacotes da instalação + objetos das versões) e exigiriam "Consolidar instalação" a cada versão grande; a v3 já reaproveita 2.666 dos 2.681 `.hfs` da v1 sem nada a fazer;
- o reparo, a retomada e o paralelismo já são por arquivo: um `.hfs` corrompido baixa um objeto de ≈ 4 MB em média;
- extrair em fluxo não precisa do dobro de disco: o pico extra é um objeto comprimido e um arquivo `.novo` (o maior, ≈ 115 MB).

A única vantagem dos pacotes, menos requisições, não pesa com 3.613 objetos. Se um dia pesar (por exemplo, muitos ícones novos soltos em `scaleform/`), o manifesto pode ganhar um "lote" opcional de arquivos < 64 KB (hoje 631 arquivos, 10 MB), sem mudar o resto.

### 5.3 Leiaute no disco do servidor (`${CLIENT_DIST_DIR}`)

```text
objects/<aa>/<sha256>.zst          um por conteúdo; <sha256> = arquivo final; zstd adaptativo + transform
deltas/<sha antigo>-<sha novo>.zst patch-from sobre a forma clara; só se < 50 % do objeto
manifests/<n>.json  + .sig         estado completo da versão n: caminho, sha256, tamanho, transform,
                                   tamanho do objeto, preservar; deltas disponíveis a partir de n-1
versao.json + .sig                 versão atual, mínima, launcher (sha, objeto)
status.json                        dinâmico, não assinado
```

Sem `packs/`. Os objetos nunca são apagados ao reverter; uma limpeza explícita no painel pode remover os que nenhum manifesto ainda publicado cita.

### 5.4 O que o launcher faz

**Instalar (ou "Já tenho o cliente"):**
1. `versao.json` e o manifesto alvo, assinatura Ed25519 conferida.
2. Para cada arquivo: se já existe com o tamanho certo e o SHA-256 bate (caso de quem já tem o cliente), pula. Senão, baixa `objects/<aa>/<sha>.zst` para `.vindictus-update/parts/<sha>.part` (retomável com `Range`; duas conexões em paralelo).
3. Descomprime em fluxo (`zstd` crate, `Decoder` com buffer de 64 KB), aplica o `transform` (XOR em fluxo pela posição absoluta), calcula o SHA-256 enquanto grava `<caminho>.novo`; confere o SHA-256, renomeia, apaga o `.part`.
4. Grava o estado e o manifesto aplicado. O disco extra nunca passa de um objeto comprimido mais um arquivo novo.

**Atualizar N→M (pode pular versões):** diferença entre o manifesto local e o alvo, por caminho. Para cada arquivo que mudou: se o manifesto alvo lista um delta a partir do SHA que o jogador tem, baixa o delta; lê o arquivo antigo, aplica a transformação inversa (forma clara), usa-o como dicionário, aplica a transformação de novo, confere o SHA-256 e grava `.novo`. Senão, baixa o objeto inteiro. Tudo vai para a pasta de preparo e só é renomeado no fim, com o jogo fechado e um diário de aplicação para retomar. "Verificar arquivos" nunca usa delta.

**Regras do delta (no servidor, ao publicar):** só para arquivos que existiam na versão anterior, com antigo e novo ≤ 64 MB (memória de aplicação ≤ 128 MB, segura num processo de 32 bits) e delta < 50 % do objeto. Pela tabela 4.5: sempre para `heroes.db3`, DLLs e `.hfs` refeitos com arquivos armazenados; não para o índice (fase 1) nem para pacotes cujo arquivo trocado é um `.comp` (o delta fica do tamanho do objeto, e a regra dos 50 % o descarta sozinha). Deltas só da versão imediatamente anterior bastam: o launcher encadeia, e quem pula muitas versões baixa objetos.

**Windows 7 de 32 bits:** a descompressão do zstd 19 pede 9 MB de memória. A velocidade medida aqui (611 MB/s, CPU moderna de 64 bits) não se aplica a um PC antigo de 32 bits; mesmo dez vezes mais lento, a rede continua sendo o gargalo. O xz (51 MB/s aqui) poderia não ser.

## 6. Tamanhos esperados (fase 1: zstd adaptativo + `hfs-xor`)

| Item | Tamanho |
|---|---|
| Download para instalar a v1 | 9.121 MB (de 13.205 MB) |
| **Download para instalar a v3** | **10.937–10.962 MB** (de 15.044 MB; o adaptativo custa ≈ 20 MB); 15,1 min a 100 Mbit/s com a descompressão; fase 2: ≈ 10.026 MB |
| Atualizar v1→v2 (14 pacotes refeitos, índice, `heroes.db3`, 3 DLLs; divisão suposta, seção 3) | **≈ 7,1 MB** com deltas (4,9 + índice inteiro 1,8 + 0,4 + 0,02); 41 MB só com objetos inteiros; fase 2: ≈ 2 MB |
| Atualizar v2→v3 (137 pacotes novos, índice, `heroes.db3`, DLLs) | **≈ 1.816 MB** (1.813 de pacotes novos, sem delta possível); fase 2: ≈ 1.550 MB |
| v1→v3 por atualizações | ≈ 1.823 MB, contra 10.937 MB de uma instalação nova |
| **Servidor, v1+v2+v3** | **≈ 11,0 GB**: 10.937 MB de objetos da v3 + ≈ 60 MB de versões antigas dos 15 `.hfs` e dos arquivos soltos alterados + ≈ 7 MB de deltas |
| Mesmo desenho do pw (pacotes da v1 + objetos + consolidação da v3) | ≈ 9,1 + 1,9 + 10,9 ≈ 21,9 GB |
| Importar a base no servidor (CPU) | ≈ 37 min numa thread com o nível adaptativo (52 min só com zstd 19); metade com 2 threads; uma vez só |
| Publicar v2→v3 (CPU) | ≈ 6 s de compressão adaptativa + ≈ 8 s de SHA-256 da árvore; 1.824 MB enviados |

## 6.1 Tamanhos reais (publicação do L1, 07/11/2026)

As três versões montadas pela receita (`build_client_version.py`) foram publicadas por `client-dist publish` (zstd adaptativo + `hfs-xor`, duas threads) num armazém novo e conferidas por `client-dist verify --deep` (3.543 objetos restaurados com o SHA-256 certo):

| Item | Estudo (extrapolado) | Real |
|---|---|---|
| Instalar a v1 (13.851 MB no disco) | 9.121 MB (69,1 %) | **10.328 MB (74,6 %)** |
| Instalar a v3 (15.770 MB no disco) | 10.937–10.962 MB (72,7 %) | **12.224 MB (77,5 %)** |
| Atualizar v1→v2, só objetos inteiros | 41 MB | **22 MB** (10 objetos) |
| Atualizar v2→v3, só objetos inteiros | ≈ 1.816 MB | **1.935 MB** (150 objetos) |
| Servidor, v1+v2+v3 (sem deltas) | ≈ 11,0 GB | **12,29 GB** |
| CPU da importação da v1 | ≈ 26 min com 2 threads | ≈ 30 min com 2 threads (máquina do operador) |

- A diferença vem da extrapolação (ponto 3 da seção 8): 1.809 arquivos, 6.336 MB (46 % da v1), ficam acima de 97 % no zstd 3 e quase não comprimem (`.comp` de materiais e modelos guardados em zlib pelo próprio jogo). A amostra pesava menos esses pacotes. O `hfs-xor` funciona: num pacote de mapa, o zlib cai de 100 % para 88,7 % e o zstd 19 para 83,8 %.
- Quem mais ganharia com a fase 2 (`hfs-comp`, 4.3) é justamente esse grupo de 6,3 GB.

## 7. Ajustes para `game-launcher.md` 4.3 (aplicados em 02/11/2026)

- 4.3.2: nível 19 **sem** `--long=27` (sem ganho por arquivo, 100 MB de memória).
- 4.3.3: trocar os pacotes de instalação por grupo por objetos por arquivo; remover a ação "Consolidar instalação" e `packs/` do leiaute 4.3.7.
- 4.3.1: XOR em todos os `.hfs` (exceto o índice), não só onde houver ganho; o XOR do arquivo inteiro basta.
- 4.3.4: delta só sobre a forma clara e com o limite de 64 MB; o índice vai inteiro até existir a fase 2.
- Exclusão (4.3.6): acrescentar `maps/graphs/*.ain`, `demoheader.tmp`; `SAVE/` e `cfg/config.cfg` como `preservar`.

## 8. Hipóteses e pontos em aberto

1. **Velocidade no Windows 7 de 32 bits:** não medida (só o pico de memória vale para 32 bits). Medir a descompressão do zstd 19 e o XOR do launcher real no gate do L.
2. **Divisão v1/v2/v3:** a cópia de laboratório só tem o estado final; os números por passo usam a divisão suposta na seção 3. A receita `build_client_version.py` (4.7) dirá os conjuntos exatos.
3. **Extrapolação (medida em 07/11/2026, seção 6.1: os tamanhos reais ficaram ≈ 12 % acima):** amostra de 970 MB (6,5 % do cliente), ponderada por bytes; materiais e modelos têm muitos pacotes pequenos e a amostra deles é de 32–36 MB. Como os dois são 100 % `.comp`, a razão quase não varia, mas a conta exata só sai da importação real.
4. **`hfs-comp` (fase 2):** a reprodução bit a bit valeu para 1.392 de 1.392 `.comp` da amostra com o zlib do Python; falta conferir todos os `.comp` do cliente e o deflate do Rust que o launcher usar (zlib clássico; zlib-ng e `miniz_oxide` geram outros bytes).
5. **`7B71CFAE5FFE3D99A91CE97CC5D80A3C4776F540.hfs`** no laboratório tem o mesmo tamanho do original (3.131 bytes), mas começa com `INDX` e não é um pacote HF; o original é. Precisa ser explicado (algum passo de laboratório escreveu nele?) antes de a receita da v2 ou v3 publicá-lo.
6. **HTTP/2 ou muitas conexões:** não medido. Com 3.613 objetos, HTTP/1.1 com keep-alive e 2 conexões deve bastar; medir no gate com o servidor real.
7. **Tempo da importação:** 37 min (adaptativo) ou 52 min (zstd 19) numa thread, extrapolados por tipo a partir da velocidade de compressão medida na amostra; a importação real deve rodar com 2 threads, para não esquentar a máquina do operador.
