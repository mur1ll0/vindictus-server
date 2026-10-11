# Decisões de efeitos do login (M13.2d.1)

Esta subetapa implementa o planejador de decisões recuperadas de UserLoginProcessor: lealdade de amigos, dormência, retorno e seleção de recompensas por nível. O domínio gera intenções ordenadas; não altera o banco nem entrega itens. A sessão de laboratório e o frontend TCP ainda não executam esse plano. M13.2d permanece parcial.

## Referência e comparação

scripts/reverse/generate-login-effects-fixtures.ps1 extrai os blocos originais de novo usuário/lealdade/dormência e a expressão original do predicado de DormantReward. Compila com o DormantReward.cs recuperado em .NET Framework x86 e x64. As cinco leituras de DateTime.UtcNow são substituídas explicitamente por amostras independentes; condicionais, Days e predicado ficam como recuperados. O contexto do harness registra intenções e retorna observações sintéticas; GetCharacters retorna conjunto vazio. Não executa serviços, procedimentos, entrega de itens ou dependências externas originais.

Cada perfil produz330 cenários de efeitos e686 combinações de recompensa. Inclui features desligadas/combinadas, usuário novo/existente, flag de amigo NULL/0/2, datas ausentes/futuras, instantes antes/no/depois de um dia e30 dias, extremos do calendário, limites signed e leituras de relógio diferentes. Os hashes de fonte, bloco e wrapper ficam na fixture.

scripts/database/Compare-LoginEffects.ps1 compara nove conjuntos sintéticos de CASE/MAX no SQL Server real somente por SELECT. Também registra os hashes de dez procedimentos recuperados, todos ainda sem implementação equivalente completa no PostgreSQL. Original study_* permanece intacto.

## Regras preservadas

- friend_loyalty_system com flag ausente: novo usuário recebe1; usuário existente sem data de conexão recebe1; data antiga qualifica para2 quando intervalo<=Days. Flag existente, inclusive0, impede essa atribuição.
- O plano explicita a barreira InsertNewUser antes de ReplaceAchievementOwner. No legado SetFriendRecommendFlag(1) pode ocorrer antes do INSERT; registro PassportImmigrator e autenticação não estão modelados. Não executar o plano de usuário novo sem resolver essas dependências.
- DormantAccount usa o timestamp anterior de IsDormantUser e uma leitura de GetLatestConnectedDate. ReleaseDormant ocorre quando DormantNewAccountInterval<Days, comparação estrita.
- SetDormantUser exige conexão anterior e DormantAccountInterval<=Days tanto da última conexão quanto do timestamp de dormência, se este existir. ReleaseDormant não apaga o intervalo já calculado naquela execução.
- IsDormant fica true somente quando a execução qualifica e chama SetDormantUser/recompensas. Ter um timestamp antigo no banco não basta para devolver true.
- DormantAccount_ver2 atualiza evento depois das recompensas e então consulta o evento novamente. Qualificação já torna IsReturn true; qualquer linha de evento não NULL também torna true, sem teste de idade.
- Sem ver2, mas com DormantAccount, o legado limpa o evento inclusive quando não qualifica. Com DormantAccount desligado, não consulta ou limpa o evento, mesmo se ver2 estiver habilitado.
- complete requer observação pós-escrita quando o plano exige ReadDormantEvent, inclusive após uma atualização. Resultado NULL/vazio é uma leitura realizada com conjunto vazio; ausência de execução é MissingEventRead. O chamador deve executar todas as ações com sucesso antes de completar; este módulo não comprova execução.
- O predicado de recompensa exige feature habilitada e min<=nível<=max; zero abre o respectivo limite. Valores signed e faixas invertidas não são normalizados. A política de FeatureMatrix, conteúdo, personagens, ordem/duplicidade de linhas e payload de fila devem ser portados na execução persistente.

TimeSpan.Days trunca a diferença para zero, inclusive com datas futuras. São fornecidas cinco amostras em ticks100ns, sem inferir relógio de host ou converter fuso implicitamente. O adapter futuro deve fornecer os valores efetivamente observados pelo Framework e quantizar escritas nas colunas datetime (1/300s) com o perfil SQL comprovado; este plano conserva o instante do argumento, não simula sua gravação.

GetLatestConnectedDate usa MAX(CASE WHEN LastConnectDate<LastCafeLogout THEN LastCafeLogout ELSE LastConnectDate END) por personagens do usuário. LastConnectDate NULL e LastCafeLogout preenchido continuam produzindo NULL nessa linha; usar um max entre duas datas com NULL como mínimo alteraria o legado. LastCafeLogin é usado por outro procedimento e não substitui LastCafeLogout aqui.

## Persistência e execução pendentes

No original, esses efeitos ocorrem ANTES de GetMemberState/banimento e CheckLastLoginIP. O fluxo interno atual verifica ban/IP sem executar efeitos; ligá-los exige resolver a ordem e documentar atomicidade/falhas, em vez de acrescentá-los depois silenciosamente.

Procedimentos identificados: GetLatestConnectedDate (CharacterInfo/LastConnectInfo), GetDormantUserData/ReleaseDormant/SetDormantUser/DormantUserEventSelect/Update (User), GetFriendRecommendFlag/SetFriendRecommendFlag (FriendLoyaltyInfo), ReplaceUserAchievementNIDtoUID (UserAchievement) e SendDormantItem (QueuedItem). SetFriendRecommendFlag também limpa FriendCID/RecommenderCID; a intenção não representa só escrever um número. Replace só afeta UID=-1 e nome correspondente. SendDormantItem cria fila com cores-1, ItemID NULL e Processed0; não é confirmação de entrega.

Próxima subetapa M13.2d.2: importar/mapear essas dependências com suas chaves/dados e implementar execução PostgreSQL, testes de falha/replay/concorrência e fila de recompensa. Não usar tabelas vazias ou datas presumidas para fabricar um login bem sucedido. FacebookToken exige conversão comprovada do varchar/codepage, permanecendo separado; em seguida montar o resultado UserLogin completo e ligar o frontend.

## Verificação

Cinco testes Rust novos verificam decisões/ordem/flags nos dois perfis, predicado, nove resultados SQL, preservação do intervalo após ReleaseDormant e obrigação da leitura pós-escrita. Gate completo, Docker e preservação dos29 usuários são registrados em docs/evidence/auth/login-effects.json e docs/evidence/verification/REPORT.md. Isso não conclui M13.2d, entrega de itens ou login por cliente.

