# Caminho para o aceite intermediário 1.69 EN

Investigação estática em 2026-10-12 a partir do Git limpo `3608a03`. Prioridade solicitada pelo usuário; objetivo final M01–M24 preservado. Cliente não executado. Esta ordem de dependências não comprova compatibilidade nem que todas as mensagens exigidas foram identificadas.

1. Bootstrap HTTP na cópia completa e framing/cifra/TypeConverter TCP. Validar precedência sql/ versus HFS em execução. Probe só recebe UserLogin tipado478; dispatcher legado também recebe categoria0 por GUID. Primeira etapa M11.6: recuperar somente envelope UserLogin comprovado, sem aceitar negociação arbitrária do peer.
2. TCP → FrontendSession: Initial → PassportCheck, NGM_disable, identidade/perfil explícito, permissões não privilegiadas, reserva/deadline/políticas, efeitos reais e retorno UserLogin. PoliciesAccepted interno não é resposta de rede. Resolver perfil efetivo, novo usuário, FacebookToken e sete campos do retorno; segurança/café/ticket dependem das features, não simulá-los silenciosamente.
3. PassportCheck.MoveToCharacterList → CharacterList.OnEnter → QueryCharacters. Conclusão inicializa segunda senha e envia CharacterListMessage (ou SingleCharacterMessage se skip/LastCID correspondem). Recuperar QueryCharacters/CharacterSummary, aparência/equipamento, slots/prologue e codecs. Snapshots64campos não são lista de rede; não inventar lista vazia/aparência para mascarar dependências.
4. Recuperar também CreateCharacter/CheckCharacterName e a alocação persistente/appearance inicial, agora exigidos explicitamente no aceite. CharacterList.OnMessageReceive seleciona índice da lista do usuário, verifica exclusão/segunda senha. CharacterSelecting adquire PlayerService por CID e executa SelectCharacter. Após dados/MapStateInfo/Hackshield, envia **LoginOkMessage**, então InGame. Não há LoginSuccessMessage neste caminho. Carregamento/persistência/ownership/codec ainda faltam.
5. InGame registra CID, envia NoticeGameEnvironment, sincroniza CharacterCommonInfo/UpdateStat, skills/títulos/quests e liga Item/Battle/Pvp/Guild. MMOChannel escolhe MMOChannelProxy ou ChannelProxy. Recuperar ramo efetivo: entrada/recomendação/join, mapa/partition, ActionSync, posição/movimento, limites e reconexão. Não presumir dependência ou independência de ProudNet de batalha; observar transporte no cliente antes de reordenar M19.
6. Executar cópia completa: bootstrap → login → criação/lista/seleção → cidade → movimento → reconexão/persistência. Logs completos em .cache/verification, somente resumos públicos. Falhas refinam caminho. Aceite intermediário não conclui batalha/inventário/recompensas/M24.

## Bloqueios concretos

- .cache/client-lab tem somente SQLite, não cliente completo; originais são referência somente leitura. URL/porta TCP27015 sem aceitação runtime observada.
- TCP diagnóstico recusa13; não há ligação TCP→FrontendSession nem respostas de lista/seleção/sucesso/cidade.
- Perfis de identidade/features limitados; runtime/cultura/arquitetura efetivos não observados. Credenciais importadas não ativas.
- Executor de contas existentes enfileira recompensas; novo usuário/retorno integral/consumidor não ligados à sessão. Catálogo/stat/rates/inventário/correio pendentes. Verificar dependências alcançadas por cenário em vez de exigir ItemContents integral para toda investigação de login.
- QueryCharacters/SelectCharacter precisam de dados além dos snapshots, codec/segunda senha/PlayerService e ownership de CID.
- InGame tem dependências adicionais e dois ramos de canal; Rust não oferece cidade/movimento. Managed não comprova transporte nativo do ramo escolhido.

## Referências locais somente leitura

Core-8942a0c26a9a/Devcat.Core.Net.Message: SerializeWriter/SerializeWriterHelper/SerializeReader/SerializeReaderHelper/MessageHandlerFactory.
FrontendServiceCore-6be475728091/FrontendServiceCore.States: Initial/PassportCheck/CharacterList/CharacterSelecting/InGame.
FrontendServiceCore-6be475728091/FrontendServiceCore.Proxy: MMOChannelProxy/ChannelProxy/BattleServiceProxy.
ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork: UserLoginMessage/CharacterListMessage/SelectCharacterMessage/LoginOkMessage.
Todos sob legacy/decompiled; hashes do oracle acompanham etapa. Originais, SQL Server, exportações e credenciais preservados.

## Dependências concretizadas pela leitura adicional

As declarações EN-EU-X-GM dos SQLite do servidor e cliente concordam nos campos inspecionados: NGM_disable/MMOChannel/HackshieldOff/enEU/EssentialSecondPassword valem1; SecondPassword declara1.08.02; NexonGameSecurity declara3.01.01; CafeBonus/CashShop/FraudDetector/XignCode sãoNULL. CurrentVer/OverrideSetting e hashes estão em [evidência sanitizada](evidence/auth/client-minimum-path.json). Isso não observa overrides runtime nem habilita automaticamente flags com valores de versão.

QueryCharactersProcessor exige QueryLastCharacterSN, QueryCharacters, slots adicionais/premium, restrição CharacterContents, contagem de preset e transferências antes de retornar oito resultados. CharacterDataContext.QueryCharacters usa GetCharacters, IsVisible/Character, IsTransCharacter, IsJumpingEventAble, CostumeManager, Vocation e GetLevelUpExp; a aparência não vem apenas do snapshot. SelectCharacterProcessor chama CharacterLoader.JoinWithCharacter e ItemProxy.OnLogin/InformInfo antes do retorno: a dependência de itens é real nesta seleção, mas não justifica bloquear o ingresso TCP por todas as tabelas de ItemContents.

Para o ramo candidato MMOChannel, JoinChannelProcessor gera chave por CID e retorna endereço/porta/ID/key, aguardando cliente por60s. MMOChannelProxy envia AddressMessage antes de concluir entrada. HeroesChannelServer.Server abre TcpServer2, usa MessageAnalyzer e envia TypeConverter; Client exige Identify(ID,Key) antes de encaminhar mensagens ao Player. Player.Enter registra Location/Look/Kinetics e observer; Action aplica KineticsChanged/RecentAction e Move troca partition. Recuperar estes contratos e a chave de uso único é parte de cidade, em conexão TCP separada do frontend; esta fonte não instala CryptoTransform no analyzer dessa conexão. Não aplicar automaticamente a cifra Heroes do frontend no canal. Observar runtime continua obrigatório.

Fontes adicionais: PlayerService-f23d8b9df77c (QueryCharactersProcessor/SelectCharacterProcessor, CharacterDataContext, CharacterLoader), MMOChannelService-bd760399728c (JoinChannelProcessor/MMOChannelService) e HeroesChannelServer-3264f671bce0 (Server/Client/Player). O inventário anterior contém MMOServer managed; ainda recuperar os seus componentes/partições e o wire completo. ProudNet de batalha não é um bloqueio demonstrado para este caminho de cidade.

O aceite agora inclui criação de personagem, autorizada explicitamente nesta continuação. CreateCharacterProcessor passa por CharacterMaker (nome/limite/SN/ID), JoinWithCharacter, CustomizeCharacter/DefaultItemInfo, Costume.Save, DefaultSkillInfo e títulos/estado inicial; estes passos não podem ser trocados por INSERT de snapshot. Segunda senha continua parte do ramo efetivo. M13.2d.3a compõe o retorno interno e provisiona usuário; ele não satisfaz essas dependências.
