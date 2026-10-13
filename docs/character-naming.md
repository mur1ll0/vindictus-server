# Regra de formato do nome de personagem

M13.3d.2c.2a implementa somente CharacterMaker.CheckValidName para o padrão EN-EU efetivamente observado. O leitor de perfil abre heroesContents.db3 em mode=ro/query_only, confere hashes antes/depois e recupera NamingRule, MaxBytes20, Repeating-1 e ProtectPeriod30. Não publica a lista ForbiddenNameInfo nem implementa sua consulta.

O domínio recebe os valores explicitamente, recusa outros padrões e trabalha com unidades UTF16 para conservar String.Length e a iteração char do Framework. A expressão observada permite no mínimo quatro letras ASCII, ou dois segmentos de pelo menos duas letras separados por um único underscore. O `$` original também aceita uma única quebra LF terminal: o limite ainda conta essa unidade. Não há trim, reparo de surrogates nem normalização inventada. NULL conserva o erro recuperado; não equivale ao nome vazio. A restrição de padrões é deliberada: não se declara portado o mecanismo geral de expressões regulares do .NET.

O harness público próprio contém um placeholder; somente a cópia ignorada recebe o método descompilado original. Fixtures sintéticas com1050casos por arquitetura x86/x64 cobrem NULL, caixa, underscore, números, espaços, NUL, LF/CRLF, Unicode, limites negativos/zero e repetição. Dois testes Rust comparam ambas as arquiteturas e recusam padrões não recuperados; nenhum nome privado é publicado.

Reprodução: scripts/reverse/generate-character-name-fixtures.ps1; scripts/dev/verify.ps1 -DatabaseIntegration -DatabaseIntegrationDocker. Logs completos em .cache/verification; fontes e SHA256 na fixture e na evidência sanitizada.

Essa regra não indica disponibilidade de nome. CheckCharacterName ainda depende de ForbiddenName filtrado por LanguageTag/cultura, ReservedName com seu efeito de consumo, duplicidade SQL CI_AS e janela UTC de CharacterNameLog. QueryReservedInfo também precisa da preservação de ReservedName/ReservedTitle. Esses handlers continuam indisponíveis. Create exige adicionalmente allocadores CID/SN, criação SQL/defaults/aparência/itens/skills/CharacterSummary; seleção e cidade precisam de PlayerLoader e canalMMO. Não há nova tentativa útil de Create enquanto essa cadeia estiver incompleta.

Gate final:217Rust/71PostgreSQL executados/19Python, zero falhas/avisos. Docker build/migrate/readiness/smoke/restart e comparações de preservação passaram. [Resumo e hashes](evidence/auth/character-naming-rule.json). O runtime TCP continua com os handlers da etapa2c.1; esta regra de domínio ainda não está ligada a Check/Create.
