# Banimentos, IPLock e concorrência

M14.3b.3 implementa políticas e persistência comparadas ao legado. A autenticação/sessão de jogo continua pendente: nenhum endpoint operacional transforma estas políticas em login aceito.

## Regras recuperadas

GetUserBanInfo usa User.Name pelo UID, convertido ao varchar50 da procedure, para procurar UserBan.ID (nvarchar50, PK CI_AS). Seleciona Status diferente de0 e ExpireTime NULL ou estritamente maior que GETDATE. O callback bloqueia somente Status maior que0; status negativo pode ser selecionado e ainda ser considerado liberado. Status1 sempre devolve GameUI_Login_Fail_BannedReason_AccountStolen, mesmo com NotifyBanReason desligado. Outros motivos ficam vazios quando NULL ou desabilitados. Erro de consulta é estado desconhecido/erro, nunca liberação automática.

A tabela game_user_bans conserva cinco campos, incluindo motivo UTF16 opaco256 e rowversion8bytes. Não tem FK: bans do legado podem existir antes de um usuário. A política de nomes cobre ASCII50, sem alegar collation/codepage Unicode geral. A referência UserBan contém0 linhas por SELECT agregado; não havia registros reais a importar. As linhas usadas no smoke são sintéticas.

CheckLastLoginIP compara o texto UTF16 anterior com IPAddress.ToString. Uma troca de IP cria/renova o prazo; IPLockTime<=0 na FeatureMatrix usa10 minutos, Permissions.Cheat usa1 minuto. Essa permissão é entrada explícita e nunca concedida pela API. Login seguro limpa o lock e grava o IP quando há lock. IP anterior NULL é preenchido; IP anterior vazio tem efeito distinto: UserLoginProcessor suprime o prazo retornado ao cliente. Login não seguro não limpa automaticamente o prazo vencido nem troca o IP anterior. UpdateLastLoginIP faz isso somente se houver lock com vencimento<=agora.

Os dois DateTime.Now recebem amostras distintas para reproduzir passagem de tempo. A conversão TimeSpan.TotalSeconds/double/unchecked int é reproduzida nos perfis Framework x86/x64 verificados, incluindo segundos fracionários, int32 máximo e overflow que resulta em prazo0. Dados temporais persistidos e parsing continuam inteiros exatos. IPv6 compatível com IPv4 exige formatação decimal especial do Framework; a diferença do formatter Rust foi encontrada e corrigida com fixtures, incluindo ::2, ::ffff e ::1:0. Scope IDs IPv6 não fazem parte do IpAddr aceito; suporte exige contrato específico futuro.

## Relógios e referência

O banco restaurado Linux usa UTC; isso não é o fuso do servidor original. A hive SYSTEM da VM fornecida foi copiada para a pasta privada, lida sem iniciar a VM e validada por checksum/sequence numbers. Select.Current aponta ao ControlSet ativo com Singapore Standard Time, Bias=-480, sem transições DST. A DLL LoginServiceCore extraída da mesma VM tem SHA256 idêntico à referência recuperada. Evidência: docs/evidence/database/offline-vm-clock.json.

O perfil estático dessa VM é UTC+8. FixedLocalClock::reference_vm aplica esse offset a amostras UTC explícitas; o laboratório SQL UTC pode usar offset0. O código não infere fuso do host/Docker. Isso é prova de configuração offline do pacote, não observação de execução da VM. Não extrapolar para outras instalações ou regras históricas de horário de verão.

A conversão datetime2→datetime na referência em compatibility100 arredonda primeiro milissegundos e depois unidades300. O oracle comprovou diferenças perto de1/600s e antes de1900; uma quantização direta de100ns para300 não reproduzia esses casos. Dez fixtures SQL verificam o converter, carry e épocas negativas. Overflow de data/offset é erro explícito.

## Versão local

A migração0004 adiciona local_revision BIGINT inicial0, separada de legacy_last_update. Trigger BEFORE UPDATE incrementa para todo UPDATE, inclusive escritores SQL fora do adapter; tentativas de redefinir o contador e overflow são recusadas. Migração idempotente não reinicia versões. Dados e tokens importados continuam preservados.

O caso de uso lê estado/revisão, calcula a política e aplica UPDATE com WHERE id AND local_revision esperado. O trigger fornece a nova revisão; zero linhas afetadas recusa alteração obsoleta. Mudança deve retornar esperado+1, senão rollback. Operação sem mudança confere a revisão sob lock, sem UPDATE/incremento. Readiness recusa trigger desativado. Há lock_timeout2s e statement_timeout5s em transações curtas, sem chamadas externas sob lock.

## Diferenças explícitas e limites

- O callback antigo compartilha BannedReason na instância: o novo resultado pertence ao usuário, evitando troca de motivo entre sessões.
- SQL NOLOCK foi substituído por leitura de dados committed. Dirty reads e seus artefatos não são reproduzidos.
- O legado engole erro de AddMinutes fora do limite e retorna0; Rust recusa a alteração, preservando banco/versão. As fixtures marcam essa diferença deliberada.
- Credenciais/motivos/IPs não têm Debug/Serialize nem aparecem em logs. Não há rehash de IDs ou rowversion da origem.
- Políticas são APIs Rust, ainda separadas do frontend diagnóstico. IsSecuredLogin, permissões e FeatureMatrix devem vir da sessão validada futura. Personagens, LimitedAccount, efeitos/dormência e demais callbacks continuam pendentes.

## Verificação

scripts/reverse/generate-login-policy-fixtures.ps1 extrai os corpos recuperados de CheckLastLoginIP e GetMemberState. Só troca as duas chamadas de relógio por amostras e substitui DB/features/permissões/log por stubs sintéticos; não executa serviços originais. Perfis x86/x64 cobrem694 casos IP e50 callbacks cada.

scripts/database/Compare-LoginPolicy.ps1 usa o predicado WHERE recuperado sobre96 linhas sintéticas e10 conversões de relógio, somente SELECT; consulta apenas contagem da tabela real.

As integrações isoladas exercitam lookup UID/case/espaços/prazos/NULL, snapshot dos cinco campos, conflitos e erros; oito mutações concorrentes têm um vencedor, rejeitam sete versões obsoletas e preservam rowversion. Expiry inclusivo, no-op, escrita SQL externa, readiness e migração incremental são verificados. Smoke Docker conserva ban sintético, usuário/revisão e conta após restart; import-users --verify-only reconfirma os29 usuários reais sem alterá-los.
