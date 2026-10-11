# Política de recompensas de contas inativas

M13.2d.2b.2b.1 recupera o catálogo DormantReward, a seleção de FeatureMatrix, a qualificação por nível e a construção dos parâmetros de SendDormantItem. O resultado é um plano puro e um diagnóstico somente leitura; a aplicação desse plano no executor de login ainda está pendente.

O conteúdo continua separado das tabelas transacionais PostgreSQL. O exportador lê heroesContents.db3 por SQLite mode=ro/query_only, confere os hashes do banco e da configuração antes/depois e produz somente o subconjunto necessário, em .cache/private-content. A cópia não entra no Git nem na imagem. Export-RewardPolicy.ps1 restringe a ACL a usuário atual/SYSTEM antes de escrever. O parser Rust limita cada entrada a512KiB, catálogo/personagens a1000, features a128 e efeitos a8192; rejeita schema inesperado, tipos incorretos, hex inválido e colunas nullable ausentes.

O GameCode50393116 da configuração seleciona EN-EU-X-GM, ServerCode1, CurrentVer3.09.00, embora a pasta da distribuição se chame zh-tw-x-gm. GameCode é resolvido pela linha do catálogo; não é inferido do nome da pasta. Overrides excluem GameCode/CurrentVer/OverrideSetting como no loader recuperado. O catálogo tem24 linhas válidas no perfil observado. O loader antigo podia logar falhas e retornar objetos parciais; esta implementação recusa linhas inválidas ou campos ausentes, em vez de continuar com dados parciais. SELECT * não possui ORDER BY: preservamos a ordem observada da leitura, identificada pelo hash do arquivo. Duplicidades não são removidas; alterações/rebuild do SQLite exigem nova comparação.

FeaturePolicy usa chaves com case exato, última atribuição vence e NULL torna-se vazio. As expressões reproduzem a separação de || antes de &&, antes do trim, com avaliação em curto-circuito; null/vazio habilita. Expressões só de espaço dão erro explícito em lugar da exceção de índice do original. GetInteger conserva o retorno0 para formato inválido, overflow como erro, whitespace ASCII do NumberStyles.Integer e NULs terminais comprovados pelo Framework. Limites de profundidade/tamanho são restrições adicionais.

CurrentVer.CompareTo é comparação textual dependente de cultura. Não usamos semver. Foram comprovados81 pares em cinco culturas (invariant/en-US/zh-TW/zh-CN/tr-TR) e nos perfis Framework x86/x64. O Rust usa somente esses pares; pares novos são recusados e precisam de novas fixtures. Essa evidência finita não prova qualquer texto ou cultura. O catálogo de teste real só exige pares cobertos. GetInteger foi comparado em21 casos por perfil, com sinais, limites, zeros iniciais, whitespace Unicode/ASCII e NUL.

O planner mantém a ordem recebida de personagens e de linhas do catálogo. Avalia Feature antes dos limites de nível;0 é limite aberto, valores signed permanecem signed. Preserva nome NULL/vazio/NUL/surrogate por bytes UTF16 e monta os identificadores de título/texto do correio recuperados. O adaptador RewardCharacter::from aceita CharacterSnapshot, mas a consulta de personagens e o executor não estão conectados aqui. Cada chamada elegível recebe uma leitura explícita de clock SQLdatetime; a inspeção fixa clock sintético0 e nunca enfileira itens.

SqlDateTime.framework_date_time reproduz SqlClient.GetDateTime em ticks100ns após o arredondamento em milissegundos do Framework, inclusive antes de1900. Foram comparados908 casos por perfil x86/x64 com SELECT de constantes sintéticas no SQL Server, sem tabelas persistentes modificadas. Esse arredondamento de callback é distinto do armazenamento exato em unidades1/300s.

## Reprodução no laboratório

Pré-requisitos: referências originais preservadas na estrutura do README, fontes recuperadas disponíveis localmente, .NET Framework4.8/Roslyn configurados e SQL Server de referência acessível. O gerador não executa serviços antigos; extrai os corpos de IsEnable/GetInteger/foreach e usa o DTO recuperado em um harness isolado. As únicas consultas SQL desse harness são SELECTs de constantes datetime.

```powershell
pwsh -File scripts/content/Export-RewardPolicy.ps1
pwsh -File scripts/reverse/generate-reward-policy-fixtures.ps1
cargo build --locked
pwsh -File scripts/content/Verify-RewardPolicy.ps1
# Após construir a imagem:
pwsh -File scripts/content/Verify-RewardPolicy.ps1 -Docker
```

Verify-RewardPolicy compara o digest de todos os parâmetros por chamada, incluindo correio UTF16, contagem signed e clock. Com as24 linhas reais e79 personagens sintéticos de níveis de fronteira, o Framework produziu74 chamadas. Rust nativo e Docker precisam ter exatamente a mesma contagem/digest. Nenhum item/nome privado é mostrado. As fixtures públicas usam apenas catálogo e personagens sintéticos; tests/reward_policy.rs verifica expressões, versões, números, datas, plano completo, limites e recusas. A evidência sanitizada fica em docs/evidence/content/reward-policy.json.

A CLI inspect-reward-policy --input-stdin --characters-file <arquivo> aceita o pacote privado pelo stdin e uma projeção JSON dos personagens (id decimal como string, level int32, name_utf16 hex ou null). Ela funciona sem conexão de banco e retorna somente perfil, contagens, hash e flags explícitas de diagnóstico. Não publica endpoint HTTP nem habilita autenticação do jogo.

## Continuação

M13.2d.2b.2b.2 deve integrar observações reais de GetLatestConnectedDate/GetCharacters e features de dormência ao executor, ligar recibos a operações estáveis e tratar transação/retry/revisão do conteúdo e do usuário. Depois vêm inventário, correio, processamento/expiração e recuperação de falhas. SendDormantItem já foi portado na etapa anterior; sua existência não representa entrega de itens. Não marcar Processed nem a conta como atualizada sem concluir o fluxo correspondente. Login, criação de personagem e gameplay no cliente permanecem pendentes.
O gate PostgreSQL executa os cenários em sequência (cargo test --test postgres --locked -- --ignored --test-threads=1), preservando tarefas concorrentes dentro de cada cenário e os deadlines de lock. As fixtures públicas geradas usam LF/UTF8 sem BOM para conservar hashes ao entrar no Git.
