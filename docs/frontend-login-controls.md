# Dependências de login e controles do frontend

M13.3d.1 preserva HomeCafeBonus e recupera cinco mensagens usadas pelo login antes de ligar a sessão ao TCP. O cliente continua recebendo a recusa diagnóstica. Não há autenticação, criação, seleção ou movimento verificados.

A migração0012 conserva os quatro campos e a chave (UID,CID,BonusType), CID default0 e índice BonusType, sem criar FK ou identity. Os dois registros de referência são exportados somente por SELECT e importados por stdin, com arquivo privado protegido para o usuário Windows atual/SYSTEM. Importação compara todos os campos, recusa conflitos e faz rollback do lote; replay não insere novamente. SQL Server permanece somente leitura.

Somente o ramo CID=NULL de GetHomeCafeBonusInfo foi portado: prioridade3→4→2 para UID/CID0; a expiração não participa da decisão. Sem linha correspondente, o parâmetro inicial permanece, inclusive NULL. A chamada interna usa identidade/revisão da sessão admitida e valor inicial0. HomeCafeBonus desabilitado ou CafeType1 evita SQL; CafeBonus ativo exige o serviço externo e é recusado. O futuro adapter deve conservar CafeType anterior em falha da consulta, conforme o catch original. Outros ramos da procedure continuam pendentes.

O perfil real EN-EU-X-GM conserva HomeCafeBonus2.02.05 ativo em CurrentVer3.09.00. O exportador privado contém40 declarações; não modifica flags para esconder dependências. O novo par foi validado no FeatureMatrix original em duas arquiteturas e cinco culturas. NexonGameSecurity continua ativo; conexão opcional indisponível e diário de login ainda exigem tratamento explícito no runtime.

| Mensagem recuperada | Categoria original | Campos |
| --- | --- | --- |
| PingMessage |521|Tag int32|
| PongMessage |523|Tag int32|
| SystemMessage |602|categoria byte e HeroesString nullable|
| NGSecurityMessage |16|bytes nullable e checksum uint64|
| CafeLoginMessage |236|HomeCafeRemainTime, AccountHomeCafeRemainTime, CafeType int32|

GUIDs, ordem de campos, null/vazio, envelope tipado/categoria0 e serializer foram comparados ao Framework x86/x64:441 casos por arquitetura. HeroesString tem limites explícitos de coleção/bytes; categorias/GUIDs desconhecidos e System.Object presente são recusados. A mensagem de bloqueio da segunda senha conserva Dialog4 e GameUI_Heroes_SecondPassword_Locked; callbacks de retorno/dormência foram incluídos nos casos sintéticos. Nenhum opcode foi criado.

OwnedFrame deixa de imprimir payload/identidade em Debug e zera seu corpo ao liberar. LegacyReceiver zera buffers completos, em erro e ao fechar. A alteração conserva bytes e continuidade ISAAC; o adapter diagnóstico existente continua enviando recusa13.

Gate integral:204 testes Rust,59 integrações PostgreSQL executadas em schemas isolados na rede Docker Linux e19 testes Python, sem falhas/avisos. Além disso,64 cenários SQL sintéticos concordaram com o ramo original, dois registros reais foram preservados/recomparados após restart e os dados anteriormente importados permaneceram iguais. [Evidência sanitizada](evidence/auth/frontend-login-controls.json). Logs completos ficam em .cache/verification.

Próximo: adapter TCP loopback para conta sintética, revisão/ownership, segunda senha/lista real, falhas e cleanup de sessão recuperados; depois teste manual no cliente. Criação/defaults/CharacterSummary, seleção/PlayerLoader e canal MMO/cidade permanecem no caminho mínimo. A migração completa M01–M24 continua pendente.
