# Argumentos temporais de ItemClassEx

M13.2d.2b.2c.2b.1 porta a cadeia de decisão de ToArgInt e a aritmética de Utility.Extention.ToInt32, com contexto temporal injetado. A gramática geral de DateTime.TryParse ainda não foi portada. Nenhum adaptador temporal foi ligado ao consumidor real da fila.

A ordem recuperada é: null retorna zero; int.TryParse retorna o inteiro; data válida converte para segundos; sufixo sec tenta um inteiro na parte anterior, amostra o clock local, soma segundos e converte; entrada comprovadamente inválida retorna zero. Um erro de perfil indisponível permanece erro, em vez de virar zero. A entrada é limitada a 512 unidades UTF16, com texto emprestado, sem alocações ou logs de argumentos no domínio.

ArgumentContext separa parse_date, EndsWith cultural, now_local e local_to_utc. None no parser significa invalidez comprovada; formato/cultura não suportado exige ProfileUnavailable. O provedor futuro deverá fixar a cultura entre os cinco perfis verificados para os sinais inteiros, defaults de datas incompletas, clock e regras locais de fuso/DST. Não há fallback para o locale/fuso da máquina Docker. O domínio exige clock Local e resultado da conversão Utc. Ticks inválidos e provedores inconsistentes são recusados.

A conversão respeita Kind: apenas Local chama a conversão UTC; Unspecified é subtraído diretamente do epoch, mesmo que o servidor use UTC+8. A divisão de ticks por 10.000.000 trunca em direção a zero, inclusive antes de 1970. O cast CLR unchecked conserva os 32 bits inferiores, inclusive após 2038. Esses detalhes não podem ser substituídos por uma conversão Unix com erro de overflow. AddSeconds com inteiro signed32 conserva os ticks originais, inclusive a fração inferior ao milissegundo, e recusa datas fora de 0001–9999 antes de converter o fuso.

O parser ItemClassEx aceita o resolver por closure e propaga ArgumentError tipado. O teste de integração resolve dois atributos relativos, confirmando duas amostras de clock independentes. O adaptador integer_only_argument existente continua restrito a inteiros; esta etapa não amplia silenciosamente a aceitação em produção.

## Oracle e limites da evidência

scripts/reverse/extract-item-argument.py extrai somente ToArgInt e ToInt32 das referências locais, cujo SHA256 fica no fixture. Código gerado/descompilado permanece ignorado em .cache. O harness próprio intercepta DateTime.TryParse, EndsWith, clock e conversão UTC, registrando chamadas. Logger é um no-op, serviços originais não são executados.

scripts/reverse/generate-item-argument-fixtures.ps1 compila x86/x64 para .NET Framework 4.8. Cinco culturas e três zonas fixas (UTC, UTC+8 e UTC-3), cem casos por perfil: 34 entradas completas, 30 relógios/segundos e 36 datas injetadas nos três Kind. Total 3.000 comparações de resultado, exceção e ordem de chamadas.

Datas completas sem offset usam o DateTime.TryParse real do Framework. O teste Rust injeta as observações desse parser e do sufixo: portanto prova a cadeia de decisão e aritmética, não uma implementação Rust da gramática cultural. Datas injetadas cobrem ticks zero/máximo, epoch±frações, 2038 e wrap adicional. Zonas fixas são fixtures do contexto, não prova de DST, datas incompletas ou conversão de offsets textuais. Não há leitura/escrita de linhas de inventário ou alteração de Processed.

Próxima subetapa: implementar e comprovar um adaptador cultural de data/sufixo e conversão local para o perfil efetivo do pacote, com formatos realmente usados e rejeição explícita dos formatos desconhecidos. Catálogo efetivo, CheckCharacterBind, snapshots/importação de inventário, correio/logs e ligação à sessão de jogo continuam pendentes.

A subetapa 2b.2 adiciona um [adaptador Rust limitado ao perfil offline](item-argument-profile.md), comparado ao parser real do Framework sem injetar suas observações. A gramática geral continua fora do escopo e a seleção não ativa entrega real.
