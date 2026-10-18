# Prefixo de inicializacao da primeira Fiona

A composicao interna exige uma personagem local inicial, conta sintetica com revisao vigente, perfil vinculado ao catalogo e snapshot de estado preservado. Nao admite outra personagem da mesma conta, titulos/progresso ou estados de inicializacao existentes.

A ordem recuperada do construtor completo e MissionBook, TitleManager/StatManager e SkillManager. A API grava a missao, calcula/grava os atributos base e cria o slot SP0 nulo em commits separados. O primeiro INSERT de missao elege um unico caller concorrente. Replay e tentativa parcial sao recusados sem reparacao, exclusao ou retomada automatica.

GetTitleStats consulta AccountTitleStatRatio mesmo com colecoes vazias. A leitura e posterior ao commit da missao: overflow deve preservar a missao e impedir atributos/slot; formato invalido retorna zero conforme GetInteger original. A ausencia da chave na projecao e recusada; valor nulo explicito reproduz configuracao ausente. A contribuicao zero so e admitida depois de provar ausencia de titulos proprios e de outras personagens da conta. Nao implementa calculo geral de titulos adquiridos/compartilhados.

O oracle executa metodos completos originais GetTitleStats/GetInteger e CharacterStats no Framework4.8 x86/x64:72casos por arquitetura,mais duas sondas positivas que detectam consultas ao provider para titulo adquirido/compartilhado. As colecoes,GetString,log e provider sao instrumentados. Tres testes Rust comparam resultados/overflow e leitura lazy. Tres testes PostgreSQL verificam concorrencia,replay,source binding,admissao e falhas em cada fronteira de commit.

O retorno certifica somente esse prefixo. Costume,Vocation,StatusEffect,Attendance e os demais construtores ainda devem ser fechados antes de Join/UpdateLastCID/MicroPlay/presets/LoginCharacter. A fase detached de skills ja existe separadamente,depois de inventario/aparencia. Nao antecipar essas fases nem inferir que o ator completo esta pronto.

Depois faltam ItemProxy segundo Save/Undo/falhas,Summary/lifecycle/liberacao/reconciliacao e TCP Create. O legado libera a entidade temporaria antes de responder com CID;selecao recarrega equipamentos separadamente. Nao exigir frontend/cidade no contexto fresh como precondicao artificial do Create.

Esta etapa nao altera mensagens/opcodes,nao habilita Create no frontend e nao executa o cliente. Criacao persistente,relogin,selecao,cidade/movimento exigem teste real posterior. A migracao completa permanece pendente.

Evidencias:auth/fiona-managers.json,framework-initial-titles.json,gate.json e docker-smoke.json. Logs completos em .cache/verification;originais,SQLServer e exports privados permanecem protegidos/somente leitura.
