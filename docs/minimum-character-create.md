# Create mínimo do laboratório

O operador priorizou criar Fiona e entrar no mapa inicial, adiando correio, pets e serviços acessórios. Esta implementação oferece um caminho nativo explícito; não representa a composição integral dos atores legados nem conclui a migração M01–M24.

`lab-frontend --minimum-creation-assets <configuração privada>` habilita Create. Sem essa opção, a criação continua indisponível. O launcher recebe `-MinimumCreationAssetsFile`; exige o arquivo preparado sob `.cache/private-content`, ACL restrita e prova de nomes. `inspect-minimum-creation-assets --assets-file <arquivo>` valida os vínculos dos pacotes sem abrir banco ou listener e sem imprimir conteúdo privado.

O perfil admite somente o primeiro personagem Fiona normal, nível inicial, sem preset/premium, pertencente à conta sintética autenticada e à revisão atual. A segunda senha precisa estar liberada. O nome e o template vêm da mensagem recuperada CreateCharacterMessage, categoria 482, inclusive a forma com GUID na categoria zero. CID, itens e habilidades usam a mesma autoridade de entidades do processo.

Após a base e os efeitos Dormant já recuperados, o caminho compõe missão/base stats/SP, entrega inicial de inventário, aparência e habilidades com seus callbacks detached. Registra o último CID e confirma as quantidades gravadas. A migration 33 mantém um marcador distinto de inicialização mínima, vinculado ao dono e ao fingerprint do perfil. Só após o readback completo ele se torna inicializado. Um marcador parcial nunca produz lista nem permite replay automático.

O worker acompanha cancelamento e shutdown. Cancela e aguarda o inicializador antes de compensar a base comprometida; cascades removem itens, aparência, habilidades e marcador. Uma falha de compensação impede sucesso. Os códigos recuperados de falha de customização, salvamento de aparência e inicialização são conservados. Os logs registram apenas a fase e o código, sem nome, template, senha ou conteúdo privado. Identidades e journals independentes não são rebobinados.

A resposta de sucesso é o CharacterListMessage 479 já recuperado. A mesma consulta é usada no relogin: dono/revisão, procedência sintética, marcador completo, perfil, Costume e ausência validada de Vocation. Usa os campos persistidos e o XP do conteúdo; calcula slots/prologue com os providers existentes. Não transforma personagem legado ou parcial em lista vazia. O codec não mudou: reutiliza as fixtures originais de 273 casos por arquitetura. O bloco de espera de exclusão de `Character.MakeSummary` tem oracle adicional de 96 casos Framework x86/x64, com conteúdo e UTC injetados somente nas fronteiras.

Guild, pets, café/retorno/VIP, eventos, arena e outros serviços acessórios ficam sem ator nesse perfil. A lista usa o estado inicial explicitamente delimitado, incluindo Pet nulo e BriefGuildInfo vazio; não é uma projeção geral de personagens existentes. Grants de títulos, histórias/quests e progressão completa continuam fora deste caminho mínimo. O seed de cores é explícito de laboratório; não reproduz a agenda/TickCount do processo original. A aritmética corporal fica em `unambiguous-only` até existir evidência para selecionar uma arquitetura nos casos ambíguos.

## Próximo caminho até o mapa

1. Executar Create e relogin no cliente 1.69 EN com o frontend atualizado. Testes TCP sintéticos não certificam aceitação pelo jogo.
2. Receber SelectCharacterMessage como índice da lista, validar ownership e segunda senha, carregar o personagem mínimo e enviar os dados recuperados de seleção. O legado retorna LoginOkMessage, seguido do estado InGame e dos dados de personagem/stats/habilidades exigidos pelo cliente.
3. Ligar EnterChannel à admissão de cidade. O legado entrega ChannelServerAddress com endereço, porta e chave e abre uma conexão separada com HeroesChannelServer. Recuperar e validar esse transporte/handshake antes de implementar a admissão; não substituir por sucesso no frontend.
4. Recuperar os pacotes HeroesChannelServer de entidade/partição/ActionSync, validar posição/movimento e persistir o estado mínimo necessário. Executar o cliente e confirmar entrada, caminhada e relogin.

As fontes de seleção são `FrontendServiceCore.States.CharacterList`, `CharacterSelecting`, `InGame`, `FrontendServiceCore.Proxy.MMOChannelProxy`, `SelectCharacterProcessor`, `MMOChannelService.Processor.JoinChannelProcessor` e `HeroesChannelServer.Server`. `SelectCharacterMessage.Index` indexa a lista, não o CharacterSN. `LoginOk.RegionCode` converte o valor 1 em 0. `EnterChannel` carrega ChannelID, PartitionID e ActionSync. Esses achados são estáticos; nenhum deles certifica execução nativa de seleção/cidade.

Verificação completa e ativação são registradas em MIGRATION.md, no relatório corrente e em `docs/evidence/auth/minimum-character-create.json`. Logs completos permanecem em `.cache/verification`; somente resumos devem ser coletados no terminal. Originais e SQL Server continuam somente leitura.
