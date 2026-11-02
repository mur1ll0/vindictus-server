# Roteiro de cliente — launcher (trilha L)

Plano e evidência: `docs/future/game-launcher.md` (seções 4.1 e 4.4). A preparação completa, a ordem de gameplay e o modo de relatar entram aqui no gate L6.

### LA1 — Start Game direto para a seleção de personagem (login pelo launcher)
Primeira tentativa (02/11/2026, `+event_account` com `patch_client_auto_login.py`): a página de login continuou aparecendo; o botão Start Game não passa por aquele ponto. Patch trocado.
Pré-condição: Docker no ar (`docker compose up -d --wait`); cópia `.cache/play-client` com `client.dll` `0b7d0248…` (`patch_client_launcher_login.py`, aplicado em 02/11/2026); contas `admin` e `lab169_a`; jogo fechado. O `start_login.bat` da cópia já usa o parâmetro novo.
- [ ] `start_login.bat` (`vindictus.exe -stage -dev -lang en-eu-x-gm -noupdate -threads 4 -launcher_ticket admin`) → tela de título → Start Game → **seleção de personagem sem preencher ID e senha** (a página pode piscar) → segunda senha como sempre → entrar na cidade.
- [ ] Fechar e abrir pelo `start-lab.bat` (sem o parâmetro) → Start Game → a página de ID e senha aparece como antes; entrar digitando o login.
- [ ] Trocar `admin` por `conta_inexistente_la1` no `start_login.bat` → Start Game → recusa do servidor; anotar a mensagem, se a página de login aparece e se dá para entrar digitando um login válido.
Limites conhecidos: ainda sem ticket (a conta entra pelo nome, como hoje); depois de uma recusa, o próximo Start Game repete o mesmo valor. Se a página continuar aparecendo sem entrar sozinha, relatar se ela piscou e o horário, para comparar com o log do `game`.
