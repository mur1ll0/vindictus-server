# Vocation no construtor da primeira Fiona

O VocationManager original vem depois de Costume e antes de Whisper/Guild. O construtor liga o contexto, executa InitSkillDict e CalculateUsedSkillPoint e inicializa ChangedSkills vazio, VocationChanged=false e TransformLevel=0. Nesse ponto nao ha Save, treino, transformacao ou consulta de conteudo.

O schema 25 preserva os cinco campos de dbo.Vocation e os quatro de VocationSkill. LastTransform nullable usa ticks de 100 ns para manter a precisao de DateTime2. SkillID usa bytes UTF16LE sem normalizacao. As duas FKs originais estao ativas e confiaveis, com CASCADE em update/delete; o destino conserva essas regras. RowID e a unica chave de skills: nomes duplicados permanecem na importacao.

O importador limita e valida o snapshot, grava atomicamente e congela o hash SHA256. A sequencia usa last_value original, nunca MAX(RowID), com RESTART transacional; replay nao resemeia. Conflito desfaz inserts, marcador e alteracao da sequencia sem reparar dados existentes. As 3 Vocations e 33 skills reais foram importadas, comparadas integralmente e verificadas apos reinicio. SQL Server e originais permanecem somente leitura. O JSON privado fica fora do Git, protegido por ACL do usuario e SYSTEM.

O cache reproduz a regra original: o primeiro SkillID ordinal vence, duplicatas sao contadas sem imprimir nomes e a soma de ranks conserva overflow Int32. O oracle executa os tres metodos completos no Framework x86/x64, com 128 casos iguais por arquitetura. Cobre ausencia de Vocation, Unicode/case/surrogates, duplicatas e overflow. Parent, associacao e logger sao instrumentados; nenhum servico original e iniciado.

A API restrita consome o estado opaco de Costume, reconfere dono, revisao, fonte, atributos, missao, SP e aparencia retida. Exige preservacao inicializada e comprova ausencia de Vocation por CID. Schema ou metadados ausentes nao significam None. Vocation existente e recusada nesse perfil, embora preservada no banco e coberta no cache de dominio. O construtor nao grava nem consome identidade; falhas conservam os commits anteriores sem retomar um ator implicitamente.

Na futura composicao, CafeType ainda deve ser 0 nesse ponto do construtor fresco e IsReturn deve vir do player, conforme a ordem original. Restam Whisper/Guild, StatusEffect/TownCampfire/Attendance e demais construtores ativos; depois UpdateLastCID/MicroPlay/presets/Join/Login/CountBonus, ItemProxy e segundo Save/Undo, Summary, liberacao e reconciliacao, e TCP Create. Nao antecipar selecao/equipados ou frontend conectado como requisito artificial de freshCreate.

Nao houve novo teste do cliente nesta etapa. Create, selecao e cidade/movimento continuam indisponiveis. O objetivo completo M01-M24 permanece. Logs completos em .cache/verification; somente resumos no contexto.
