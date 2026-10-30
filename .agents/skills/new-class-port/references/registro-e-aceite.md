# Registro e aceite

## Entrada do `docs/history/session-log.md` (uma por passo verificado)
Modelo, no estilo dos passos C3-x da Delia:

```
Passo <ID> (<data>): <o que muda, numa linha>.
- Relato do operador (<roteiro>, <rodada>): <o que apareceu>.            # quando veio de um teste
- Causa: <o que o código ou a memória mostrou, com endereços>.
- Mudança: <grupo/arquivo e o que faz; só na cópia de laboratório, se for o caso>.
- Verificação: <testes focados> (<N> OK; `.cache/verification/<etapa>/<log>`).
- Aplicado com o jogo fechado, a partir dos `*.pre-routing.bak` (`.cache/verification/<etapa>/class-routing-apply-<rodada>.log`): `client.dll` `<hash>…`, `server.dll` `<hash>…` (igual), `engine.dll` `<hash>…`.
- Cliente: `docs/client-tests/<roteiro>.md`, <entrada> (pendente) | cliente: não necessário.
- Próxima ação: <o que fazer em seguida>.
```

- Parada (regras 2–5 do `AGENTS.md`): escrever "Parada (regra N): <motivo>" e a ação retomável.
- Hipótese descartada também se registra (o que foi lido e por que não é a causa), como nos passos 13–16.
- Mudança só na cópia de laboratório (filmes, imagens, *scripts* soltos): SHA-256 antes → depois e o caminho
  do `.loose.bak` para desfazer (C3-12 a C3-17).

## Roteiro de cliente por etapa
Arquivo `docs/client-tests/<etapa>-<classe>.md` (Delia: `C2-delia.md` com entradas D*, `C3-criacao.md` com
entradas G*). Escolha um prefixo de ID próprio para a classe. Formato de `docs/playable-game-roadmap.md`:

```
# Roteiro de testes no cliente — <etapa> <classe>

<objetivo em 2–3 linhas e a entrada do docs/history/session-log.md de origem>

## Preparação
1. Servidor: <imagem/commit>; `docker compose ps` com `game` e `server` saudáveis.
2. DLLs: <patches aplicados, hashes no docs/history/session-log.md>; jogo fechado durante a aplicação.
3. Cliente: `.cache/play-client/start-lab.bat`, conta `lab169_a`; personagem <nome> (<classe, nível, itens>;
   comandos `lab-set-class.ps1`/`lab-give-item.ps1` com a personagem fora do jogo).
4. Relatórios de queda: `Documents\Vindictus EU GM\Report Error` (nome do mais novo e horário).

## Ordem de gameplay (uma sessão)
1. **<ID1>:** ...

## Como relatar
ID e número da linha, o que apareceu (captura se possível), se travou ou fechou, horário e relatório de
queda. Na primeira falha que bloqueia as seguintes: anotar, voltar ao estado seguro e encerrar.

### <ID> — <funcionalidade>
Pré-condição: <estado necessário; comando de laboratório>.
- [ ] <ação no cliente> → <resultado esperado>.
- [ ] Não regressão: <classe antiga, mesma ação>.
Limites conhecidos: <aproximações declaradas>.
```

- Depois de cada rodada, anotar o resultado embaixo da entrada (ou numa seção "Resultados") e marcar as
  caixas; rodadas substituídas ficam como encerradas (fechamento do `C2-delia.md`).
- Quando a próxima mudança depende do resultado (regra 2), pare depois de escrever a entrada.

## Documento da classe `docs/future/<classe>-class-port.md`
Mesma estrutura do `delia-class-port.md`; não copie o da Delia, cite-o quando o mecanismo for o mesmo.
1. Visão geral e estado (o que funciona no cliente, o que não foi confirmado, o que ficou aproximado).
2. Arquivos de conteúdo (origem de cada arquivo, tabela "arquivo → quem lê → patch que o liga", mudanças
   na cópia de laboratório com antes/depois e *backup*).
3. Banco e dados de criação (migrações, linhas de laboratório, `lab-set-class.ps1`, FeatureMatrix).
4. DLLs (procedimento de aplicação, *hashes* finais, cada grupo com endereços, defeito, evidência e passo;
   criação pela grade).
5. Servidor Rust (módulo, mudança, passo/*commit*; testes focados).
6. Testes de cliente (tabela entrada → parte validada → resultado).
7. Pendências e aproximações.
8. Receita para a próxima classe (o que esta classe mudou na receita).
9. Inventário completo:
   - repositório: `git diff --name-only <primeiro commit do porte>~1 HEAD`, filtrado por área, com a lista
     bruta em `.cache/verification/<etapa>/<classe>-changed-files.txt`; separar o que é de outras sessões;
   - DLLs da cópia de laboratório: original, intermediário, final, *backups* e patch (tabela 9.2 da Delia);
   - conteúdo da cópia de laboratório: arquivo, SHA-256 antes → depois, *backup*;
   - **o que veio do NA**: arquivos copiados (origem no pacote ou no `hfsExtracted`, destino, SHA-256 e a
     decisão do operador com data) e dados/comportamentos recuperados (endereço NA de cada um).

## Git
- *Commit* local por passo, com código, testes, roteiro, documento da classe, linha do `MIGRATION.md` e entrada do histórico juntos.
- `git add` com caminhos explícitos; revisar a lista e o *diff* antes do *commit*.
- Nunca: assets do jogo, DLLs, `heroes.db3`, *dumps*, exportações privadas, credenciais, fontes
  decompiladas. `.cache/` e `docs/evidence/client/` são locais.
- Sem *push*.

## Checklist final de aceite (detalhado)
Pré-condição: todos os patches aplicados; Docker reconstruído no fechamento; roteiro completo.

| Área | Esperado | Referência na Delia |
|---|---|---|
| Seleção | modelo, rosto, cabelo, roupa e pele certos; sem decalque branco; nome e ícone da classe | D17 |
| Cidade | corpo e animações; sem queda ao entrar, no emote nem no Mercenary Outpost; estamina normal | D8v, D8c, D19 |
| Arma | uma arma no lugar certo; ataque básico; combate numa fase | D18 |
| Habilidades | janela com as habilidades da classe em posições distintas; aprender com AP; ativas até o nível alto; SP pela barra | D13, D22 |
| Mecânicas | cada mecânica do escopo combinado funciona; aproximações declaradas no roteiro | D22, D23 |
| Itens | itens da classe e comuns sem fundo vermelho; dica sem "Cannot be equipped" e com o nome da classe; itens de outras classes continuam vermelhos; Fiona sem regressão | D22 |
| Grade | a classe na grade; ator no centro com a pose de apresentação; vídeo e descrição certos | G1, G2 |
| Customização | modelo da classe; abas mudam o modelo; roupas de amostra; nome e "Check"; `Back` volta à grade; WASD travado; poses trocam na hora | G4, G6 |
| Criação até a cidade | "Create" → estalagem → fala da Tieve → "Go to town" → cidade com a personagem nova | G4 |
| Volta à seleção | a personagem criada aparece certa e entra de novo; classe antiga pela grade até a cidade (G3); voltar da grade sem criar (G5) | G3, G4, G5 |

Com o aceite do operador: marcar o item no `MIGRATION.md`, registrar no `docs/history/session-log.md` (citação do relato), atualizar o estado no documento
da classe e encerrar as caixas do roteiro.
