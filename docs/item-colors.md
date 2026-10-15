# Cores e materiais de equipamentos

M15.1b.2e.7 recupera Color, Material, ColorTable.PickColorInt e Item.SetColor.
O componente usa RGB24 (R no byte alto), lê triplas em ordem e ignora os
últimos um ou dois bytes incompletos. Tabelas vazias usam White na seleção
aleatória; seleção por coordenadas em tabela vazia conserva erro de divisão
por zero. Coordenadas usam x + 256*y em Int32 unchecked e resto com sinal;
índice negativo conserva erro, sem normalização adicional.

ColorRandom reproduz constructor/InternalSample/Next(max) do System.Random
Framework, incluindo seed mínimo e consumo em Next(0); máximo negativo falha
antes do consumo. É uma instância compartilhada entre as escolhas de materiais,
separada do gerador da segunda senha. O seed é explícito. O TickCount original
da inicialização do processo ainda precisa ser ligado no runtime; não há seed
fixo de produção. O seed zero do diagnóstico apenas torna a comparação repetível.

SetColor conserva NULL de pedido, valor explícito inclusive negativo diferente
de -1, LOOK ausente e LOOK sem equipamento. Material1 NULL mantém a primeira
cor; Material2/3 NULL produzem White. Chamadas seguem canais1/2/3. Erro posterior
mantém mutações anteriores e impede Update(Color) final; sucesso retorna o
sinal dessa atualização mesmo sem mudança. Efeitos Item.Update não são executados.

## Conteúdo privado e vínculo

O exportador lê SQLite/configuração/material configurado somente leitura,
verifica hashes antes/depois e conserva SELECT * EquipItemInfo sem inventar
ordenação. Nomes de material pertencem a um perfil de caminho relativo ASCII
limitado, sem diretórios/symlinks; demais perfis falham explicitamente. Falha de
leitura não é convertida em ausência. Arquivo conhecido ausente fica registrado
como NULL; vazio e bytes finais são distintos e preservados.

O sidecar v1 exige hashes, campos, ordem/índice/chave/Feature de cada linha e
hash de cada arquivo, vinculado ao catálogo efetivo já existente. Conserva o
último equipamento habilitado antes da projeção de classes. Material não listado
é erro de perfil, nunca justificativa para fabricar White. O pool distingue NULL,
arquivo conhecido ausente e material vazio. Limites:16MiB JSON,50000 linhas,
256 materiais/256 unidades UTF16 por nome,196610 bytes por arquivo e8MiB de dados.
O snapshot é imutável: FileSystemWatcher/invalidação por cultura não está migrado.

O diagnóstico inspect-item-colors recebe o sidecar por stdin e o catálogo privado
por arquivo, antes de Config/banco/listener. Emite somente contagens/digests.
Na cópia local,12377 linhas usam103 materiais,999082 triplas RGB; todos os inteiros
e32 escolhas por material, com gerador compartilhado, são comparados por digest
com os métodos originais Framework x86/x64, Rust nativo e Linux Docker. Valores,
nomes e arquivos originais permanecem no cache privado com ACL usuário+SYSTEM.

## Verificação e limite do aceite

O harness próprio injeta classes/métodos originais somente em .cache/framework,
sem FileSystemWatcher e sem serviços originais. O filesystem sintético é local
ao cache; o modo privado abre os próprios arquivos originais com FileAccess.Read.
O oracle cobre3000 SetColor,384 coordenadas,9 sequências/1188 chamadas Next,
21 comprimentos raw e11 conversões assinadas por arquitetura. Testes verificam
também limites, perfis incompletos, duplicatas, hashes, fonte/projeção efetiva e
saída por stdin sem credenciais de banco. A fixture só contém valores sintéticos.

Este componente ainda não está ligado a Item.Initialize completo/Create TCP.
Faltam atributos efetivos e validação das seleções de aparência/innerarmor,
validade/checksum/MakeEntityID, colocação/SaveWithoutSync/Undo/limpeza,
Costume/perfil aritmético/skills/títulos/Summary. Criação/relogin, seleção e
cidade/movimento continuam dependentes de teste real do cliente; M01–M24 mantido.
