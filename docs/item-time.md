# Regras temporais dos itens iniciais

M15.1b.2e.4 recupera `Build_Attribute_Expire`, `Build_Attribute_MinUpTime` e o bloco temporal de `Item.Initialize`. É um componente da construção do item; não substitui o construtor completo nem entrega itens.

`domain::item_time` recebe datas tipadas `SqlDateTime2`, texto UTF16 e um relógio UTC explícito. O perfil inteiro é o en-US já usado na referência; o calendário do construtor DateTime de seis inteiros é Gregorian. Não se usa o locale, fuso ou relógio implícito do host. A conversão Gregorian é compartilhada com `item_argument_profile`, sem alterar seu parser público.

- `EXPIRE` aceita exatamente seis campos separados por `/`. Inteiros e datas inválidos mantêm a data e o atributo. O original captura os erros dentro do construtor da data; valor NULL falha em Split antes desse catch e agora retorna erro tipado. Um limite defensivo de 1024 unidades UTF16 retorna erro explícito.
- Uma data explícita anterior ao atributo remove `EXPIRE`; em igualdade ele permanece. Caso contrário, a data do atributo passa a valer.
- `MIN_UPTIME` presente com valor NULL ou vazio recebe os ticks UTC decimais de `Utility.Extention.ToFormatableString`. Ausência e texto não vazio não amostram o relógio; um espaço é texto não vazio.
- `ExpireIn` é Int32 nullable. Se presente, `Item.Initialize` sempre amostra UTC e soma segundos inteiros, preservando os ticks inferiores a um milissegundo. Falta de data adota esse resultado. Uma data existente só é encurtada se `EXPIRE` ainda existir, removendo-o; a igualdade conserva o atributo. Sem `EXPIRE`, uma data explícita permanece mesmo se for posterior à da classe.
- Datas fora do intervalo CLR e relógio indisponível retornam erros. O resultado expressa a data e a remoção do atributo; o chamador deve aplicá-los na ordem original. As funções não alteram a propriedade em caso de erro nem ocultam os efeitos de etapas anteriores do construtor.

O oracle extrai os métodos para `.cache/framework`, troca somente a expressão do relógio por uma amostra instrumentada e compila um harness próprio contra Framework4.8 x86/x64. Não executa serviços legados. Os 1848 casos por arquitetura verificam datas Gregorian, séculos e bissextos, limites Int32/DateTime, sinais/espaços/NUL, datas inválidas, NULL/vazio, expiração relativa positiva/negativa, igualdade, remoção e ordem/quantidade de chamadas ao relógio. As fixtures públicas contêm somente dados sintéticos e hashes; fontes descompiladas e logs completos continuam ignorados.

~~~powershell
./scripts/reverse/generate-item-time-fixtures.ps1
cargo test --locked --test item_time
./scripts/dev/verify.ps1 -DatabaseIntegration -DatabaseIntegrationDocker
~~~

Faltam os demais ramos de `ItemRequestProperty.Build_Common` e síntese, geração de atributos, stats, cores, durabilidade, validade/checksum e autoridade MakeEntityID; depois colocação, entrega e SaveWithoutSync/Undo/ProcessAfterDeleteItem reais. A persistência existente ainda não grava novos itens de criação. Concluir também Costume/perfil aritmético/habilidades/títulos/Summary e ligá-los ao lifecycle/frontend antes de habilitar Create TCP. Nenhuma nova execução ou compatibilidade do cliente é alegada; criação/relogin, seleção, cidade e movimento continuam pendentes.
