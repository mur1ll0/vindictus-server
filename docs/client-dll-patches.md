# DLLs do cliente de laboratório: como são gerados e versionados

Vale para `bin/client.dll`, `bin/server.dll` e `bin/engine.dll` da cópia `.cache/play-client`. O `server.dll` aqui é o servidor de jogo local que o próprio cliente carrega, não o servidor Rust.

## Resumo
- **Os DLLs não vão para o Git.** São binários proprietários (`AGENTS.md`, "Preservação"). Ficam só na cópia de laboratório, fora do repositório.
- **O que vai para o Git é o código que os gera.** São seis *scripts* Python em `scripts/client/`, cada um com seu teste. Cada DLL do laboratório é o original do 1.69 mais os patches desses *scripts*, aplicados numa ordem fixa. A saída é determinística: os mesmos originais e o mesmo *commit* dão os mesmos bytes.
- **Nenhum DLL é editado à mão** (editor hexadecimal, *debugger*, cópia de bytes avulsa). Toda mudança entra como código num desses *scripts*, com teste, e o DLL é refeito.
- **Verificado em 02/11/2026:** os três DLLs foram reconstruídos a partir dos originais, numa pasta descartável, com o *commit* `79c425d` da `feat/miri-class`. Os SHA-256 deram iguais aos da cópia em uso. Logs em `.cache/verification/dll-repro/`.
- O `Vindictus.exe` e os outros 34 DLLs de `bin/` são iguais aos originais do 1.69 (`../Vindictus1.69EN/Vindictus EU/en-EU`). Só esses três mudam.

## Cadeias

| DLL | Original 1.69 (SHA-256, tamanho) | Cadeia de patches (ordem) | Atual (02/11/2026) |
|---|---|---|---|
| `client.dll` | `b284ba719047556b`, 14.719.200 | `patch_shop_list_limit.py` → `314272ec…` → `patch_class_table.py` → `0a0b866d…` → `patch_class_routing.py` → `c0b5d416…` → `patch_client_launcher_login.py` | `0b7d0248d17a34c2`, 17.589.248 |
| `server.dll` | `d7727e182f6cdec1`, 16.311.216 | `patch_class_table.py` → `c85a56d5…` → `patch_class_routing.py` | `5f0429365ee0ccd1`, 19.028.480 |
| `engine.dll` | `a3add9742f7fa03c`, 11.237.760 | `patch_engine_class_flags.py` → `51ea90e7…` → `patch_engine_iocp_sign.py` | `df6c70d2d66f52ed`, 11.238.400 |

Os hashes intermediários são os que os *scripts* esperam na entrada. O hash final muda a cada mudança em `patch_class_routing.py` ou `EXTRA_CLASSES`. A lista acima vale para o *commit* da verificação; nos passos seguintes, o hash atual fica registrado no `docs/history/session-log.md`.

## Os *scripts*

| *Script* (teste) | DLL | O que faz | Entrada aceita | Marca no DLL | *Backup* que cria |
|---|---|---|---|---|---|
| `patch_shop_list_limit.py` (`test_patch_shop_list_limit.py`) | client | Limite de download da lista do *cash shop*: 1 MiB → 16 MiB (dois `push` imediatos) | Só o original (`SOURCE_SHA256`) | Nenhuma (bytes trocados) | `client.dll.b284ba719047556b.bak` |
| `patch_class_table.py` (`test_patch_class_table.py`) | client, server | Tabelas `{nome, corpo}` de classe e lista de nomes numa seção nova, com 32 entradas; `ACTIVE_CLASSES` | Sem `.hcls` | Seção `.hcls` | `<dll>.<hash>.pre-hcls.bak` |
| `patch_class_routing.py` (`test_patch_class_routing.py`) | client, server | Todo o roteamento e as mecânicas das classes extras (`EXTRA_CLASSES`: Delia 10, Miri 11); detalhe abaixo | `EXPECTED` (saída do `.hcls`); recusa DLL que já tem `.hrte` | Seção `.hrte`; `bin/class-routing.json` | `<dll>.<hash>.pre-routing.bak` |
| `patch_engine_class_flags.py` (`test_patch_engine_class_flags.py`) | engine | Um *byte* de classe por item para as classes extras (`CLASS_FLAGS`: 10 → `+0x1cd`, 11 → `+0x1ce`) | Sempre parte do original (`EXPECTED`); se a cópia já tem o patch, usa o `*.pre-classflags.bak` | Seção `.hflg` | `engine.dll.a3add9742f7fa03c.pre-classflags.bak` |
| `patch_client_launcher_login.py` (`test_patch_client_launcher_login.py`) | client | Login pelo launcher: com `-launcher_ticket <t>`, o `OnShow` do LoginDialog faz o que o OK faz, com o ticket como ID, sem esperar digitação; sem o parâmetro nada muda (`docs/future/game-launcher.md` §4.1) | O epílogo original em `0x3826ae0c` (`EPILOGUE`) | Seção `.hlgn` | `client.dll.<hash>.pre-launcherlogin.bak` |
| `patch_engine_iocp_sign.py` (`test_patch_engine_iocp_sign.py`) | engine | ProudNet: `jge` → `jmp` no teste com sinal do `OVERLAPPED` (sem isso, eventos de rede acima de 2 GB eram descartados; `docs/battle-transport.md`) | Os 4 bytes originais no endereço (`ORIGINAL`) | Bytes trocados | `engine.dll.<hash>.pre-iocpsign.bak` |

