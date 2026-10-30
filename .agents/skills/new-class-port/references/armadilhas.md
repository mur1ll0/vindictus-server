# Armadilhas do porte da Delia

Cada item: o sintoma, a causa, como evitar e onde está a evidência (passo do `docs/history/session-log.md`, entrada de
roteiro ou seção do `docs/future/delia-class-port.md`).

## 1. Limites de 10 classes antes de tabelas por classe
- **Formas encontradas:**
  - `cmp reg, 9; ja` → caso padrão: arquivo de estado de ação, nome em minúsculas, roupa do jogador
    (`COSTUME_GATE`: corpo invisível na cidade, passo 21), evento de animação (`EVENT_FILE`, passo 15),
    habilidades (`CLASS_BITS`: janela vazia, passos 25 e 27), roupas de amostra (C3-9);
  - `cmp reg, 0xa; jge` → bloco inteiro pulado: preparação do jogador local (`GATES`, queda `0x387da6b0`, passo 4);
  - `cmp R, 0xa; jl; xor R, R` → lê a linha da Lethita: armas (duas espadas, passo 31);
  - tabela sem limite → lê a tabela seguinte: `.csf`, sons, animações (esqueleto masculino), padrões de
    roupa (decalque branco na seleção, passo 29);
  - cadeia de comparações sem caso padrão → valor 0: estamina parada (`CLASS_ALIAS`, passo 32);
  - caso 10 = pet: expressões e palavras-chave (o pet foi para 28, passo 3);
  - limite de condições `< 0x79` (passo 33).
- **Como evitar:** antes de cada teste de cliente, varrer os caminhos que a classe toca
  (`v169_class_uses.py`; varreduras `d12-classbit-scan.txt`, `d14-classbit-scan2.txt`, `d19-class-chains.txt`
  em `.cache/verification`). Um caminho novo da classe (arma, criação, efeito) costuma trazer outro limite.

## 2. Tabela de nascimento e `SetClass` no servidor local
- **Sintoma:** a Delia criada pela grade aparecia masculina na customização; memória com `m_iClass` 0 em
  `+0x1400` (servidor) e `+0x18ec` (cliente).
- **Causa:** o nascimento compara `cl_playerclass` com as 10 chaves de `0x40CFA1B0` (laço `cmp esi, 0xa` em
  `0x4055A336`) e chama `SetClass(0)` quando nenhuma bate; o `SetClass` (`0x4055A190`) ainda corta em 9.
  Sem fábrica `player_<classe>`, o servidor cria a entidade base `player`, cuja classe fica 0 se ninguém a grava.
- **Como evitar:** a chave da classe nova entra pela tabela (`TABLES`) e os limites por `SERVER_CLASS_BOUNDS`,
  ambos já dirigidos por `EXTRA_CLASSES`. Na cidade a classe vem dos dados da personagem; na criação, só do
  nascimento. Evidência: C3-5, C3-6.

## 3. Byte de prólogo só na estalagem
- **Sintoma:** "Go to town" travava em "Loading" (C3-21); com o byte ligado na customização, digitar o nome
  numa pose de andar voltava a girar o modelo e as poses paravam (C3-22).
- **Causa:** o botão `ToTown` (`0x383c4390`) só toma o ramo do prólogo (`force_start_touch trigger_startgame`
  → `cc_start_game_first`) com o byte `[[máquina]+4]+4` ligado; desligado, manda `QueryNpcTalk` ao servidor
  e marca a fala como `Loading`. O byte também troca mouse e teclado do prólogo (`0x38591500`, `0x385f8b20`).
- **Como evitar:** ligar o byte no mapa pronto da estalagem (estado 12, *slot* 4), antes da fala; o próprio
  1.69 desliga ao entrar na cidade vindo do estado 13. É genérico: a classe nova não muda isso.
- **Lição de método:** a premissa do C3-21 (gatilho do mapa falhando) estava errada; ler a condição do ramo
  na DLL original antes de contornar um caminho.

## 4. Arquivo solto × pacote renomeado
- **Sintoma:** botão de voltar ausente na customização apesar de *stubs* certos (C3-8 a C3-11); depois,
  campo de nome sumido por um diagnóstico errado do atlas (C3-13, C3-19).
- **Causa:** a cópia carrega os filmes e imagens soltos de `scaleform/`; o pacote `9FB527B3…`, com a versão
  mais nova do `CreateCharacterDialog.gfx`, está renomeado `RENOMEADo_9FB527B3….hfs` e não é lido. A análise
  usou o filme do pacote; o jogo usava o solto, sem `Button_Back` no palco.
- **Como evitar:** antes de analisar um filme, imagem ou *script*, confirmar qual arquivo o cliente lê (solto,
  pacote legível, pacote renomeado) e registrar o SHA-256 do que está em uso. Trocar um solto deixa
  `<arquivo>.<hash>.loose.bak` ao lado. Imagens soltas `_I<n>.dds` podem ser atlas usados por outro filme.

