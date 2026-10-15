# Durabilidade de equipamentos

M15.1b.2e.6 recupera os getters/setters de Item e a última parte de Initialize.
O componente de domínio recebe o ItemStat base do equipamento e o ItemStat já
composto com atributos: HasDurability depende exclusivamente do máximo original
positivo; usar o máximo modificado nessa decisão mudaria o comportamento.

A inicialização começa com ReducedDurability/MaxDurabilityBonus zero, aplica o
máximo solicitado (Stat + bônus), depois a durabilidade solicitada (máximo -
redução). O limite MaxDurabilityLimit é entrada explícita, inclusive valores
negativos. Os dois testes de limite usam o pedido original; substituí-los por
clamp mudaria resultados. Todas as somas/subtrações Int32 são unchecked.

O setter do máximo sempre produz Update(EquipmentStat); o setter da durabilidade
somente quando sua expressão interna difere do pedido. Essa expressão difere do
getter quando máximo zero e redução negativa. O componente retorna o número de
chamadas para a inicialização e bool por setter; ainda não executa efeitos do
Item.Update. Reidratação conserva campos persistidos sem corrigi-los na leitura.
Ausência de EquipItemInfo pula a inicialização sem criar Equippable.

O oracle injeta os métodos e o trecho original de Initialize somente em
.cache/framework e compila um harness próprio Framework4.8 x86/x64 com /checked-.
São3125 inicializações,180 sequências com1620 setters e um caso sem equipamento
por arquitetura. A fixture contém apenas valores sintéticos e hashes das fontes.

## Ordem no caminho de Fiona

scripts/content/inspect_creation_item_branches.py abre o SQLite original com
mode=ro e query_only, compara hashes antes/depois e emite apenas contagens/hashes.
Os33 candidatos DefaultItemInfo de Fiona/compartilhados, antes de Feature, não
contêm expressões estendidas, prefixos avatar, categoria BRACELET ou associação
VariableStatInfo. A checagem considera até linhas desabilitadas, conservadoramente.
Isso permite antecipar durabilidade sem implementar sorteios fora desse subconjunto.
A inspeção não é autoridade de runtime nem substitui o construtor; itens adicionais
de customização/innerarmor ainda precisam ser analisados.

Build_Common geral/atributos, cores, validade/checksum/IDs, colocação,
SaveWithoutSync/Undo/limpeza, habilidades/títulos/Summary e Create TCP permanecem
pendentes. Não houve novo teste de cliente nem criação persistente/cidade/movimento.
O componente não é Item.Initialize completo e não o declara válido.
