# Executor transacional de recompensas de login

Etapa M13.2d.2b.2b.2. API interna `application::login_rewards::apply_existing_login_rewards`; não há endpoint de jogo nem comando que conceda recompensas a contas reais automaticamente.

## Contrato

O chamador fornece UID, nome ASCII com igualdade recuperada, revisão esperada, UUID estável, pacote privado de conteúdo e os cinco clocks UTC independentes das decisões. A data SQL de INSERT de lealdade é explícita. O callback síncrono e limitado de relógio das recompensas é chamado uma vez por item elegível na primeira execução; um replay não o chama.

A transação reivindica primeiro o UUID em `game_login_reward_runs`, bloqueia o usuário, confere UID/nome/revisão e consulta lealdade, dormência e as duas observações condicionais de `GetLatestConnectedDate`. Essas observações usam o CASE/MAX original e convertem unidades SQL datetime para os ticks CLR comprovados pelo oracle. Intervalos de features só são convertidos nos ramos que o código original alcança. `GetCharacters` é chamado somente quando o plano exige recompensas, com a ordem COALESCE(OrderNum,255),ID e locks FOR SHARE nas linhas observadas.

As intenções são executadas em ordem: flag de amigo, liberação/definição de dormência, consulta de personagens/planejamento/inserção da fila, atualização e leitura pós-escrita do evento. Cada UPDATE do usuário avança o trigger de revisão existente; uma alteração somente na lealdade também avança a revisão do usuário. Não existe revision_after=revision+1 obrigatório para um lote com várias escritas.

O recibo guarda flags, revisão histórica e IDs da fila junto com as alterações, em um único commit. Seu request vincula revisão, clocks e SHA256 semântico do manifesto, mapa de features resolvido e todas as recompensas ordenadas, incluindo duplicatas e bytes UTF16. Alterar os valores em memória mantendo os hashes do arquivo não permite reaproveitar o recibo. Outro UID/nome, conteúdo ou clock com o mesmo UUID retorna conflito. Replays idênticos devolvem o resultado histórico mesmo após atualização do usuário, processamento/exclusão dos itens ou remoção dos personagens.

## Datas e limites

`SqlDateTime2::sql_parameter` reproduz `new SqlDateTime(DateTime)` e o envio de parâmetros SqlDbType.DateTime pelo SqlClient Framework. Essa conversão arredonda diretamente para 1/300s; não é a conversão datetime2→datetime do SQL Server compatibility100, que já possui outro contrato no projeto. O caso especial DateTime.MaxValue e valores que estouram ao arredondar foram testados. A leitura inversa continua usando `SqlDateTime::framework_date_time`.

Limites: 1000 personagens, 1000 linhas de catálogo, 8192 itens por lote, request de recibo <=2048 bytes, statement_timeout5s, lock_timeout2s e deadline total15s incluindo aquisição do pool. Callback de clock não deve bloquear ou fazer IO. Uma falha, cancelamento ou revisão obsoleta desfaz usuário, lealdade, fila e recibo. A sequência PostgreSQL conserva lacunas de identity após rollback, como o contrato SQL já recuperado. Identity não inicializada ou esgotada recusa o lote.

## Diferenças deliberadas e limites de paridade

O legado fazia chamadas separadas e usava NOLOCK. Este executor oferece atomicidade, dados committed, bloqueio do usuário, CAS e replay durável. A fase de observação obtém as datas antes de executar as intenções; a fonte fazia ReleaseDormant antes da segunda consulta. Sob concorrência externa, essa mudança pode alterar o momento observado. As duas consultas continuam separadas em READ COMMITTED; não se promete snapshot global nem bloqueio de novos personagens. FOR SHARE mantém nome/nível/ownership das linhas retornadas até o commit. Escritores externos de outras tabelas devem adotar o mesmo protocolo de revisão; isso ainda não é enforced globalmente.

Os clocks dos itens são amostrados durante o planejamento, antes dos INSERTs, em vez de durante roundtrips separados das procedures originais. Os clocks explícitos e o recibo tornam esse comportamento testável e os retries seguros; não se afirma equivalência temporal de chamadas reais não sincronizadas. Um commit com resposta perdida deve ser consultado repetindo exatamente o mesmo request/UUID; não trocar UUID para tentar novamente.

`game_login_reward_runs` é operacional, sem FK a usuários ou itens: seu resultado histórico precisa sobreviver às exclusões. Placeholders incompletos não são aceitos como resultados e nunca são publicados pelo executor antes do commit. Não reutilizar UUID de outra tentativa. Retenção/limpeza dos recibos exigirá política operacional que preserve a janela de retries.

Criação de novos usuários, ownership de achievement nesse fluxo, FacebookToken, ordem completa com ban/IPLock/frontend/UserLogin, consumidores da fila, inventário/correio e autenticação do cliente continuam pendentes. A transação não entrega itens ao inventário.

## Verificação reproduzível

```powershell
./scripts/reverse/generate-datetime-parameter-fixtures.ps1
./scripts/reverse/generate-login-reward-executor-fixtures.ps1
# TEST_DATABASE_URL deve apontar para PostgreSQL com permissão CREATE SCHEMA.
./scripts/dev/verify.ps1 -DatabaseIntegration
docker compose build --quiet server
docker compose up -d --wait server
./scripts/dev/smoke-docker.ps1
./scripts/database/Import-Characters.ps1 -Mode Verify
./scripts/database/Import-Users.ps1 -Mode Verify
```

O oracle de datas usa apenas SELECT e parâmetros sintéticos no SQL Server. O oracle do executor extrai o bloco existente de UserLoginProcessor e FeatureMatrix sem executar serviços legados: os cinco UtcNow são substituídos por um clock fixo, os resultados de procedures por adapters sintéticos e ISingleResult por IEnumerable. A matriz tem48 cenários por perfil x86/x64; os clocks independentes continuam cobertos pelo oracle anterior de330 decisões. PostgreSQL executa os48 cenários em schema isolado e compara flags, datas, lealdade, ordem e18campos da fila. Testes adicionais cobrem16retries/1execução,16UUIDs/1vencedorCAS,11conflitos de request/conteúdo, falha tardia/rollback, clock/identity, cancelamento, features desligadas e upgrade9→10. Evidência final sanitizada: `docs/evidence/auth/login-reward-executor.json`.