Sem `--apply`, todos simulam e só imprimem um relatório JSON com `before_sha256` e `after_sha256`. Todos se recusam a mexer no cliente original e só gravam numa cópia.

### `patch_class_routing.py`
Concentra quase todo o trabalho de classe nova: cerca de 4.000 linhas e mais de 120 tabelas de constantes. Os grupos que ele aplica (chaves do relatório JSON): `switches`, `tables`, `bits`, `class_bits`, `class_name`, `class_alias`, `class_defaults`, `class_weapons`, `client_weapon_switch`, `entity_slots`, `shape_class`, `precache`, `match_table`, `guard`, `gates`, `conditions`, `class_conditions`, `frame_keys`, `flag_checks`, `tooltips`, `item_mask`, `item_copy`, `costume`, `costume_gate`, `keyword_tables`, `event_file`, `custom_effect`, `delia_effects`, `flamebreath_leave`, `summon_parent`, `creation_grid`.
- O código novo vai para a seção `.hrte`. No código original entram só saltos para ela e trocas de imediatos. Os endereços de cada *stub* ficam em `bin/class-routing.json`, que `scripts/dev/lab_memory.py` lê para inspecionar a memória do jogo.
- Uma classe nova é uma entrada a mais em `EXTRA_CLASSES` (`ExtraClass(índice, chave, classe_base, corpo, figura, constantes, rostos, weapons=…, server_constants=…)`). Os grupos leem essa tabela. Mecânicas exclusivas de uma classe ficam presas ao índice dela (por exemplo `DELIA_MECHANICS` = `(10,)`).
- Para cada grupo, o teste desmonta o *stub* gerado (Capstone) e confere instruções e constantes no DLL patchado.

## Como a versão é rastreada
1. **Código:** a versão de um DLL é a do *commit* dos seis *scripts*. Uma mudança de patch é um *commit* comum, com teste, roteiro de cliente e entrada no histórico (`AGENTS.md`, "Git e conclusão de etapa").
2. **Guardas de entrada:** cada *script* confere o hash ou os bytes de entrada (`SOURCE_SHA256`, `EXPECTED`, `ORIGINAL`) e a seção marcadora. Assim nenhum patch é aplicado duas vezes nem fora de ordem. Quando a entrada esperada muda (por exemplo, `ACTIVE_CLASSES` muda o `.hcls`), o `EXPECTED` do roteamento e o do `scripts/reverse/v169_class_uses.py` mudam no mesmo *commit*.
3. **Hashes no histórico:** cada aplicação registra no `docs/history/session-log.md` o hash antes → depois e o log (`.cache/verification/<etapa>/`).
4. **Testes:** os do roteamento, do `engine.dll` *flags* e do IOCP montam o patch sobre os DLLs reais da cópia de laboratório (ou os *backups* originais) e conferem o código gerado. Sem a cópia eles pulam, e teste pulado não conta como evidência. Os da tabela de classes e da loja usam imagens sintéticas.
5. **Reconstrução:** a seção seguinte refaz os três DLLs a partir dos originais e prova que a cópia em uso corresponde ao código.

