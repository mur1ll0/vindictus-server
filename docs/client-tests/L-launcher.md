# Roteiro de cliente — launcher (trilha L)

Plano e evidência: `docs/future/game-launcher.md` (seções 4.1 e 4.4). A preparação completa, a ordem de gameplay e o modo de relatar entram aqui no gate L6.

### LA1 — Start Game direto para a seleção de personagem (login pelo launcher)
Pré-condição: Docker no ar (`docker compose up -d --wait`); cópia `.cache/play-client` com `client.dll` `608bf9bc…` (`patch_client_auto_login.py`, aplicado em 02/11/2026); conta `lab169_a`; jogo fechado. Abrir o jogo por um terminal na pasta `.cache/play-client` (o `start-lab.bat` não passa o parâmetro).
- [ ] `vindictus.exe -stage -dev -lang en-eu-x-gm -noupdate -threads 4 +event_account lab169_a` → tela de título → Start Game → **seleção de personagem sem a janela de ID e senha** → segunda senha como sempre → entrar na cidade.
- [ ] Fechar e abrir pelo `start-lab.bat` (sem o parâmetro) → Start Game → a janela de ID e senha aparece como antes; entrar digitando `lab169_a`.
- [ ] `vindictus.exe -stage -dev -lang en-eu-x-gm -noupdate -threads 4 +event_account conta_inexistente_la1` → Start Game → recusa do servidor; anotar a mensagem e se o jogo volta ao título ou abre a janela de login.
Limites conhecidos: ainda sem ticket (a conta entra pelo nome, como hoje); depois de uma recusa, o Start Game repete o mesmo valor até abrir o jogo de novo. Se a primeira linha mostrar a janela de login, o `+event_account` não foi executado na inicialização: relatar, e o próximo passo é gravar o valor num `.cfg` (seção 4.4).
