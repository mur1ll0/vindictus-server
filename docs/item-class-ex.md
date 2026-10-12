# ItemClassEx e localização — M13.2d.2b.2c.2a

Esta subetapa porta a sintaxe comum usada pelo consumidor, Build de classe/atributos, igualdade de atributos interpretados e o sentinel de localização. O ramo 2c.2 permanece parcial: datas/relativos, catálogo completo, binder MobileAppBuy e snapshots de inventário ainda pendentes.

## Parser recuperado

ItemClassExBuilder separa por vírgula, [ e ], conservando segmentos vazios. O primeiro segmento é a classe sem Trim; os demais são atributos. Parse de atributos isolados considera todos os segmentos. ParseSingleAttribute possui máquina de estados, não uma expressão regular de formato estrito.

Nome/valor recebem o Trim do Framework, por unidades UTF16: inclusive NBSP e espaços Unicode previstos no Framework 4.8, preservando NUL e surrogate isolado. Parênteses/ângulos alteram estado mesmo se a entrada for malformada; vários formatos sem fechamento conservam o atributo com argumento zero. Um segundo ':' no estado errado invalida o atributo. Atributos inválidos são ignorados; nomes repetidos, inclusive nome vazio, geram erro. Case diferente é nome diferente.

O resolver recebe a substring crua somente nos fechamentos correspondentes ao estado. Não antecipar a interpretação de argumentos nem validar o formato completo antes da máquina: isso altera a ordem de chamadas/falhas do legado. Erros não expõem os valores da entrada.

Build preserva ordem da coleção e concatena nome:value, com (arg) e <arg2> apenas quando não-zero, e colchetes se há atributos. Não pressupor que Build/Parse formem roundtrip para valores com delimitadores. Igualdade entre resultados de Parse compara classe, nomes, valores e ambos argumentos, ignorando ordem da inserção.

Rust aceita no máximo 1024 bytes UTF16 da coluna QueuedItem.ItemClassEx, tem limite defensivo de 256 atributos e Build limitado a essa mesma largura. Nenhum buffer textual novo tem Debug/Serialize ou logs; os buffers intermediários UTF16/bytes são zeroizados. Erros são tipados e null é recusado. Conveniências Build de enhance/quality e o parser somente de classe não foram adicionados; o caminho classe+atributos usado na entrega está portado.

## Argumentos: limite explícito

O ToArgInt original tenta int.TryParse, depois DateTime.TryParse, depois sufixo sec com DateTime.Now.AddSeconds; ao falhar retorna zero com aviso. Utility.ToInt32 converte Local para UTC, divide ticks do epoch1970 truncando para zero e faz cast signed32. Cultura, relógio e timezone são dependências reais.

O parser Rust exige resolver explícito. try_integer_argument reproduz apenas a primeira tentativa Integer nos cinco perfis comprovados; integer_only_argument retorna erro se o ramo inteiro não aceitar a entrada. Não converte datas ou inválidos silenciosamente em zero. O futuro adapter deverá recuperar os outros ramos e seus clocks antes de tratar todas as entradas reais. O resolver dos testes é determinístico: usa int.TryParse para inteiros e um valor sintético nos demais, gravando todas as substrings chamadas. O oracle prova a sintaxe/ordem dessas chamadas, não a interpretação temporal completa.

## Localização e inventário

ItemLocation.IsNull exige exatamente Collection=255 e Slot=-1. Collection=255 com outro slot, ou Slot=-1 em outra coleção, não é null. O modelo preserva byte e slot signed32 e igualdade; não porta GetHashCode nem métodos de conveniência. A localização null não prova Item.IsValid ou ausência de OwnerID.

O catálogo sanitizado inventory-contract.json recupera cinco tabelas/31 colunas: Item9, ItemAttribute6, ItemCollection2, ItemStorageStatus7 e Equippable7. Item.OwnerID pode ser NULL; ExpireDateTime é datetime2; storage ExpireTime é datetime; Attribute/Value são varchar50, enquanto classe de Item é nvarchar50. Não converter UTF16 dos atributos a varchar por uma codepage presumida.

FKs: Item e ItemStorageStatus → ItemCollection; ItemCollection → Character, com UPDATE/DELETE CASCADE. ItemAttribute e Equippable não têm FKs declaradas nesse catálogo. IDs destas tabelas não devem receber um allocator presumido; o manifest conserva as identity declaradas. Não há nova migration, importação ou escrita de inventário nesta subetapa.

## Catálogo e vínculo ainda não portados

ItemContents inicializa ItemClassInfo com FeatureMatrix.IsEnable(row.Feature) antes de row.IsValid(), sobrescreve chaves duplicadas e aplica ajustes adicionais. GetItemClassInfo retorna null para chave null/ausente; o lookup é por chave exata.

CheckCharacterBind: info null → true; vínculo explícito → true; não equipável e não gold → true; Bind; TradeOnBind; prefixo enchant_scroll → Macro_EnchantTradeRestriction; demais → CheckHighLevelEquipment. StartsWith/ToLower do Framework usam cultura: não substituir por casefold/ordinal sem oracle específico.

CheckHighLevelEquipment depende de classe cash especial, ENHANCE.Value Convert.ToInt32 ≥9, LOOK existente, QUALITY.Arg ≥5, PREFIX/SUFFIX com EnchantLevel ≥7 e slots de bracelete/gemstone. Lookup de enchant ausente pode lançar exceção. A lista de dependências é recuperação do código, sem alegação de execução Rust desse binder.

GetSystemItem consulta ID via SingleOrDefault e exige Item.IsValid e localização null. IsValid pode recriar Equippable conforme FeatureMatrix e carregar atributos/checksums. Não reduzir esse contrato a um SELECT de Item ou ao sentinel de localização.

## Evidência e reprodução

scripts/reverse/generate-item-parser-fixtures.ps1 compila métodos extraídos de ItemClassExBuilder com resolver injetado, DTO e ItemLocation recuperados. Código descompilado fica em .cache, sem commit. Duas arquiteturas × cinco culturas: 327 casos de sintaxe por cultura, 33 int.TryParse e 24 igualdades; 48 localizações por arquitetura. O corpus cobre null/vazio, UTF16 opaco, delimitadores repetidos/sem fechamento, duplicate/case, signed32, Trim, NUL e ordem de resolver. Seis testes Rust novos incluem limite de Build e recusa de argumentos temporais não resolvidos.

py -3.13 scripts/reverse/catalog-inventory-contract.py exporta somente estruturas/hashes, sem linhas ou definições originais. Gate scripts/dev/verify.ps1 -DatabaseIntegration, build Docker locked, smoke/restart e snapshot Verify continuam obrigatórios.

Próxima subetapa 2c.2b: resolver temporal com perfis explícitos, catálogo/CheckCharacterBind e modelos/importação de inventário com tipos/collation/cascades comprovados. Entrega/correio/logs e login de cliente permanecem pendentes.