## Reconstruir do zero (a partir dos originais)
Para conferir que a cópia em uso corresponde ao código, ou para recuperá-la:

```bash
S=<pasta descartável>; B=.cache/play-client/bin; mkdir -p "$S/bin" .cache/verification/dll-repro
cp $B/client.dll.b284ba719047556b.bak "$S/bin/client.dll"
cp $B/server.dll.d7727e182f6cdec1.pre-hcls.bak "$S/bin/server.dll"
cp $B/engine.dll.a3add9742f7fa03c.pre-classflags.bak "$S/bin/engine.dll"
for p in patch_shop_list_limit patch_class_table patch_class_routing patch_client_launcher_login patch_engine_class_flags patch_engine_iocp_sign; do
  python -s -E scripts/client/$p.py --client-copy "$S" --apply > .cache/verification/dll-repro/$p.log 2>&1 || echo "$p falhou"
done
sha256sum "$S"/bin/{client,server,engine}.dll $B/{client,server,engine}.dll
```

Os originais também estão em `../Vindictus1.69EN/Vindictus EU/en-EU/bin` e em `../Client/bin`, que são somente leitura: copie, nunca aplique lá. Os *backups* com hash no nome podem ser conferidos pelo próprio nome.

## Reaplicar na cópia em uso depois de mudar um patch
Sempre **com o jogo fechado**: os DLLs ficam carregados enquanto ele roda.
- **Roteamento** (o caso comum): copie os `*.pre-routing.bak` atuais (`client.dll.0a0b866dcc0a1852.pre-routing.bak`, `server.dll.c85a56d5b845810f.pre-routing.bak`) sobre `bin/` e rode `patch_class_routing.py --apply`. Ele recusa um DLL que já tem `.hrte`. Depois, no `client.dll`, rode `patch_client_launcher_login.py --apply` de novo.
- **`.hcls`** (mudou `ACTIVE_CLASSES` ou a tabela de classes): parta dos `*.pre-hcls.bak` e rode de novo o `.hcls` e, depois, o roteamento.
- **`engine.dll`**: `patch_engine_class_flags.py` sempre parte do original e, por isso, **desfaz o patch IOCP**. Depois dele, rode `patch_engine_iocp_sign.py` de novo. A ordem é sempre essa: *flags* → IOCP.
- Depois de aplicar, confira o hash final no relatório e registre-o no histórico.

## *Backups* na pasta `bin/`
Cada *script* deixa a entrada que recebeu ao lado do DLL, com o hash dela no nome (`.pre-hcls`, `.pre-routing`, `.pre-classflags`, `.pre-iocpsign`, e o `.bak` do original do `client.dll`). Os outros nomes (`.pre-miri`, `.pre-chain`, `.pre-flame`, `.pre-look`, `.pre-keyword`, `.pre-dk`, `.pre-r2`…, `.pre-m7y`, `.pre-m8*`) são retratos que cada sessão guardou do DLL patchado antes de reaplicar, para voltar atrás rápido durante um teste de cliente. Com o código, qualquer um deles pode ser refeito a partir do *commit* do passo. Em 02/11/2026 eram 41 arquivos (671 MB). Apagar exige decisão do operador. Os quatro *backups* dos originais e dos `pre-routing`/`pre-hcls` atuais nunca saem.

## Regras para um patch novo
- Escreva o patch como código num dos *scripts* (no roteamento, como grupo novo ou como campo de `ExtraClass`) e acrescente o teste que desmonta o resultado. Nada de bytes avulsos na cópia.
- Endereços e *layouts* vêm de evidência registrada no comentário do grupo (endereço do 1.69, do NA x64, teste de cliente). Não invente valores.
- Um patch que parte do original, como o `engine.dll` *flags*, obriga a reaplicar os que vêm depois dele. Documente a ordem no *docstring* e nesta página.
- Antes de cada teste de cliente, procure limites de 10 classes (`cmp reg, 9`/`0xa`) nos caminhos novos (`scripts/reverse/v169_class_uses.py`).
- Um patch que corrige algo genérico do 1.69, sem relação com classe (como o IOCP), pode nascer em outra branch. Antes de reaplicar o `engine.dll`, junte as branches: os dois patches convivem na mesma cadeia.
