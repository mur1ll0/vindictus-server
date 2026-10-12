# Vínculo de itens e equipamento avançado

M13.2d.2b.2c.2b.3a porta CheckCharacterBind e CheckHighLevelEquipment para src/domain/item_binding.rs. É uma decisão de domínio executável, com adaptador concreto para FeaturePolicy e catálogo de encantamentos. Não carrega ainda o catálogo efetivo completo, não entrega itens nem atualiza Processed.

## Ordem recuperada

CheckCharacterBind retorna true para info ausente ou vínculo anterior. Em seguida, item não equipável diferente de gold retorna true. Bind e TradeOnBind são avaliados nesta ordem, com curto-circuito; null/vazio conservam o resultado true da FeatureMatrix. Classes que começam com enchant_scroll consultam Macro_EnchantTradeRestriction e retornam diretamente. As demais passam pelo teste de equipamento avançado.

CheckHighLevelEquipment retorna false se não equipável. Depois testa a classe succubus_wing_upper_cash em minúsculas, ENHANCE convertido para Int32 ≥9, presença de LOOK, QUALITY.Arg ≥5, PREFIX.EnchantLevel ≥7, SUFFIX.EnchantLevel ≥7 e gemas. Cada etapa pode dispensar as seguintes. LOOK com elemento null ainda está presente; os outros atributos com elemento null são tratados como ausentes. ENHANCE.Value null converte para zero; texto inválido e overflow conservam erros distintos. Encantamento ausente não vira nível zero: o NullReferenceException original é representado por MissingEnchant.

GemstoneTypes vem de Enum.GetValues: Diamond=0, Sapphire=1, Ruby=2, Emerald=3. O loop consulta BraceletMaxSlotCount a cada condição, incluindo a final falsa. Quantidade negativa/zero não consulta gemas; quantidade >4 só falha ao acessar o quinto índice. Uma gema anterior pode retornar true e evitar esse erro. O erro original de índice passa a GemstoneRange, sem panic.

## Representação e integração

Classes, nomes e valores continuam bytes UTF16LE opacos, sem normalização ou exposição em erros. Views emprestadas conservam elementos null, valor null e Arg. Limites explícitos: 512 unidades por campo, 256 atributos, 8192 encantamentos; UTF16 com quantidade ímpar de bytes e nomes duplicados são recusados. Construir uma view aplica os limites antes da decisão, uma fronteira intencional de validação.

CatalogBindingContext usa FeaturePolicy já verificada e lookup ordinal exato dos encantamentos. O source usa ToDictionary para EnchantInfo, portanto duplicatas são erro; variantes de case permanecem distintas. check_parsed_character_bind conecta os atributos já recuperados de ItemClassEx sem copiar seus valores. BindingInfo é uma projeção que o futuro loader deve fornecer depois de recuperar o catálogo: equippable significa presença da chave exata em equipItemInfoDic.

O consumidor original seleciona esta regra para MobileAppBuy, com correio AuctionItemMail. Ela não aplica a exceção de gold/silver_coin do DefaultCharacterBinderChecker. O dispatcher de criação/entrega continua pendente, inclusive sua recusa prévia quando ItemClassInfo não existe. O true do predicado com info ausente não autoriza criar classe inexistente.

## Cultura e evidência

A comparação StartsWith e o ToLower originais usam cultura corrente. O adapter aceita somente classes ASCII imprimíveis quando alcança essas operações, com perfil en-US explicitamente selecionado. Unicode, controles e NUL nesse ponto retornam CultureUnsupported. Curto-circuitos anteriores continuam podendo retornar true sem operar sobre cultura. A configuração regional offline sustenta a escolha de perfil, mas não prova a cultura da thread do servidor em execução.

scripts/reverse/generate-item-binding-fixtures.ps1 extrai apenas os três predicados revisados e o enum para .cache, compila um harness próprio Framework 4.8 x86/x64 e produz fixtures sintéticas. Os corpos dos predicados não são reescritos. GetEquipItemInfo é substituído pela projeção de presença, sem traçar suas chamadas; consultas de feature, inteiro e encantamento usam dependências sintéticas, cujas chamadas são comparadas. Isso verifica a decisão, não a carga real de conteúdo.

São 2017 casos por arquitetura, 4034 comparações de resultado/erro/trace e 7 recusas culturais por arquitetura. Inclui mutações dos 95 caracteres ASCII imprimíveis do prefixo, case da classe especial, valores limite/erros Int32, curto-circuitos, null, slots e catálogo ausente. Sete testes Rust também exercitam o adaptador concreto, integração com parser, duplicatas e limites.

## Próxima dependência

M13.2d.2b.2c.2b.3b: carregar a projeção efetiva de EquipItemInfo/ItemClassInfo com filtros feature, IsValid/MaxStack, duplicatas last-wins e ajustes pós-load (Storage e tabelas auxiliares), ordem e cultura explícitas. Depois recuperar índices/defaults/identities/tipos, snapshots e importação do inventário antes da entrega real. A migração e o login do cliente permanecem incompletos.

Verificação e hashes: docs/evidence/auth/item-binding.json, docs/evidence/verification/gate.json e REPORT.md. O SQL Server original, snapshots privados e fila real são preservados.