## 5. Registradores e pilha nos *stubs*
- **Sintoma:** "Create" derrubava o cliente em `0x384b111d` (C3-15); o laço da `SkillActionStateMatchTable`
  caía em `0x407cda68` (D8c).
- **Causa:** o *stub* voltava ao original depois de chamadas (`strcmp`, teste de *feature*) que trocaram
  `ecx`; o original seguia com `mov esi, ecx`. No D8c, o laço relia `&mapa->m_Elements` de `[esp+0x10]`,
  que o *stub* não atualizava; no passo 22 o caminho da classe extra precisava manter `lea edx, [esp+0x10]`.
- **Como evitar:** antes de escrever um *stub*, desmontar o original até o fim do uso dos registradores e da
  pilha; restaurar `ecx` (e o `this` em geral) antes de voltar; atualizar todo valor que o original relê
  depois. O teste do *stub* deve desmontar o caminho de volta.

## 6. Nada de mudança de estado dentro de *callbacks*
- **Casos:**
  - disparar `cc_start_game_first`/um evento da máquina de UI dentro do *callback* de uma fala derrubaria o
    objeto em uso (C3-21, retirado no C3-22);
  - reconstruir o modelo de roupa dentro do despacho de eventos de animação liberava o `CStudioHdr` que o
    despacho ainda lia (queda `0x40383bab`, passos 45–46);
  - queda de nível e perda de reserva de efeitos ficam no *think*, nunca no *callback* do efeito (passos 39–40).
- **Como evitar:** marcar a mudança numa entrada da `.hrte` e executá-la no *think* da classe ou no caminho
  original que o 1.69 já usa para isso.

## 7. Memória antes de código
- **Casos:** três correções estáticas seguidas sem efeito no D8g (passo 9); a leitura ao vivo dos registros
  de item mostrou que `+0x1a2` era `Indestructible` (passo 43); a memória mostrou o *stub* do C3-21 nunca
  executado e o byte de prólogo em 0 (C3-22); a memória achou `m_iClass` 0 na customização (C3-6) e as peças
  resolvidas ainda presentes no barco (passo 50).
- **Como evitar:** depois de uma correção sem efeito, não empilhar outra hipótese estática: ler a memória
  com o jogo parado no ponto do defeito (`lab_memory.py serve`, `entities`, `dword`, `str`, `save`/`diff`
  entre uma classe que funciona e a nova). Pontos de parada de escrita (`lab_watch.py`) acham o gravador;
  os de execução derrubam o jogo. Regra 5 do `AGENTS.md`: depois de uma investigação focada sem resultado,
  parar e registrar.

## 8. Heredocs do Bash quebram escapes de *bytes*
- **Sintoma:** *scripts* Python de leitura ou de teste gerados por *heredoc* chegam com `\x..`, `\\` e `$`
  alterados pelo *shell*: *bytes* errados na comparação ou no patch, sem erro aparente.
- **Como evitar:** gravar o *script* com a ferramenta de escrita de arquivos em `.cache/` ou no *scratchpad* e
  rodar o arquivo (`python -s -E caminho.py`); usar `bytes.fromhex('…')` em vez de literais com `\x`.

## 9. Outras lições curtas
- **Grupos da Delia valem para toda classe extra**: `GUARD`, `CLIENT_GUARD`, `CLASS_CONDITIONS` e
  `DELIA_EFFECTS` despacham qualquer classe ≥ 10 para o mesmo corpo. Restringir antes de pôr a Miri.
- **Valores para o gerenciador de UI do Scaleform**: o *set* de variável `vt+0x1c` recebe texto (um inteiro
  virou ponteiro e derrubou o `shaderapidx9.dll`, C3-7); `"true"` vira `NaN` numa propriedade de exibição,
  use `"1"` (C3-10). Escritas repetidas do C++ num campo de texto tiram o foco (C3-16).
- **Tamanho de vetores por cópia**: o construtor de cópia copiava 11 *bytes* de *flags*, mas o décimo primeiro
  era outro campo (`Indestructible`). Confirmar o campo pelo carregador, não pela cópia (passo 43).
- **Linhas de 5 nomes** na lista de armas por classe (queda `0x40825900`, D18).
- **Caminho de chamadas estático**: o `callpath.py` do passo 45 lia além do `ret`; conferir o fim de cada
  função antes de afirmar um caminho (correção no passo 46).
- **Habilidades acrescentadas, nunca renomeadas** na troca de classe de laboratório (com as habilidades
  renomeadas, a entrada na cidade desconectava, D10; passo 24). A forma do corpo e a figura padrão da classe também mudam (passo 26).
- **Aplicar com o jogo fechado e a partir dos *backups***: o patch de roteamento recusa um DLL que já tem `.hrte`.
- **Depurador**: o antitrapaça pode abortar o cliente depois de desanexar (D8o); conte com reiniciar.
