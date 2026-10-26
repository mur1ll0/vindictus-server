# Roteiro de testes no cliente — B4 painel de administração

Bloco B4 (painel de administração e análise de extensões). Plano em `docs/b4-painel-plano.md`. Entradas acrescentadas a cada passo com efeito no cliente; o topo é completado no gate do B4.

## Preparação
1. Runtime no Docker já recompilado no fechamento do grupo B4.1+B4.2 (25/10/2026): `docker compose ps` mostra `game` e `server` saudáveis. Se o código mudou depois disso: `COMPOSE_PARALLEL_LIMIT=1 docker compose build` e `docker compose up -d --wait` (o serviço `migrate` aplica as migrações).
2. Painel: `http://127.0.0.1:27090` (só no próprio computador).
3. Conta de administração criada a pedido do operador: login `admin`, papel admin, ligada ao usuário de jogo `lab169_a` (Fiona Murael, nível 20).

### B4.2 — Conta jogável com login e senha
Pré-condição: cliente fechado; containers no ar.
- [ ] Abrir `http://127.0.0.1:27090`, entrar com `admin` e a senha → o painel abre em Contas, mostra "admin (Admin)" no topo e a conta `admin` com "no jogo: lab169_a", "UID 952894723, 1 personagem(ns)".
- [ ] Auditoria → aparecem a criação da conta pelo CLI (papel e ligação ao `lab169_a`, com motivo) e a sua entrada no painel.
- [ ] Gerar o código: `pwsh scripts/dev/play.ps1 -Username admin -StartClient` (ou a página `http://127.0.0.1:27090/jogar`) → mostra um código de 12 letras/números e o copia para a área de transferência; o cliente abre.
- [ ] Na tela de login do jogo, digitar (ou colar) o código no campo ID, senha em branco → pede a segunda senha de sempre e mostra a Fiona Murael na lista; entrar na cidade → personagem, inventário e ouro como antes.
- [ ] Sair do jogo e tentar o mesmo código de novo → recusado (mensagem de permissão).
- [ ] Tentar entrar digitando `lab169_a` no campo ID (como antes) → recusado: o nome agora pertence à conta `admin` e exige o código.
- [ ] Gerar um código com a senha errada → "Login ou senha inválidos"; nada muda no jogo.
- [ ] No painel, Criar conta `teste_b4` (sem papel, senha de 12+ caracteres, motivo) → aparece na lista como "nunca entrou"; gerar o código com ela, entrar no jogo → lobby vazio, criar uma Fiona; voltar ao painel → "UID …, 1 personagem(ns)".
- [ ] Desativar `teste_b4` no painel (motivo) e gerar um código com ela → recusado.
- [ ] Relogar com `admin` (código novo) → a Murael continua com tudo preservado.
Limites conhecidos: o campo de senha da tela de login do jogo é ignorado (o cliente não o envia com `NGM_disable`) e `-passport` passa pelo SSO da Nexon, por isso o código; o código vale 5 minutos e uma vez; ser GM dentro do jogo (comandos de GM, `Permissions.Cheat`) ainda não existe — o papel admin vale só para o painel; bloquear com a mensagem de ban do jogo, sessões ao vivo e desconectar pelo painel chegam nos próximos passos.

## Como relatar
Para cada linha marcada: ID da entrada e número da linha (ex.: `B4.2-4`), o que foi feito, o que apareceu na tela (texto do diálogo/mensagem; captura se possível) e o horário aproximado (hh:mm), para comparar com `docker compose logs game` e a tela Auditoria do painel. Linhas não testadas ficam desmarcadas com o motivo.
