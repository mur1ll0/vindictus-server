# Adaptador temporal do pacote offline

M13.2d.2b.2c.2b.2 implementa ReferenceVmArgumentContext em Rust: cultura selecionada en-US/gregoriano, clock UTC injetado e fuso fixo UTC+8. O código não consulta locale, fuso ou data corrente do host. A seleção é explícita; o adaptador ainda não foi ligado ao consumidor real da fila.

## Perfil recuperado e dados atuais

Seis cópias privadas de NTUSER.DAT da VM foram lidas com python-registry, sem carregar hives no registro do host ou iniciar a VM. As seis têm header/checksum/sequence numbers válidos; cinco possuem configuração regional e concordam em en-US, gregoriano, M/d/yyyy, separador de hora e AM/PM. A configuração de fuso e o hash do LoginServiceCore já recuperados comprovam UTC+8 fixo para o pacote offline.

Isso é evidência de configuração offline, não observação de cultura de uma thread em execução. Overrides gerais do NLS/colação e outras instalações não estão comprovados. O relatório público contém apenas configurações selecionadas, hashes e índices; caminhos privados de perfis Windows e conteúdo das hives não são publicados. read-offline-item-profile.py exige as cópias locais preparadas e valida SHA256 da hive SYSTEM contra a evidência anterior.

O censo somente leitura do snapshot privado confirmou 140 entradas de QueuedItem, nenhuma com atributos ou delimitadores de argumentos temporais. Não há datas reais da fila que justifiquem ampliar silenciosamente a gramática. A implementação é comparada por casos sintéticos, não por divulgação de linhas.

## Formatos aceitos

- Inteiros continuam usando o ramo Integer previamente portado, incluindo limites signed32.
- Datas completas yyyy-MM-dd e M/d/yyyy (ano com quatro dígitos).
- Data e hora separadas por espaço ou T, com segundos obrigatórios; minutos/segundos de dois dígitos e fração opcional de um a sete dígitos.
- Hora com AM/PM, inclusive variantes ASCII minúsculas, em datas separadas por espaço; intervalo de uma a doze horas.
- Segundos relativos ASCII assinados com sufixo sec. Whitespace ASCII de int.TryParse é preservado no prefixo; o sufixo é conferido no texto original, sem trim final.
- Whitespace ASCII externo de datas completas.

A combinação M/d/yyyyT... é reconhecida como inválida, reproduzindo TryParse=false; o separador ISO T só é aceito com yyyy-MM-dd. Calendário impossível e hora inválida dentro das formas reconhecidas reproduzem TryParse=false e o retorno zero da cadeia antiga. Formatos desconhecidos devolvem ProfileUnavailable: nomes de mês, ano com dois dígitos, datas incompletas, apenas hora, offsets textuais/Z, frações de mais de sete dígitos, espaços internos extras e Unicode/NLS fora do escopo. Não há substituição por zero para essas recusas. A comparação ordinal do sufixo é limitada à representação ASCII aceita e verificada; não é alegação de equivalência cultural Unicode geral.

O parser original de atributos recusa um segundo caractere de dois-pontos, inclusive dentro do argumento; assim, um timestamp com hora pode ser descartado antes de chamar ToArgInt. A aceitação do adaptador isolado não amplia essa gramática. O teste de integração usa data completa sem hora e confirma separadamente esse descarte sem callback.

Datas completas sem offset têm Kind=Unspecified e não consultam clock nem convertem fuso. O cálculo usa gregoriano 0001–9999 em ticks100ns. O clock de argumentos relativos é amostrado por argumento; exige Kind=Utc e conserva falhas do provedor. A conversão para Local aplica UTC+8 e clamping no limite superior, e Local→Utc aplica o mesmo perfil e clamping inferior, conforme o oracle. Não há regras históricas/DST genéricas.

## Verificação e reprodução

generate-item-argument-profile-fixtures.ps1 compila os métodos recuperados ToArgInt/ToInt32 para Framework4.8 x86/x64. O harness usa DateTime.TryParse e EndsWith reais, cultura en-US sem overrides e zona UTC+8 explícita; nenhuma observação de parsing/sufixo é injetada nos testes Rust.

Quatro clocks UTC × 575 entradas × duas arquiteturas: 4.600 comparações de parser, resultado/exceção e ordem das chamadas. O corpus cobre anos1/4/100/400/1600/1900/1969/2000/2038/9999, meses/limites de dias, AM/PM, sete precisões fracionárias, 2038, overflow de seconds e clamping do fuso. Dezesseis recusas são testadas por arquitetura. Testes adicionais integram o parser ItemClassEx, confirmam clock por argumento, bounds e erros de kind/provedor.

Fixtures sintéticos e hashes são versionados; C# extraído, hives e snapshot continuam privados/ignorados. Gate completo, Docker build locked, smoke/restart e conferência dos dados reais são obrigatórios para concluir esta subetapa.

Próximo: catálogo efetivo de ItemClassInfo e CheckCharacterBind/MobileAppBuy, depois modelos e importação do inventário. Novos formatos temporais só serão adicionados com evidência/oracle. Inventário/correio/logs, processamento da fila e login no cliente continuam pendentes.
